use std::{collections::HashMap, fmt, time::Duration};

use anyhow::Result;
use holochain_client::AgentPubKey;
use holochain_types::{
    dna::{ActionHashB64, AgentPubKeyB64, EntryHashB64},
    prelude::{holochain_serial, CapSecret, SerializedBytes, Timestamp},
};
use holofuel_types::fuel::Fuel;
use rocket::serde::{Deserialize, Serialize};

use crate::hpos::Ws;

// Return value of API /hosted_happs endpoint
#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct HappDetails {
    pub id: ActionHashB64,
    pub name: String,
    pub description: String,
    pub categories: Vec<String>,
    pub enabled: bool,
    pub is_paused: bool,
    pub source_chains: u16,
    pub days_hosted: u16,
    pub earnings: Earnings,
    pub usage: RecentUsage,
    pub hosting_plan: HostingPlan,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct Earnings {
    total: Fuel,
    last_7_days: Fuel,
    average_weekly: Fuel,
}

#[derive(Serialize, Deserialize, Default)]
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
    Paid,
}

impl fmt::Display for HostingPlan {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            HostingPlan::Free => write!(f, "free"),
            HostingPlan::Paid => write!(f, "paid"),
        }
    }
}

// return type of hha/get_happs
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
    pub publisher_pricing_pref: PublisherPricingPref,
    pub login_config: LoginConfig,
    pub special_installed_app_id: Option<String>,
    pub host_settings: HostSettings,
    pub last_edited: Timestamp,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublisherPricingPref {
    pub cpu: Fuel,
    pub storage: Fuel,
    pub bandwidth: Fuel,
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

#[derive(Debug, Serialize, Deserialize, SerializedBytes)]
pub struct HostSettings {
    pub is_enabled: bool,
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

// return type of a zome call to hha/get_happ_preferences
#[derive(Debug, Serialize, Deserialize, SerializedBytes, Clone)]
pub struct ServiceloggerHappPreferences {
    pub provider_pubkey: AgentPubKey,
    pub max_fuel_before_invoice: Fuel,
    pub price_compute: Fuel,
    pub price_storage: Fuel,
    pub price_bandwidth: Fuel,
    pub max_time_before_invoice: Duration,
}

#[cfg(test)]
mod test {
    use holochain_types::dna::ActionHashB64;

    #[test]
    fn decode_hash() {
        let str = "uhCkklkJVx4u17eCaaKg_phRJsHOj9u57v_4cHQR-Bd9tb-vePRyC";
        ActionHashB64::from_b64_str(str).unwrap();
    }
}
