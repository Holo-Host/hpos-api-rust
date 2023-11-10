use crate::consts::{ADMIN_PORT, APP_PORT};
use anyhow::{anyhow, Context, Result};
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

    /// make a zome call to a running app
    pub async fn call_zome<T: Debug + Serialize, R: Debug + for<'de> Deserialize<'de>>(
        &mut self,
        app_id: InstalledAppId,
        role_name: &'static str,
        zome_name: &'static str,
        fn_name: &'static str,
        payload: T,
    ) -> Result<R> {
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

        ExternIO::decode(&response).map_err(|err| anyhow!("{:?}", err))
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
