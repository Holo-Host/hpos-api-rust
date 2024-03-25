use crate::consts::{ADMIN_PORT, APP_PORT};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use core::fmt::Debug;
use holochain_client::{
    AdminWebsocket, AgentPubKey, AppInfo, AppWebsocket, InstalledAppId, ZomeCall,
};
use holochain_conductor_api::{CellInfo, ProvisionedCell};
use holochain_keystore::MetaLairClient;
use holochain_types::prelude::{ExternIO, ZomeCallUnsigned};
use hpos_hc_connect::{
    holo_config::{self, HappsFile},
    utils::fresh_nonce,
};
use rocket::tokio::sync::Mutex;
use serde::{Deserialize, Serialize};

/// Mutex that guards access to mutable websocket. The down side of such an approach is that
/// a call to one endpoint will block other endpoints, but in our use case it is not an issue.
/// Other option would be to create a pool of websocket connections (but it's too time consuming)
/// or to pass only a keystore inside State between threads and open a separate websocket
/// connection for each thread at a call time.
/// I decided to go with Mutex :-)
pub type WsMutex = Mutex<Ws>;

/// Opens a single websocket connection to holochain using pre-initiated keystore
pub struct Ws {
    app: AppWebsocket,
    pub admin: AdminWebsocket,
    keystore: MetaLairClient,
    pub core_app_id: String,
    last_connect_check: i64,
}

impl Ws {
    pub async fn connect(k: &Keystore) -> Result<Self> {
        let app = AppWebsocket::connect(format!("ws://localhost:{}/", APP_PORT))
            .await
            .context("failed to connect to holochain's app interface")?;
        let admin = AdminWebsocket::connect(format!("ws://localhost:{}/", ADMIN_PORT))
            .await
            .context("failed to connect to holochain's app interface")?;

        Ok(Self {
            app,
            admin,
            keystore: k.keystore.clone(),
            core_app_id: k.core_app_id.clone(),
            last_connect_check: Utc::now().timestamp(),
        })
    }

    /// get cell details of the hha agent
    pub async fn get_cell(
        &mut self,
        app_id: InstalledAppId,
        role_name: &'static str,
    ) -> Result<(ProvisionedCell, AgentPubKey)> {
        match self
            .app
            .app_info(app_id)
            .await
            .map_err(|err| anyhow!("{:?}", err))?
        {
            Some(AppInfo {
                cell_info,
                agent_pub_key,
                ..
            }) => {
                let cell = match &cell_info.get(role_name).unwrap()[0] {
                    // [0] because first one in a Vec is Provisioned cell
                    CellInfo::Provisioned(c) => c.clone(),
                    _ => return Err(anyhow!("unable to find {}", role_name)),
                };
                Ok((cell, agent_pub_key))
            }
            _ => Err(anyhow!("{} is not installed", role_name)),
        }
    }

    /// make a zome call to a running app and decode to <T> type
    pub async fn call_zome<T: Debug + Serialize, R: Debug + for<'de> Deserialize<'de>>(
        &mut self,
        app_id: InstalledAppId,
        role_name: &'static str,
        zome_name: &'static str,
        fn_name: &'static str,
        payload: T,
    ) -> Result<R> {
        ExternIO::decode(
            &self
                .call_zome_raw::<T>(app_id, role_name, zome_name, fn_name, payload)
                .await?,
        )
        .map_err(|err| anyhow!("{:?}", err))
    }

    /// make a zome call to a running app and return ExterIO bytes
    pub async fn call_zome_raw<T: Debug + Serialize>(
        &mut self,
        app_id: InstalledAppId,
        role_name: &'static str,
        zome_name: &'static str,
        fn_name: &'static str,
        payload: T,
    ) -> Result<ExternIO> {
        self.verify_and_reconnect().await;
        let (cell, agent_pubkey) = self.get_cell(app_id, role_name).await?;
        let (nonce, expires_at) = fresh_nonce()?;
        let zome_call_unsigned = ZomeCallUnsigned {
            cell_id: cell.cell_id,
            zome_name: zome_name.into(),
            fn_name: fn_name.into(),
            payload: ExternIO::encode(payload).map_err(|err| anyhow!("{:?}", err))?,
            cap_secret: None,
            provenance: agent_pubkey,
            nonce,
            expires_at,
        };
        let signed_zome_call =
            ZomeCall::try_from_unsigned_zome_call(&self.keystore, zome_call_unsigned).await?;

        let response = self
            .app
            .call_zome(signed_zome_call)
            .await
            .map_err(|err| anyhow!("{:?}", err))?;

        log::debug!("zome call response raw bytes: {:?}", &response);

        Ok(response)
    }

    // checks if the web socket connections are still alive
    // uses app_list and app_info functions
    // if app_list returns a len of zero then app_info will be skiped
    pub async fn is_connected(&mut self) -> Result<bool> {
        let app_list_result = self.admin.list_apps(None).await;

        match app_list_result {
            Ok(_) => match self.app.app_info(self.core_app_id.to_string()).await {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            },
            Err(_) => Ok(false),
        }
    }

    // try to reconnect with websocket connection
    pub async fn reconnect(&mut self, max_retries: u32) {
        let is_connected: bool;
        let mut retries = 0;

        loop {
            if retries >= max_retries {
                is_connected = false;
                break;
            }

            log::warn!(
                "attempting to reconnect: {}/{} attempt",
                retries + 1,
                max_retries
            );
            // create a new connection
            let keystore = Keystore::init().await.unwrap();
            let new_ws = match Ws::connect(&keystore).await {
                Ok(result) => result,
                Err(_) => {
                    retries += 1;
                    continue;
                }
            };
            // if connected then replace the current values with the new connection
            self.keystore = new_ws.keystore;
            self.app = new_ws.app;
            self.admin = new_ws.admin;
            is_connected = true;
            break;
        }
        if !is_connected {
            log::error!(
                "failed to establish websocket connection after {} retries",
                max_retries
            );
        }
        log::info!("successfully reconnected!");
    }

    // verify connection is alive, if not attempt to reconnect
    pub async fn verify_and_reconnect(&mut self) {
        let max_reconnect_tries = 5;
        let check_duration = 60; // seconds
        if Utc::now().timestamp() <= self.last_connect_check + check_duration {
            return;
        }

        self.last_connect_check = Utc::now().timestamp();
        let is_connected = self.is_connected().await.unwrap_or_default();

        if !is_connected {
            log::warn!("connection dropped with websocket, attempting to reconnect");
            self.reconnect(max_reconnect_tries).await;
        }
    }
}

/// Connects to Holochain using env vars that are specific for a flavour of a network (devNet, mainNet, etc)
/// Env vars required:
/// - CORE_HAPP_FILE
/// - HOLOCHAIN_DEFAULT_PASSWORD
/// - HOLOCHAIN_WORKING_DIR
/// - DEV_UID_OVERRIDE
pub struct Keystore {
    keystore: MetaLairClient,
    pub core_app_id: String,
}

impl Keystore {
    pub async fn init() -> Result<Self> {
        let passphrase =
            sodoken::BufRead::from(holo_config::default_password()?.as_bytes().to_vec());
        let keystore = holochain_keystore::lair_keystore::spawn_lair_keystore(
            url2::url2!("{}", holo_config::get_lair_url()?),
            passphrase,
        )
        .await?;

        let app_file = HappsFile::load_happ_file_from_env()?;
        let core_app = app_file
            .core_happs
            .iter()
            .find(|x| x.id().contains("core-app"))
            .ok_or(anyhow!("Could not find a core-app in HPOS file"))?;

        Ok(Self {
            keystore,
            core_app_id: core_app.id(),
        })
    }
}
