use std::{env, path::PathBuf, fs::File, sync::Arc, collections::HashMap, fmt};
use holochain_conductor_api::{CellInfo, ProvisionedCell};
use hpos_config_core::*;
use hpos_config_seed_bundle_explorer::unlock;
use holochain_env_setup::{
    environment::{setup_environment, Environment},
    holochain::{create_log_dir, create_tmp_dir},
    storage_helpers::download_file,
};
use holochain_types::{prelude::{AgentPubKey, ExternIO, holochain_serial, ZomeCallUnsigned, SerializedBytes, Timestamp, UnsafeBytes, AppBundleSource, Nonce256Bits}, dna::{ActionHashB64, AgentPubKeyB64}};
use holochain_client::{AdminWebsocket, AppWebsocket, ZomeCall, AppInfo, InstallAppPayload};
use holofuel_types::fuel::Fuel;
use hpos_api_rust::consts::{ADMIN_PORT, APP_PORT};
use anyhow::{anyhow, Context, Result};
use rocket::serde::json::serde_json;
use ed25519_dalek::Keypair;
use log::{debug, info, trace};
use serde::{Serialize, Deserialize};
use url::Url;
use std::time::Duration;
use std::fmt::Debug;

// https://github.com/Holo-Host/holo-nixpkgs/blob/develop/profiles/logical/happ-releases.nix#L9C5-L9C5
pub const HHA_URL: &str = "https://holo-host.github.io/holo-hosting-app-rsm/releases/downloads/core-app/0_5_13/core-app.0_5_13-skip-proof.happ";
pub const SL_URL: &str = "https://holo-host.github.io/servicelogger-rsm/releases/downloads/0_4_18/servicelogger.0_4_18.happ";


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
        env::set_var("CORE_HAPP_FILE", "TODO: ");
        env::set_var("DEV_UID_OVERRIDE", "123456789");
        let path = env::var("CARGO_MANIFEST_DIR").unwrap();
        let hpos_config_path = format!("{}/resources/test/hpos-config.json", path);
        env::set_var("HPOS_CONFIG_PATH", &hpos_config_path);

        // Get device_bundle from hpos-config and pass it to setup_environment so that lair
        // can import a keypar for an agent from hpos-config
        let (agent_string, device_bundle) = from_config(hpos_config_path.into(), PASSWORD.into()).await.unwrap();

        let bytes = base64::decode_config(&agent_string[1..], base64::URL_SAFE_NO_PAD).unwrap();
        let agent: AgentPubKey = AgentPubKey::from_raw_39(bytes).unwrap();

        info!("agent: {}, bundle: {}", agent, device_bundle);

        let tmp_dir = create_tmp_dir();
        let log_dir = create_log_dir();

        // Set up holochain environment
        let hc_env = setup_environment(&tmp_dir, &log_dir, Some(&device_bundle), None)
            .await
            .expect("Error spinning up Holochain environment");

        info!("Started holochain in tmp dir {:?}", &tmp_dir);

        let mut admin_ws = AdminWebsocket::connect(format!("ws://localhost:{}", ADMIN_PORT))
            .await
            .expect("failed to connect to holochain's admin interface");

        let _ = admin_ws.attach_app_interface(APP_PORT).await;

        let app_ws = AppWebsocket::connect(format!("ws://localhost:{}", APP_PORT))
            .await
            .expect("failed to connect to holochain's app interface");

        // Now install SL and core-app and activte them

        Self {
            hc_env,
            agent,
            admin_ws,
            app_ws,
        }
    }

    pub async fn install_app(&mut self, happ: Happ) -> AppInfo {
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
        let hha_path =
            download_file(&Url::parse(url).expect(&format!("failed to parse {}", stringify!(url))))
                .await
                .expect("failed to download happ bundle");

        let payload = InstallAppPayload {
            agent_key: self.agent.clone(),
            installed_app_id: Some(happ.to_string()),
            source: AppBundleSource::Path(hha_path),
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
    let config_file = File::open(&config_path).context(format!("failed to open file {}", &config_path.to_string_lossy()))?;
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
            Ok((public_key::to_holochain_encoded_agent_key(&public), device_bundle))
        },
        _ => Err(anyhow!("Unsupported version of hpos config"))
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

pub enum Happ {
    HHA,
    SL,
}
impl fmt::Display for Happ {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Happ::HHA => write!(f, "hha"),
            Happ::SL => write!(f, "servicelogger"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes)]
pub struct PresentedHappBundle {
    pub id: ActionHashB64,
    pub provider_pubkey: AgentPubKeyB64,
    pub is_draft: bool,
    pub is_paused: bool,
    pub uid: Option<String>,
    pub bundle_url: String,
    pub ui_src_url: Option<String>,
    pub dnas: Vec<DnaResource>,
    pub hosted_urls: Vec<String>,
    pub name: String,
    pub logo_url: Option<String>,
    pub description: String,
    pub categories: Vec<String>,
    pub jurisdictions: Vec<String>,
    pub exclude_jurisdictions: bool,
    pub login_config: LoginConfig,
    pub special_installed_app_id: Option<String>,
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
    pub hash: String, // hash of the dna, not a stored dht address
    pub src_url: String,
    pub nick: String,
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes, Clone)]
pub struct HappAndHost {
    pub happ_id: ActionHashB64,
    pub holoport_id: String,
}
