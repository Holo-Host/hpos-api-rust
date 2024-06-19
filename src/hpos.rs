use std::collections::HashMap;

use crate::common::consts::ADMIN_PORT;
use anyhow::{anyhow, Context, Result};
use holochain_client::AgentPubKey;
use holochain_keystore::MetaLairClient;
use hpos_hc_connect::{
    holo_config::{self, Happ, HappsFile},
    AdminWebsocket, AppConnection,
};
use rocket::tokio::sync::Mutex;
use std::process::{Command, Stdio};
use std::{env, path::PathBuf};

/// Mutex that guards access to admin websocket and lair keystore connection. This Mutex also stores
/// information about app interfaces enabled in holochain that websocket zome calls to specific apps can open.
pub type WsMutex = Mutex<Ws>;

/// Connects to Holochain using env vars that are specific for a flavour of a network (devNet, mainNet, etc)
/// Env vars required:
/// - CORE_HAPP_FILE
/// - HOLOCHAIN_DEFAULT_PASSWORD
/// - HOLOCHAIN_WORKING_DIR
/// - DEV_UID_OVERRIDE
/// - SL_COLLECTOR_PUB_KEY
/// - HOST_PUBKEY_PATH (only required in non-test envs)
/// - IS_TEST_ENV (only required in a test env)

/// Opens a single admin websocket connection to holochain using pre-initiated keystore
pub struct Ws {
    pub admin: AdminWebsocket,
    keystore: MetaLairClient,
    pub apps: HashMap<String, AppConnection>,
    pub core_app_id: String,
    pub base_sl: Happ,
    pub hp_id: String,
    pub host_pub_key: Option<AgentPubKey>,
}

impl Ws {
    pub async fn connect() -> Result<Mutex<Self>> {
        let admin = AdminWebsocket::connect(ADMIN_PORT)
            .await
            .context("failed to connect to holochain's app interface")?;

        let passphrase =
            sodoken::BufRead::from(holo_config::default_password()?.as_bytes().to_vec());
        let keystore = holochain_keystore::lair_keystore::spawn_lair_keystore(
            url2::url2!("{}", holo_config::get_lair_url(None)?),
            passphrase,
        )
        .await?;

        let app_file = HappsFile::load_happ_file_from_env(None)?;
        let core_app_id = app_file
            .core_happs
            .iter()
            .find(|x| x.id().contains("core-app"))
            .ok_or(anyhow!("Could not find a core-app in HPOS file"))?
            .id();

        let base_sl = app_file
            .core_happs
            .iter()
            .find(|x| x.id().contains("servicelogger"))
            .ok_or(anyhow!("Could not find a servicelogger in HPOS file"))?
            .to_owned();

        let hp_id = get_holoport_id();

        let host_pub_key = get_host_pubkey()?;

        let apps = HashMap::new();

        Ok(Mutex::new(Self {
            admin,
            keystore,
            apps,
            core_app_id,
            base_sl,
            hp_id,
            host_pub_key,
        }))
    }

    async fn open_connection(&mut self, app_id: String) -> Result<AppConnection> {
        let app_ws = AppConnection::connect(&mut self.admin, self.keystore.clone(), app_id).await?;

        // Not really because it returns mutable reference
        Ok(app_ws)
    }

    pub async fn get_connection(&mut self, app_id: String) -> Result<&mut AppConnection> {
        if self.apps.contains_key(&app_id) {
            // I can unwrap here because I have just checked if queried key existed
            return Ok(self.apps.get_mut(&app_id).unwrap());
        } else {
            let connection = self.open_connection(app_id.clone()).await?;
            self.apps.insert(app_id.clone(), connection);
            // I can unwrap here because I just inserted queried key
            Ok(self.apps.get_mut(&app_id).unwrap())
        }
    }
}

pub fn get_host_pubkey() -> Result<Option<AgentPubKey>> {
    let host_pub_key_path: PathBuf = match env::var("HOST_PUBKEY_PATH") {
        Ok(p) => p.into(),
        Err(_) => {
            if std::env::var("IS_TEST_ENV").is_ok() {
                return Ok(None);
            }
            return Err(anyhow!(
                "Failed to read the HOST_PUBKEY_PATH. Is it set in env?"
            ));
        }
    };

    let file = std::fs::File::open(host_pub_key_path)?;
    let host_pub_key: AgentPubKey = rocket::serde::json::serde_json::from_reader(file)?;

    Ok(Some(host_pub_key))
}

pub fn get_holoport_id() -> String {
    if std::env::var("IS_TEST_ENV").is_ok() {
        return "W3cPOJ9Em4vR3A4jlLwD7n++wqk3rNP3Rk59UHxjPI7rAZ8HKmlQQdFHuUB5XfnSw2eMgV+JbiK7fV5VEYaSGQ==".to_string();
    }

    let device_seed_password = std::env::var("DEVICE_SEED_DEFAULT_PASSWORD")
        .expect("Failed to read DEVICE_SEED_DEFAULT_PASSWORD. Is it set in env?");

    // command: `hpos-config-into-base36-id --config-path /run/hpos-init/hp-*.json --password ${DEVICE_SEED_DEFAULT_PASSWORD}`
    let command = Command::new("hpos-config-into-base36-id")
        .arg("--config-path")
        .arg("/run/hpos-init/hp-*.json")
        .arg("--password")
        .arg(device_seed_password)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to execute `hpos-config-into-base36-id` command");

    let output = command
        .wait_with_output()
        .expect("Failed to wait on `hpos-config-into-base36-id` command");

    let hp_id = String::from_utf8(output.stdout)
        .expect("Output for `hpos-config-into-base36-id` was not valid utf-8");

    hp_id.trim().to_string()
}
