use holochain_types::dna::ActionHashB64;
use hpos_hc_connect::app_connection::CoreAppRoleName;
use anyhow::Result;

use crate::hpos::Ws;
use rocket::serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HappAndHost {
    pub happ_id: ActionHashB64,
    pub holoport_id: String, // in base36 encoding
}

impl HappAndHost {
    pub async fn init(happ_id: &str, ws: &mut Ws) -> Result<Self> {
        // AgentKey used for installation of hha is a HoloHash created from Holoport owner's public key.
        // This public key encoded in base36 is also holoport's id in `https://<holoport_id>.holohost.net`
        let app_connection = ws.get_connection(ws.core_app_id.clone()).await?;

        let cell = app_connection.cell(CoreAppRoleName::HHA.into()).await?;

        let a = cell.agent_pubkey().get_raw_32();

        let holoport_id = base36::encode(a);

        Ok(HappAndHost {
            happ_id: ActionHashB64::from_b64_str(happ_id)?,
            holoport_id,
        })
    }
}
