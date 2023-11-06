use std::{env, path::PathBuf, fs::File};
use hpos_config_core::*;
use hpos_config_seed_bundle_explorer::unlock;
use holochain_env_setup::{
    environment::{setup_environment, Environment},
    holochain::{create_log_dir, create_tmp_dir},
    // storage_helpers::download_file,
};
use holochain_types::prelude::AgentPubKey;
use holochain_client::{AdminWebsocket, AppWebsocket};
use hpos_api_rust::consts::{ADMIN_PORT, APP_PORT};
use anyhow::{anyhow, Context, Result};
use rocket::serde::json::serde_json;
use ed25519_dalek::Keypair;

pub struct Test {
    pub hc_env: Environment,
    //pub agent: HoloHash<Agent>,
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

        let bytes = base64::decode_config(&agent_string, base64::URL_SAFE_NO_PAD).unwrap();

        let agent: AgentPubKey = AgentPubKey::from_raw_39(bytes).unwrap();

        println!("in: {}, out: {}", &agent_string, agent);

        //println!("agent: {}, bundle: {}", agent_string, device_bundle);

        let tmp_dir = create_tmp_dir();
        let log_dir = create_log_dir();

        // Set up holochain environment
        let hc_env = setup_environment(&tmp_dir, &log_dir, Some(&device_bundle), None)
            .await
            .expect("Error spinning up Holochain environment");

        println!("Started holochain in tmp dir {:?}", &tmp_dir);

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
            // agent,
            admin_ws,
            app_ws,
        }
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
