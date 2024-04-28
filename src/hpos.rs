use crate::common::consts::{ADMIN_PORT, APP_PORT};
use anyhow::{anyhow, Context, Result};
use core::fmt::Debug;
use holochain_client::{
    AdminWebsocket, AgentPubKey, AppWebsocket, ConductorApiError, InstalledAppId, ZomeCall,
};
use holochain_conductor_api::{CellInfo, ProvisionedCell};
use holochain_keystore::MetaLairClient;
use holochain_types::prelude::{ExternIO, ZomeCallUnsigned};
use holochain_websocket::WebsocketError;
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
        })
    }

    /// get cell details of the hha agent
    pub async fn get_cell(
        &mut self,
        app_id: InstalledAppId,
        role_name: &'static str,
    ) -> Result<(ProvisionedCell, AgentPubKey)> {
        let response = self.app.app_info(app_id.clone()).await;

        match response {
            Ok(response) => match response {
                Some(app_info) => {
                    let cell = match &app_info.cell_info.get(role_name).unwrap()[0] {
                        // [0] because first one in a Vec is Provisioned cell
                        CellInfo::Provisioned(c) => c.clone(),
                        _ => return Err(anyhow!("unable to find {}", role_name)),
                    };
                    Ok((cell, app_info.agent_pub_key))
                }
                _ => Err(anyhow!("{} is not installed", role_name)),
            },
            Err(error) => match error {
                ConductorApiError::WebsocketError(websocket_err) => {
                    match self.handle_websocket_error(websocket_err).await {
                        Ok(result) => match result {
                            true => match self.app.app_info(app_id).await.map_err(|err| anyhow!("{:?}", err))? {
                                Some(app_info) => {
                                    let cell = match &app_info.cell_info.get(role_name).unwrap()[0] {
                                        // [0] because first one in a Vec is Provisioned cell
                                        CellInfo::Provisioned(c) => c.clone(),
                                        _ => return Err(anyhow!("unable to find {}", role_name)),
                                    };
                                    Ok((cell, app_info.agent_pub_key))
                                },
                                _ => Err(anyhow!("{} is not installed", role_name)),
                            },
                            false => Err(anyhow!("failed to reconnect websocket connection, Could not execute zome call")),
                        },
                        err => Err(anyhow!("{:?}", err)),
                    }
                }
                err => Err(anyhow!("{:?}", err)),
            },
        }
    }

    /// make a zome call to a running app and decode to <T> type
    pub async fn call_zome<T: Debug + Clone + Serialize, R: Debug + for<'de> Deserialize<'de>>(
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
    pub async fn call_zome_raw<T: Debug + Clone + Serialize>(
        &mut self,
        app_id: InstalledAppId,
        role_name: &'static str,
        zome_name: &'static str,
        fn_name: &'static str,
        payload: T,
    ) -> Result<ExternIO> {
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

        let response = self.app.call_zome(signed_zome_call.clone()).await;

        match response {
            // return response if no error is thrown
            Ok(response) => {
                log::debug!("zome call response raw bytes: {:?}", &response);
                Ok(response)
            }
            Err(err) => match err {
                // check if websocket connection has issues
                ConductorApiError::WebsocketError(websocket_err) => {
                    match self.handle_websocket_error(websocket_err).await {
                        Ok(result) => match result {
                            // if re-connected. run the same zome call
                            true => self.app.call_zome(signed_zome_call).await.map_err(|err| anyhow!("{:?}", err)),
                            false => Err(anyhow!("failed to reconnect websocket connection, Could not execute zome call")),
                        },
                        err => Err(anyhow!("{:?}", err)),
                    }
                }
                err => Err(anyhow!("{:?}", err)),
            },
        }
    }

    // re-connect websocket connection if a specific error was thrown
    async fn handle_websocket_error(&mut self, err: WebsocketError) -> Result<bool> {
        match err {
            WebsocketError::RespTimeout => self.reconnect(5).await,
            WebsocketError::Shutdown => self.reconnect(5).await,
            err => Err(anyhow!("{:?}", err)),
        }
    }

    // try to reconnect websocket connection
    pub async fn reconnect(&mut self, max_retries: u32) -> Result<bool> {
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
            let keystore = Keystore {
                keystore: self.keystore.clone(),
                core_app_id: self.core_app_id.clone(),
            };
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
        match is_connected {
            true => {
                log::info!("successfully reconnected!");
            }
            false => {
                log::error!(
                    "failed to establish websocket connection after {} retries",
                    max_retries
                );
            }
        };
        Ok(is_connected)
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
