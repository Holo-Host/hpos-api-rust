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
    pub total: Fuel,
    pub last_7_days: Fuel,
    pub average_weekly: Fuel,
}
impl Default for Earnings {
    fn default() -> Self {
        Earnings {
            total: Fuel::new(0),
            last_7_days: Fuel::new(0),
            average_weekly: Fuel::new(0),
        }
    }
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

// Simplified typeype for yaml::to_str to extract happ_id form Invoice Note
#[derive(Serialize, Deserialize, Debug)]
pub struct InvoiceNote {
    pub hha_id: ActionHashB64,
}

// Return type of zome call holofuel/transactor/get_completed_transactions
#[derive(Serialize, Deserialize, Debug, Clone, SerializedBytes)]
pub struct Transaction {
    pub id: EntryHashB64,
    pub amount: String,
    pub fee: String,
    pub created_date: Timestamp,
    pub completed_date: Option<Timestamp>,
    pub transaction_type: TransactionType,
    pub counterparty: AgentPubKeyB64,
    pub direction: TransactionDirection,
    pub status: TransactionStatus,
    pub note: Option<String>,
    pub proof_of_service: Option<POS>,
    pub url: Option<String>,
    pub expiration_date: Option<Timestamp>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TransactionType {
    Request, //Invoice
    Offer,   //Promise
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TransactionDirection {
    Outgoing, // To(Address),
    Incoming, // From(Address),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TransactionStatus {
    Actionable, // tx that is create by 1st instance and waiting for counterparty to complete the tx
    Pending,    // tx that was created by 1st instance and second instance
    Accepted,   // tx that was accepted by counterparty but has yet to complete countersigning.
    Completed,
    Declined,
    Expired,
}

#[derive(Serialize, Deserialize, Debug, Clone, SerializedBytes)]
#[serde(rename_all = "snake_case")]
pub enum POS {
    Hosting(CapSecret),
    Redemption(String), // Contains wallet address
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
