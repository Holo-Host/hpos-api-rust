use std::{collections::HashMap, fmt};

use anyhow::Result;
use holochain_types::dna::ActionHashB64;
use holofuel_types::fuel::Fuel;
use rocket::serde::{Deserialize, Serialize};

use crate::hpos::Ws;

// Return value of hosted_happs
#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct HappDetails {
    id: String,
    name: String,
    description: String,
    categories: Vec<String>,
    enabled: bool,
    is_paused: bool,
    source_chains: u16,
    days_hosted: u16,
    earnings: Earnings,
    usage: RecentUsage,
    hosting_plan: HostingPlan,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct Earnings {
    total: Fuel,
    last_7_days: Fuel,
    average_weekly: Fuel
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct RecentUsage {
    bandwidth: u64, // in bytes?
    cpu: u64,       // now always set to 0
    storage: u64,   // now always set to 0
    interval: u32,  // in seconds
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub enum HostingPlan {
    Free,
    Paid
}

impl fmt::Display for HostingPlan {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            HostingPlan::Free => write!(f, "free"),
            HostingPlan::Paid => write!(f, "paid"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HappAndHost {
    pub happ_id: ActionHashB64,
    pub holoport_id: String, // in base36 encoding
}

impl HappAndHost {
    pub async fn init(happ_id: &str, ws: &mut Ws) -> Result<Self> {
        // AgentKey used for installation of hha is a HoloHash created from Holoport owner's public key.
        // This public key encoded in base36 is also holoport's id in `https://<holoport_id>.holohost.net`
        let (_, pub_key) = ws.get_cell(ws.core_app_id.clone(), "core-app").await?;

        let a = pub_key.get_raw_32();

        let holoport_id = base36::encode(a);

        Ok(HappAndHost {
            happ_id: ActionHashB64::from_b64_str(happ_id)?,
            holoport_id,
        })
    }
}

pub type AllEarnings = HashMap<&'static str, HappEarninrs>;

pub struct HappEarninrs {}

#[cfg(test)]
mod test {
    use holochain_types::dna::ActionHashB64;

    #[test]
    fn decode_hash() {
        let str = "uhCkklkJVx4u17eCaaKg_phRJsHOj9u57v_4cHQR-Bd9tb-vePRyC";
        ActionHashB64::from_b64_str(str).unwrap();
    }
}
