pub mod core_apps;

use anyhow::{anyhow, Context, Result};
use core_apps::Happ;
use core_apps::{HHA_URL, SL_URL};
use ed25519_dalek::Keypair;
use holochain_client::{AdminWebsocket, AppInfo, AppWebsocket, InstallAppPayload, ZomeCall};
use holochain_conductor_api::{CellInfo, ProvisionedCell};
use holochain_env_setup::{
    environment::{setup_environment, Environment},
    holochain::{create_log_dir, create_tmp_dir},
    storage_helpers::download_file,
};
use holochain_types::app::AppManifest;
use holochain_types::dna::{ActionHash, ActionHashB64, DnaHash};
use holochain_types::prelude::{
    holochain_serial, AgentPubKey, AppBundleSource, ExternIO, Nonce256Bits, SerializedBytes,
    Signature, Timestamp, UnsafeBytes, YamlProperties, ZomeCallUnsigned,
};
use holofuel_types::fuel::Fuel;
use hpos_api_rust::consts::{ADMIN_PORT, APP_PORT};
use hpos_config_core::*;
use hpos_config_seed_bundle_explorer::unlock;
use log::{debug, info, trace};
use rocket::serde::json::serde_json;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::time::Duration;
use std::{collections::HashMap, env, fs::File, path::PathBuf, sync::Arc};
use hpos_api_rust::types::{
    ActivityLog, CallSpec, ClientRequest, ExtraWebLogData, HostMetrics, HostResponse,
    RequestPayload,
};
use url::Url;

pub struct Test {
    pub hc_env: Environment,
    pub agent: AgentPubKey,
    pub admin_ws: AdminWebsocket,
    pub app_ws: AppWebsocket,
}
impl Test {
    /// Set up an environment resembling HPOS
    pub async fn init() -> Self {
        const PASSWORD: &str = "pass";

        // Env vars required for runnig stuff that imitates HPOS
        env::set_var("HOLOCHAIN_DEFAULT_PASSWORD", PASSWORD); // required by holochain_env_setup crate
        env::set_var("DEVICE_SEED_DEFAULT_PASSWORD", PASSWORD); // required by holochain_env_setup crate
        let manifets_path = env::var("CARGO_MANIFEST_DIR").unwrap();
        let hpos_config_path = format!("{}/resources/test/hpos-config.json", &manifets_path);
        env::set_var("HPOS_CONFIG_PATH", &hpos_config_path);
        env::set_var(
            "CORE_HAPP_FILE",
            format!("{}/resources/test/config.yaml", &manifets_path),
        );

        // Get device_bundle from hpos-config and pass it to setup_environment so that lair
        // can import a keypar for an agent from hpos-config
        let (agent_string, device_bundle) = from_config(hpos_config_path.into(), PASSWORD.into())
            .await
            .unwrap();

        let bytes = base64::decode_config(&agent_string[1..], base64::URL_SAFE_NO_PAD).unwrap();
        let agent: AgentPubKey = AgentPubKey::from_raw_39(bytes).unwrap();

        info!("agent: {}, bundle: {}", agent, device_bundle);

        let tmp_dir = create_tmp_dir();
        let log_dir = create_log_dir();

        env::set_var("HOLOCHAIN_WORKING_DIR", &tmp_dir);

        // Set up holochain environment
        let hc_env = setup_environment(&tmp_dir, &log_dir, Some(&device_bundle), None)
            .await
            .expect("Error spinning up Holochain environment");

        info!("Started holochain in tmp dir {:?}", &tmp_dir);

        let mut admin_ws = AdminWebsocket::connect(format!("ws://localhost:{}", ADMIN_PORT))
            .await
            .expect("failed to connect to holochain's admin interface");

        let _ = admin_ws
            .attach_app_interface(APP_PORT)
            .await
            .expect("failed to attach app interface");

        let app_ws = AppWebsocket::connect(format!("ws://localhost:{}", APP_PORT))
            .await
            .expect("failed to connect to holochain's app interface");

        Self {
            hc_env,
            agent,
            admin_ws,
            app_ws,
        }
    }

    /// Generate SL activity Payload
    pub async fn generate_sl_payload(&mut self, sl_cell: &ProvisionedCell) -> ActivityLog {
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

        let request_signature: Signature = self
            .call_zome(sl_cell, "service", "sign_request", request.clone())
            .await;

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

    /// Constructs AppBundleSource::Bundle(AppBundle) from scratch for servicelogger
    pub async fn create_servicelogger_source(&mut self, path: PathBuf) -> Result<AppBundleSource> {
        let mut source = AppBundleSource::Path(path);
        use mr_bundle::Bundle;
        let bundle = match source {
            AppBundleSource::Bundle(bundle) => bundle.into_inner(),
            AppBundleSource::Path(path) => Bundle::read_from_file(&path).await.unwrap(),
        };
        let AppManifest::V1(mut manifest) = bundle.manifest().clone();
        let place_holder_dna =
            DnaHash::try_from("uhC0kGNBsMPAi8Amjsa5tEVsRHZWaK-E7Fl8kLvuBvNuYtfuG1gkP").unwrap();
        let place_holder_pubkey =
            AgentPubKey::try_from("uhCAk76ikqpgxdisc5bRJcCY-lOTVB8osHEkiGj8hP4kxA01jSrjC").unwrap();
        let place_holder_happ_id =
            ActionHash::try_from("uhCkkNEufiBrVmH-INOLgb6W2OBpa3v0xTIMilD8PIA4vmRtg8jSy").unwrap();

        for role_manifest in &mut manifest.roles {
            let json = format!(
                r#"{{"bound_happ_id":"{}", "bound_hha_dna":"{}", "bound_hf_dna":"{}", "holo_admin": "{}"}}"#,
                place_holder_happ_id.to_string(),
                place_holder_dna.to_string(),
                place_holder_dna.to_string(),
                place_holder_pubkey
            );
            let properties = Some(YamlProperties::new(serde_yaml::from_str(&json).unwrap()));
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
                let sl_source = self.create_servicelogger_source(happ_path).await.unwrap();
                (Some(format!("{}::servicelogger", id)), sl_source)
            }
            None => (Some(happ.to_string()), AppBundleSource::Path(happ_path)),
        };

        let payload = InstallAppPayload {
            agent_key: self.agent.clone(),
            installed_app_id,
            source,
            membrane_proofs,
            network_seed: None,
            ignore_genesis_failure: false,
        };

        let app_info = self
            .admin_ws
            .install_app(payload)
            .await
            .expect("failed to install happ");

        trace!("{} app_info: {:#?}", happ, &app_info);

        // enable happ
        let _ = self
            .admin_ws
            .enable_app(app_info.installed_app_id.clone())
            .await
            .expect("failed to enable app");

        debug!("AppInfo for newly installed app {}: {:#?}", happ, app_info);

        app_info
    }

    pub async fn call_zome<T: Debug + Serialize, R: Debug + for<'de> Deserialize<'de>>(
        &mut self,
        hha_cell: &ProvisionedCell,
        zome_name: &str,
        fn_name: &str,
        payload: T,
    ) -> R {
        let (nonce, expires_at) = fresh_nonce();

        let zome_call_unsigned = ZomeCallUnsigned {
            provenance: self.agent.clone(),
            cell_id: hha_cell.clone().cell_id,
            zome_name: zome_name.into(),
            fn_name: fn_name.into(),
            cap_secret: None,
            payload: ExternIO::encode(payload).unwrap(),
            nonce,
            expires_at,
        };

        let signed_zome_call =
            ZomeCall::try_from_unsigned_zome_call(&self.hc_env.keystore, zome_call_unsigned)
                .await
                .unwrap();

        let response = self.app_ws.call_zome(signed_zome_call).await.unwrap();

        ExternIO::decode(&response).unwrap()
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
            let Keypair { public, .. } =
                unlock(&device_bundle, Some(password))
                    .await
                    .context(format!(
                        "unable to unlock the device bundle from {}",
                        &config_path.to_string_lossy()
                    ))?;
            Ok((
                public_key::to_holochain_encoded_agent_key(&public),
                device_bundle,
            ))
        }
        _ => Err(anyhow!("Unsupported version of hpos config")),
    }
}

/// generates nonce for zome calls
/// https://github.com/Holo-Host/hpos-service-crates/blob/e1d1baa0c9741a46a29626ce33d9e019c1391db8/crates/hpos_connect_hc/src/utils.rs#L13
fn fresh_nonce() -> (Nonce256Bits, Timestamp) {
    let mut bytes = [0; 32];
    getrandom::getrandom(&mut bytes).unwrap();
    let nonce = Nonce256Bits::from(bytes);
    // Rather arbitrary but we expire nonces after 5 mins.
    let expires: Timestamp = (Timestamp::now() + Duration::from_secs(60 * 5)).unwrap();
    (nonce, expires)
}

/// Extract destructively cell from AppInfo
pub fn to_cell(mut hha_app_info: AppInfo, role_name: &str) -> ProvisionedCell {
    let mut a = hha_app_info.cell_info.remove(role_name).unwrap();
    match a.pop() {
        Some(CellInfo::Provisioned(hha_cell)) => hha_cell,
        _ => panic!("Couldn't find cell for hha"),
    }
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes, Clone, Default)]
pub struct HappInput {
    pub hosted_urls: Vec<String>,
    pub bundle_url: String,
    pub ui_src_url: Option<String>,
    pub special_installed_app_id: Option<String>,
    pub name: String,
    pub logo_url: Option<String>,
    pub dnas: Vec<DnaResource>,
    pub description: String,
    pub categories: Vec<String>,
    pub jurisdictions: Vec<String>,
    pub exclude_jurisdictions: bool,
    pub publisher_pricing_pref: PublisherPricingPref,
    pub login_config: LoginConfig,
    pub uid: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes, Clone)]
pub struct PublisherPricingPref {
    pub cpu: Fuel,
    pub storage: Fuel,
    pub bandwidth: Fuel,
}
impl Default for PublisherPricingPref {
    fn default() -> Self {
        PublisherPricingPref {
            cpu: Fuel::new(0),
            storage: Fuel::new(0),
            bandwidth: Fuel::new(0),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes, Clone, Default)]
pub struct LoginConfig {
    pub display_publisher_name: bool,
    pub registration_info_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes, Clone)]
pub struct DnaResource {
    pub hash: String,
    pub src_url: String,
    pub nick: String,
}
