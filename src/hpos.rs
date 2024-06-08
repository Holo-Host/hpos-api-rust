use crate::common::consts::ADMIN_PORT;
use anyhow::{anyhow, Context, Result};
use holochain_keystore::MetaLairClient;
use hpos_hc_connect::{
    holo_config::{self, HappsFile},
    AdminWebsocket,
};
use rocket::tokio::sync::Mutex;

/// Mutex that guards access to admin websocket and lair keystore connection. This Mutex also stores
/// information about app interfaces enabled in holochain that websocket zome calls to specific apps can open.
pub type WsMutex = Mutex<Ws>;

/// Connects to Holochain using env vars that are specific for a flavour of a network (devNet, mainNet, etc)
/// Env vars required:
/// - CORE_HAPP_FILE
/// - HOLOCHAIN_DEFAULT_PASSWORD
/// - HOLOCHAIN_WORKING_DIR
/// - DEV_UID_OVERRIDE

/// Opens a single admin websocket connection to holochain using pre-initiated keystore
pub struct Ws {
    pub admin: AdminWebsocket,
    keystore: MetaLairClient,
    // HashMap of open interfaces that has to be populated at start and maintained over runtime of this binary
    pub core_app_id: String,
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

        Ok(Mutex::new(Self {
            admin,
            keystore,
            core_app_id,
        }))
    }
}
