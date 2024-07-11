use crate::common::types::PresentedHappBundle;
use crate::hpos::Ws;
use anyhow::{anyhow, Result};
use holochain_client::{AdminResponse, InstalledAppId};
use holochain_client::{AgentPubKey, AppInfo};
use holochain_conductor_api::{AppStatusFilter, CellInfo};
use holochain_types::app::{AppManifest, CreateCloneCellPayload, InstallAppPayload};
use holochain_types::dna::{ActionHash, DnaHashB64};
use holochain_types::prelude::{
    AppBundleSource, ClonedCell, DnaModifiersOpt, RoleName, YamlProperties,
};
use hpos_hc_connect::app_connection::CoreAppRoleName;
use hpos_hc_connect::sl_utils::{sl_clone_name, SlCloneSpec, SL_BUCKET_SIZE_DAYS};
use hpos_hc_connect::AppConnection;
use mr_bundle::Bundle;
use std::collections::HashMap;

use super::types::{CellInfoMap, RawInstallAppPayload, SuccessfulInstallResult};

pub struct FixedDataForSlCloneCall {
    pub bound_hha_dna: String,
    pub bound_hf_dna: String,
    pub holo_admin: String,
    pub bucket_size: u32,
    pub time_bucket: u32,
}
impl FixedDataForSlCloneCall {
    pub fn init(
        core_happ_cell_info: &CellInfoMap,
        bucket_size: u32,
        time_bucket: u32,
    ) -> Result<Self> {
        Ok(Self {
            bound_hha_dna: get_base_dna_hash(&core_happ_cell_info, CoreAppRoleName::HHA.into())?,
            bound_hf_dna: get_base_dna_hash(
                &core_happ_cell_info,
                CoreAppRoleName::Holofuel.into(),
            )?,
            holo_admin: get_sl_collector_pubkey(),
            bucket_size,
            time_bucket,
        })
    }
}

pub fn build_json_sl_props(bound_happ_id: &str, data: &FixedDataForSlCloneCall) -> String {
    format!(
        r#"{{"bound_happ_id":"{}", "bound_hha_dna":"{}", "bound_hf_dna":"{}", "holo_admin": "{}", "bucket_size": {}, "time_bucket": {}}}"#,
        bound_happ_id,
        data.bound_hha_dna,
        data.bound_hf_dna,
        data.holo_admin,
        data.bucket_size,
        data.time_bucket,
    )
}

pub async fn handle_holochain_enable(
    admin_connection: &mut hpos_hc_connect::AdminWebsocket,
    installed_app_id: &InstalledAppId,
) -> Result<AppInfo> {
    match admin_connection.enable_app(installed_app_id).await {
        Ok(r) => match r {
            AdminResponse::AppEnabled { app, errors } => {
                if !errors.is_empty() {
                    return Err(anyhow!("Warning while enabling app with installed_app_id {:?}.  Errors: {:#?}", installed_app_id, errors));
                }
                Ok(app)
            },
            _ => Err(anyhow!("Failed to enable installed_app_id {:?}.  Received invalid conductor admin response: {:#?}", installed_app_id, r))
        },
        Err(e) => Err(e)
    }
}

pub async fn handle_install_app_raw(
    admin_connection: &mut hpos_hc_connect::AdminWebsocket,
    payload: RawInstallAppPayload,
) -> Result<SuccessfulInstallResult> {
    let uid_override = get_uid_override();
    let installed_app_id = payload.installed_app_id.clone();

    let p = InstallAppPayload {
        ignore_genesis_failure: false,
        source: payload.source,
        agent_key: payload.agent_key,
        installed_app_id: Some(payload.installed_app_id),
        membrane_proofs: payload.membrane_proofs,
        network_seed: if payload.uid.is_some() {
            match &uid_override {
                Some(uid) => Some(format!("{:?}::{:?}", payload.uid.unwrap(), uid)),
                None => Some(payload.uid.unwrap()),
            }
        } else {
            uid_override
        },
    };
    log::trace!("Starting installation of app with bundle: {:?}", p.source);

    match admin_connection.install_app(p).await {
        Ok(r) => match r {
            AdminResponse::AppInstalled(a) => Ok(SuccessfulInstallResult::New(a)),
            _ => Err(anyhow!("Failed to install app with installed_app_id {:?}.  Received invalid installation response: {:#?}", installed_app_id, r))
        },
        Err(e) => {
            log::warn!("Warning while installing app {:?} : {:?}", installed_app_id, e);

            // Don't return installation error whenever app is already installed
            if !(e.to_string().contains("AppAlreadyInstalled") || e.to_string().contains("CellAlreadyExists")) {
                return Err(e);
            }

            Ok(SuccessfulInstallResult::AlreadyInstalled)
        }
    }
}

pub async fn update_happ_bundle(
    mut source: AppBundleSource,
    modifier_props_json: String,
) -> Result<AppBundleSource> {
    let bundle = match source {
        AppBundleSource::Bundle(bundle) => bundle.into_inner(),
        AppBundleSource::Path(path) => Bundle::read_from_file(&path).await.unwrap(),
    };
    let AppManifest::V1(mut manifest) = bundle.manifest().clone();
    for role_manifest in &mut manifest.roles {
        let properties = Some(YamlProperties::new(
            serde_yaml::from_str(&modifier_props_json).unwrap(),
        ));
        log::trace!("Updated app manifest properties: {:#?}", properties);

        role_manifest.dna.modifiers.properties = properties
    }
    source = AppBundleSource::Bundle(
        bundle
            .update_manifest(AppManifest::V1(manifest))
            .unwrap()
            .into(),
    );

    Ok(source)
}

// Install & enable the sl instance and create the first time-bucket clone.  Returns the sl app id.
pub async fn install_assigned_sl_instance(
    ws: &mut Ws,
    //    admin_connection: &mut hpos_hc_connect::AdminWebsocket,
    happ_id: &String,
    host_pub_key: AgentPubKey,
    core_happ_cell_info: &CellInfoMap,
    sl_path_source: AppBundleSource,
    bucket_size: u32,
    time_bucket: u32,
) -> Result<String> {
    log::debug!(
        "Starting installation process of servicelogger for hosted happ: {:?}",
        happ_id
    );

    let mut admin_connection = ws.admin.clone();

    let mut data = FixedDataForSlCloneCall::init(&core_happ_cell_info, bucket_size, 0)?;

    // base instance uses 0 timebucket for now.  This will be removed when we can do CloneOnly install.
    let sl_props_json = build_json_sl_props(happ_id, &data);

    let sl_source = update_happ_bundle(sl_path_source, sl_props_json.clone())
        .await
        .unwrap();

    data.time_bucket = time_bucket;
    let sl_props_json = build_json_sl_props(happ_id, &data);

    // Note: Assigned sl apps are those associated with a hosted happ
    // This is different than the baseline core sl app stored in WS.
    let assigned_sl_id = get_sl_id(happ_id);

    let sl_install_payload = RawInstallAppPayload {
        source: sl_source,
        agent_key: host_pub_key,
        installed_app_id: assigned_sl_id,
        membrane_proofs: HashMap::new(), // sl apps do not require mem proofs
        uid: None, // sl apps should use the pure `DEV_UID_OVERRIDE` env var as the network id
    };

    let sl_app_id = match handle_install_app_raw(&mut admin_connection, sl_install_payload).await? {
        SuccessfulInstallResult::New(a) => a.installed_app_id,
        SuccessfulInstallResult::AlreadyInstalled => get_sl_id(&happ_id),
    };

    handle_holochain_enable(&mut admin_connection, &sl_app_id).await?;

    let app_ws = ws.get_connection(sl_app_id.clone()).await?;
    handle_install_sl_clone(app_ws, sl_props_json, time_bucket).await?;

    Ok(sl_app_id)
}

// do the cloning but ignore any duplicate cell errors
pub async fn do_sl_cloning(
    app_ws: &mut AppConnection,
    happ_id: &str,
    sl_clone_data: &FixedDataForSlCloneCall,
) -> Result<Option<ClonedCell>> {
    let sl_props_json = build_json_sl_props(&happ_id, sl_clone_data);
    let r = handle_install_sl_clone(app_ws, sl_props_json, sl_clone_data.time_bucket).await;
    match r {
        Err(err) => {
            let err_text = format!("{:?}", err);
            if !err_text.contains("DuplicateCellId") {
                return Err(err);
            }
            Ok(None)
        }
        Ok(cell) => Ok(Some(cell)),
    }
}

pub async fn handle_install_sl_clone(
    app_ws: &mut AppConnection,
    sl_props_json: String,
    time_bucket: u32,
) -> Result<ClonedCell> {
    let payload = CreateCloneCellPayload {
        role_name: "servicelogger".into(),
        modifiers: DnaModifiersOpt::none().with_properties(YamlProperties::new(
            serde_yaml::from_str(&sl_props_json).unwrap(),
        )),
        membrane_proof: None,
        name: Some(sl_clone_name(SlCloneSpec {
            days_in_bucket: SL_BUCKET_SIZE_DAYS,
            time_bucket,
        })),
    };
    app_ws.create_clone(payload).await
}

pub async fn get_app_details(
    core_app_connection: &mut AppConnection,
    happ_id: ActionHash,
) -> Result<PresentedHappBundle> {
    let happ_id_clone = happ_id.clone();
    core_app_connection
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            "hha".into(),
            "get_happ".into(),
            happ_id,
        )
        .await.map_err(|e| {
            anyhow!(
                "Failed to install happ id {:?}.  Supporting call to fetch happ details failed.  Error: {:#?}.", happ_id_clone, e
            )
        })
}

pub async fn is_already_installed(
    admin_connection: &mut hpos_hc_connect::AdminWebsocket,
    installed_app_id: String,
) -> Result<bool> {
    match admin_connection
        .list_apps(Some(AppStatusFilter::Running) )
        .await {
            Ok(apps) => Ok(apps.iter().any(|app| app.installed_app_id == installed_app_id)),
            Err(err) => {
                Err(anyhow!(
                    "Failed to install happ id {:?}.  Supporting call to fetch running happs failed.  Error: {:#?}.", installed_app_id, err
                ))
            }
        }
}

pub fn get_base_dna_hash(cell_map: &CellInfoMap, role_name: RoleName) -> Result<String> {
    let dna = match cell_map.get_key_value(&role_name) {
        Some((_name, cells)) => cells.iter().find_map(|c| match c {
            CellInfo::Provisioned(c) => Some(c.cell_id.dna_hash()),
            _ => None,
        }),
        None => {
            return Err(anyhow!(
                "Failed to install. Unable to locate cell info for {}",
                role_name
            ))
        }
    };

    match dna {
        Some(dna_hash) => {
            let hash_b64: DnaHashB64 = dna_hash.to_owned().into();
            Ok(hash_b64.to_string())
        }
        None => Err(anyhow!(
            "Failed to install. Unable to locate cell info for {}",
            role_name
        )),
    }
}

pub async fn get_host_pub_key(
    maybe_pubkey: Option<AgentPubKey>,
    core_app_connection: &mut AppConnection,
) -> Result<AgentPubKey> {
    if let Some(pub_key) = maybe_pubkey {
        Ok(pub_key)
    } else {
        // NB: The host_pub_key is set to `None` only in a test env.
        // In a test env, we can just use the host agent pubkey from the core app cell_id as the host pubkey
        Ok(core_app_connection.app_info().await?.agent_pub_key)
    }
}

pub fn get_sl_id(happ_id: &String) -> String {
    format!("{}::servicelogger", happ_id)
}

pub fn get_uid_override() -> Option<String> {
    std::env::var("DEV_UID_OVERRIDE").ok()
}

pub fn get_sl_collector_pubkey() -> String {
    std::env::var("SL_COLLECTOR_PUB_KEY")
        .expect("Failed to read SL_COLLECTOR_PUB_KEY. Is it set in env?")
}
