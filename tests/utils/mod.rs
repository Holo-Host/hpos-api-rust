pub mod core_apps;

use anyhow::{anyhow, Context, Result};
use core_apps::Happ;
use core_apps::{HHA_URL, SL_URL};
use holochain_client::{AppInfo, InstallAppPayload};
use holochain_conductor_api::AdminResponse;
use holochain_env_setup::{
    environment::{setup_environment, Environment},
    holochain::{create_log_dir, create_tmp_dir},
    storage_helpers::download_file,
};
use holochain_types::app::{RoleSettings, RoleSettingsMap};
use holochain_types::dna::{ActionHash, ActionHashB64, DnaHash};
use holochain_types::prelude::{
    AgentPubKey, AppBundleSource, SerializedBytes, Signature, Timestamp, UnsafeBytes,
};
use hpos_api_rust::common::consts::ADMIN_PORT;
use hpos_api_rust::handlers::hosted_happs::{
    ActivityLog, CallSpec, ClientRequest, ExtraWebLogData, HostMetrics, HostResponse,
    RequestPayload,
};
use hpos_api_rust::handlers::install;

use hpos_api_rust::common::types::{HappAndHost, HappInput, PresentedHappBundle};
use hpos_config_core::*;
use hpos_config_seed_bundle_explorer::unlock;
use hpos_hc_connect::app_connection::CoreAppRoleName;
use hpos_hc_connect::hha_agent::CoreAppAgent;
use hpos_hc_connect::AdminWebsocket;
use hpos_hc_connect::AppConnection;
use log::{debug, info, trace};
use rocket::serde::json::serde_json;
use std::{collections::HashMap, env, fs::File, path::PathBuf, sync::Arc};
use url::Url;

pub struct Test {
    pub hc_env: Environment,
    pub agent: AgentPubKey,
    pub admin_ws: AdminWebsocket,
}
impl Test {
    /// Set up an environment resembling HPOS
    pub async fn init() -> Self {
        const PASSWORD: &str = "pass";

        // Env vars required for running stuff that imitates HPOS
        env::set_var("HOLOCHAIN_DEFAULT_PASSWORD", PASSWORD); // required by holochain_env_setup crate
        env::set_var("DEVICE_SEED_DEFAULT_PASSWORD", PASSWORD); // required by holochain_env_setup crate
        let manifets_path = env::var("CARGO_MANIFEST_DIR").unwrap();
        let hpos_config_path = format!("{}/resources/test/hpos-config.json", &manifets_path);
        env::set_var("HPOS_CONFIG_PATH", &hpos_config_path);
        env::set_var(
            "CORE_HAPP_FILE",
            format!("{}/resources/test/config.yaml", &manifets_path),
        );
        env::set_var(
            "SL_COLLECTOR_PUB_KEY",
            "uhCAk0vTcqgNCoZLnUVuozGzi6onFdi8jfR6CeUw8pNGd4-2Ht0tA", // dev env collector pub key
        );
        env::set_var("IS_TEST_ENV", "true");

        const HBS_BASE_PATH: &str = "https://hbs.dev.holotest.net";
        env::set_var("HBS_URL", HBS_BASE_PATH);

        // Get device_bundle from hpos-config and pass it to setup_environment so that lair
        // can import a keypar for an agent from hpos-config
        let (agent_string, device_bundle) = from_config(hpos_config_path.into(), PASSWORD.into())
            .await
            .unwrap();

        let bytes = base64::decode_config(&agent_string[1..], base64::URL_SAFE_NO_PAD).unwrap();
        let agent: AgentPubKey = AgentPubKey::from_raw_39(bytes).unwrap();

        info!("agent: {}, bundle: {}", agent, device_bundle);

        let tmp_dir = create_tmp_dir();
        let lair_dir = tmp_dir.join("lair-keystore");
        let log_dir = create_log_dir();

        env::set_var("LAIR_WORKING_DIR", &lair_dir);

        // Set up holochain environment
        let hc_env = setup_environment(&tmp_dir, &log_dir, Some(&device_bundle), None)
            .await
            .expect("Error spinning up Holochain environment");

        info!("Started holochain in tmp dir {:?}", &tmp_dir);

        let admin_ws = AdminWebsocket::connect(ADMIN_PORT)
            .await
            .expect("failed to connect to holochain's admin interface");

        Self {
            hc_env,
            agent,
            admin_ws,
        }
    }

    /// Generate SL activity Payload
    pub async fn generate_sl_payload(&mut self, ws: &mut AppConnection) -> ActivityLog {
        use rand::seq::SliceRandom;

        let hosts = vec![
            "host1", "host2", "host3", "host4", "host5", "host6", "host7", "host8", "host9",
        ];
        let ips = vec![
            "IP1", "IP2", "IP3", "IP4", "IP5", "IP6", "IP7", "IP8", "IP9",
        ];

        let fake_action_hash: ActionHash = ActionHash::try_from(
            "uhCkkMpS5xUbci4IiBXpmlFCAJF3unOq-ZBkMrbJTsuiieTllOLtY".to_string(),
        )
        .unwrap();

        // create signature of a call spec
        let request = RequestPayload {
            host_id: hosts.choose(&mut rand::thread_rng()).unwrap().to_string(),
            timestamp: Timestamp::now(),
            hha_pricing_pref: fake_action_hash.clone(),
            call_spec: CallSpec {
                args_hash: vec![0 as u8; 10],
                function: "function".to_string(),
                zome: "zome".to_string(),
                role_name: "role_name".to_string(),
                hha_hash: fake_action_hash,
            },
        };

        //let sl_websocket

        let request_signature: Signature = ws
            .zome_call_typed(
                "servicelogger".into(),
                "service".into(),
                "sign_request".into(),
                request.clone(),
            )
            .await
            .unwrap();

        ActivityLog {
            request: ClientRequest {
                agent_id: self.agent.clone(),
                request,
                request_signature,
            },
            response: HostResponse {
                host_metrics: HostMetrics {
                    cpu: 12,
                    bandwidth: 12,
                },
                weblog_compat: ExtraWebLogData {
                    source_ip: ips.choose(&mut rand::thread_rng()).unwrap().to_string(),
                    status_code: 200,
                },
            },
        }
    }

    pub async fn install_app(&mut self, happ: Happ, happ_id: Option<ActionHashB64>) -> AppInfo {
        let url = match happ {
            Happ::HHA => HHA_URL,
            Happ::SL => SL_URL,
        };

        // Install happ with host_agent key
        let mut membrane_proofs = HashMap::new();
        membrane_proofs.insert(
            happ.to_string(),
            Arc::new(SerializedBytes::from(UnsafeBytes::from(vec![0]))),
        );
        let happ_path =
            download_file(&Url::parse(url).expect(&format!("failed to parse {}", stringify!(url))))
                .await
                .expect("failed to download happ bundle");

        let (installed_app_id, source) = match happ_id {
            Some(id) => {
                let place_holder_dna =
                    DnaHash::try_from("uhC0kGNBsMPAi8Amjsa5tEVsRHZWaK-E7Fl8kLvuBvNuYtfuG1gkP")
                        .unwrap();
                let place_holder_pubkey =
                    AgentPubKey::try_from("uhCAk76ikqpgxdisc5bRJcCY-lOTVB8osHEkiGj8hP4kxA01jSrjC")
                        .unwrap();
                let place_holder_happ_id =
                    ActionHash::try_from("uhCkkNEufiBrVmH-INOLgb6W2OBpa3v0xTIMilD8PIA4vmRtg8jSy")
                        .unwrap();

                let sl_props_json = format!(
                    r#"{{"bound_happ_id":"{}", "bound_hha_dna":"{}", "bound_hf_dna":"{}", "holo_admin": "{}"}}"#,
                    place_holder_happ_id.to_string(),
                    place_holder_dna.to_string(),
                    place_holder_dna.to_string(),
                    place_holder_pubkey
                );

                // Constructs AppBundleSource::Bundle(AppBundle) from scratch for servicelogger
                let sl_source =
                    install::update_happ_bundle(AppBundleSource::Path(happ_path), sl_props_json)
                        .await
                        .unwrap();

                (Some(format!("{}::servicelogger", id)), sl_source)
            }
            None => (Some(happ.to_string()), AppBundleSource::Path(happ_path)),
        };

        let roles_settings: RoleSettingsMap = membrane_proofs
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

        let payload = InstallAppPayload {
            agent_key: Some(self.agent.clone()),
            installed_app_id: installed_app_id.clone(),
            source,
            roles_settings: Some(roles_settings),
            network_seed: None,
            ignore_genesis_failure: false,
            allow_throwaway_random_agent_key: false,
        };

        let app_info = if let AdminResponse::AppInstalled(app_info) = self
            .admin_ws
            .install_app(payload)
            .await
            .expect("failed to install happ")
        {
            trace!("{} app_info: {:#?}", happ, &app_info);
            app_info
        } else {
            panic!("Failed to install happ with id {:?}", installed_app_id);
        };

        // enable happ
        let _ = self
            .admin_ws
            .enable_app(&app_info.installed_app_id.clone())
            .await
            .expect("failed to enable app");

        debug!("AppInfo for newly installed app {}: {:#?}", happ, app_info);

        app_info
    }
}

async fn from_config(config_path: PathBuf, password: String) -> Result<(String, String)> {
    let config_file = File::open(&config_path).context(format!(
        "failed to open file {}",
        &config_path.to_string_lossy()
    ))?;
    match serde_json::from_reader(config_file)? {
        Config::V2 { device_bundle, .. } => {
            // take in password
            let public = unlock(&device_bundle, Some(password))
                .await
                .context(format!(
                    "unable to unlock the device bundle from {}",
                    &config_path.to_string_lossy()
                ))?
                .verifying_key();
            Ok((
                public_key::to_holochain_encoded_agent_key(&public),
                device_bundle,
            ))
        }
        _ => Err(anyhow!("Unsupported version of hpos config")),
    }
}

pub async fn publish_and_enable_hosted_happ(
    hha: &mut CoreAppAgent,
    payload: HappInput,
) -> Result<ActionHashB64> {
    // howto: https://github.com/Holo-Host/holo-hosting-app-rsm/blob/develop/tests/unit-test/provider-init.ts#L52
    let draft_hha_bundle: PresentedHappBundle = hha
        .app
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            "hha".into(),
            "create_draft".into(),
            payload,
        )
        .await?;

    let payload = draft_hha_bundle.id;
    let hha_bundle: PresentedHappBundle = hha
        .app
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            "hha".into(),
            "publish_happ".into(),
            payload,
        )
        .await?;

    let test_hosted_happ_id = hha_bundle.id;
    info!(
        "Published hosted happ in hha with id {}",
        &test_hosted_happ_id
    );

    // enable test happ in hha
    let payload = HappAndHost {
        happ_id: test_hosted_happ_id.clone(),
        holoport_id: "5z1bbcrtjrcgzfm26xgwivrggdx1d02tqe88aj8pj9pva8l9hq".to_string(),
    };

    debug!("payload: {:?}", payload);
    let _: () = hha
        .app
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            "hha".into(),
            "enable_happ".into(),
            payload,
        )
        .await?;

    info!("Hosted happ enabled in hha - OK");

    Ok(test_hosted_happ_id)
}
