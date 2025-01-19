use crate::common::types::PresentedHappBundle;
use anyhow::{anyhow, Result};
use holochain_client::{AdminResponse, InstalledAppId};
use holochain_client::{AgentPubKey, AppInfo};
use holochain_conductor_api::{AppStatusFilter, CellInfo};
use holochain_types::app::{AppManifest, InstallAppPayload, RoleSettings, RoleSettingsMap};
use holochain_types::dna::{ActionHash, DnaHashB64};
use holochain_types::prelude::{AppBundleSource, RoleName, YamlProperties};
use hpos_hc_connect::app_connection::CoreAppRoleName;
use hpos_hc_connect::AppConnection;
use mr_bundle::Bundle;
use std::collections::HashMap;

use super::types::{CellInfoMap, RawInstallAppPayload, SuccessfulInstallResult};

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

    let roles_settings: RoleSettingsMap = payload.membrane_proofs
        .into_iter()
        .map(|(role_name, serialized)| {
            // Convert SerializedBytes into MembraneProof.
            let membrane_proof = serialized;
            
            (
                role_name,
                RoleSettings::Provisioned {
                    membrane_proof: Some(membrane_proof),
                    modifiers: None,
                },
            )
        })
        .collect();

    let p = InstallAppPayload {
        ignore_genesis_failure: false,
        source: payload.source,
        agent_key: Some(payload.agent_key),
        installed_app_id: Some(payload.installed_app_id),
        roles_settings: Some(roles_settings),
        network_seed: if payload.uid.is_some() {
            match &uid_override {
                Some(uid) => Some(format!("{}::{}", payload.uid.unwrap(), uid)),
                None => Some(payload.uid.unwrap()),
            }
        } else {
            uid_override
        },
        // network_seed: payload.uid,
        allow_throwaway_random_agent_key: false,
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

pub async fn install_assigned_sl_instance(
    admin_connection: &mut hpos_hc_connect::AdminWebsocket,
    happ_id: &String,
    host_pub_key: AgentPubKey,
    core_happ_cell_info: &CellInfoMap,
    sl_path_source: AppBundleSource,
) -> Result<SuccessfulInstallResult> {
    log::debug!(
        "Starting installation process of servicelogger for hosted happ: {:?}",
        happ_id
    );

    let sl_props_json = format!(
        r#"{{"bound_happ_id":"{}", "bound_hha_dna":"{}", "bound_hf_dna":"{}", "holo_admin": "{}"}}"#,
        happ_id,
        get_base_dna_hash(core_happ_cell_info, CoreAppRoleName::HHA.into())?,
        get_base_dna_hash(core_happ_cell_info, CoreAppRoleName::Holofuel.into())?,
        get_sl_collector_pubkey()
    );

    let sl_source = update_happ_bundle(sl_path_source, sl_props_json)
        .await
        .unwrap();

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

    handle_install_app_raw(admin_connection, sl_install_payload).await
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
