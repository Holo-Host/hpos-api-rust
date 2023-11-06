use std::env;

use holochain_env_setup::{
    environment::{setup_environment, Environment},
    holochain::{create_log_dir, create_tmp_dir},
    storage_helpers::download_file,
};
use holochain_types::prelude::{HoloHash, hash_type::Agent};
use holochain_client::{AdminWebsocket, AppInfo, AppWebsocket, ZomeCall};
use log::info;
use hpos_api_rust::consts::{ADMIN_PORT, APP_PORT};

pub struct Test {
    pub hc_env: Environment,
    pub agent: HoloHash<Agent>,
    pub admin_ws: AdminWebsocket,
    pub app_ws: AppWebsocket,
}
impl Test {
    /// Set up an environment resembling HPOS
    pub async fn init() -> Self {
        // Start Holochain and Lair
        env::set_var("HOLOCHAIN_DEFAULT_PASSWORD", "pass"); // required by holochain_env_setup crate
        env::set_var("DEVICE_SEED_DEFAULT_PASSWORD", "pass"); // required by holochain_env_setup crate

        // Get device_bundle from hpos-config and pass it to setup_environment so that lair
        // can import a keypar for an agent from hpos-config

        let device_bundle = "abba";

        let tmp_dir = create_tmp_dir();
        let log_dir = create_log_dir();

        // Set up holochain environment
        let hc_env = setup_environment(&tmp_dir, &log_dir, Some(device_bundle), None)
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
}
