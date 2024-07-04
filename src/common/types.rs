use crate::hpos::Ws;
use anyhow::Result;
use core::fmt::Debug;
use holochain_types::{
    dna::{ActionHashB64, AgentPubKeyB64, EntryHashB64},
    prelude::{holochain_serial, CapSecret, SerializedBytes, Timestamp},
};
use holofuel_types::fuel::Fuel;
use hpos_hc_connect::app_connection::CoreAppRoleName;
use rocket::serde::{Deserialize, Serialize};

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

#[derive(Serialize, Deserialize, Debug, SerializedBytes)]
pub struct PendingTransactions {
    pub invoice_pending: Vec<Transaction>,
    pub promise_pending: Vec<Transaction>,
    pub invoice_declined: Vec<Transaction>,
    pub promise_declined: Vec<Transaction>,
    pub accepted: Vec<Transaction>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TransactionType {
    Request, //Invoice
    Offer,   //Promise
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, SerializedBytes, Clone)]
pub struct RedemptionState {
    pub earnings: Fuel,
    pub redeemed: Fuel,
    pub available: Fuel,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HappAndHost {
    pub happ_id: ActionHashB64,
    pub holoport_id: String, // in base36 encoding
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Ledger {
    pub balance: Fuel,
    pub promised: Fuel,
    pub fees: Fuel,
    pub available: Fuel,
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

// return type of hha/get_happs and hha/register
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

impl Default for PublisherPricingPref {
    fn default() -> Self {
        PublisherPricingPref {
            cpu: Fuel::new(0),
            storage: Fuel::new(0),
            bandwidth: Fuel::new(0),
        }
    }
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
    pub is_host_disabled: bool, // signals that the host was the origin of the last disable request/action
    pub is_auto_disabled: bool, // signals that an internal hpos service was the origin of the last disable request/action
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes, Clone, Default)]
#[serde(crate = "rocket::serde")]
pub struct HappInput {
    pub hosted_urls: Vec<String>,
    pub bundle_url: String,
    #[serde(default)]
    pub ui_src_url: Option<String>,
    #[serde(default)]
    pub special_installed_app_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub logo_url: Option<String>,
    #[serde(default)]
    pub dnas: Vec<DnaResource>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub jurisdictions: Vec<String>,
    #[serde(default)]
    pub exclude_jurisdictions: bool,
    #[serde(default)]
    pub publisher_pricing_pref: PublisherPricingPref,
    #[serde(default)]
    pub login_config: LoginConfig,
    #[serde(default)]
    pub uid: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]

pub struct RedemptionRecord {
    pub redemption_id: EntryHashB64,
    pub holofuel_acceptance_hash: ActionHashB64,
    pub ethereum_transaction_hash: String,
    pub processing_stage: ProcessingStage,
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub enum ProcessingStage {
    Invalid,
    New,
    Verified,
    SentHolofuel,
    AcceptedHolofuel,
    ScheduledForCountersigning,
    CountersignedHolofuel,
    Finished,
}
