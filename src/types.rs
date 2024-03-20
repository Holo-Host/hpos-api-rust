use std::{fmt, str::FromStr, time::Duration};

use anyhow::{anyhow, Result};
use core::fmt::Debug;
use holochain_client::AgentPubKey;
use holochain_types::{
    dna::{ActionHash, ActionHashB64, AgentPubKeyB64, DnaHashB64, EntryHashB64},
    prelude::{holochain_serial, CapSecret, SerializedBytes, Signature, Timestamp},
};
use holofuel_types::fuel::Fuel;
use log::warn;
use rocket::{
    serde::{json::serde_json, Deserialize, Serialize},
    Responder,
};

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
    pub is_auto_disabled: bool,
    pub is_paused: bool,
    pub source_chains: Option<u16>,
    pub days_hosted: Option<u16>,
    pub earnings: Option<Earnings>,
    pub usage: Option<HappStats>,
    pub hosting_plan: Option<HostingPlan>,
    pub bundle_url: String,
    pub hosted_urls: Vec<String>,
}
impl HappDetails {
    pub async fn init(
        happ: &PresentedHappBundle,
        transactions: Vec<Transaction>,
        usage_interval: i64,
        ws: &mut Ws,
    ) -> Self {
        HappDetails {
            id: happ.id.clone(),
            name: happ.name.clone(),
            description: happ.name.clone(),
            categories: happ.categories.clone(),
            enabled: happ.host_settings.is_enabled,
            is_auto_disabled: happ.host_settings.is_auto_disabled,
            is_paused: happ.is_paused,
            source_chains: count_instances(happ.id.clone(), ws)
                .await
                .unwrap_or_else(|e| {
                    warn!("error counting instances for happ {}: {}", &happ.id, e);
                    None
                }),
            days_hosted: count_days_hosted(happ.last_edited).unwrap_or_else(|e| {
                warn!("error counting earnings for happ {}: {}", &happ.id, e);
                None
            }),
            earnings: count_earnings(transactions).await.unwrap_or_else(|e| {
                warn!("error counting earnings for happ {}: {}", &happ.id, e);
                None
            }),
            usage: get_usage(happ.id.clone(), usage_interval, ws)
                .await
                .unwrap_or_else(|e| {
                    warn!("error getting plan for happ {}: {}", &happ.id, e);
                    None
                }), // from SL TODO: actually query SL for this value
            hosting_plan: get_plan(happ.id.clone(), ws).await.unwrap_or_else(|e| {
                warn!("error getting plan for happ {}: {}", &happ.id, e);
                None
            }),
            bundle_url: happ.bundle_url.clone(),
            hosted_urls: happ.hosted_urls.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Clone, SerializedBytes)]
pub struct UsageTimeInterval {
    pub duration_unit: String,
    pub amount: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, SerializedBytes)]
#[serde(crate = "rocket::serde")]
pub struct HappStats {
    // we can return this is you want to return all source_chain that were running on this holoport
    // pub source_chain_count: u32,
    pub cpu: u64,
    pub bandwidth: u64, // payload size,
    pub disk_usage: u64,
}

async fn get_usage(
    happ_id: ActionHashB64,
    usage_interval: i64,
    ws: &mut Ws,
) -> Result<Option<HappStats>> {
    log::debug!("Calling get_stats for happ: {}::servicelogger", happ_id);
    let result: HappStats = ws
        .call_zome(
            format!("{}::servicelogger", happ_id),
            "servicelogger",
            "service",
            "get_stats",
            UsageTimeInterval {
                duration_unit: "DAY".to_string(),
                amount: usage_interval,
            },
        )
        .await?;
    Ok(Some(result))
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
    pub is_host_disabled: bool, // signals that the host was the origin of the last disable request/action
    pub is_auto_disabled: bool, // signals that an internal hpos service was the origin of the last disable request/action
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

pub async fn get_plan(happ_id: ActionHashB64, ws: &mut Ws) -> Result<Option<HostingPlan>> {
    let core_app_id = ws.core_app_id.clone();

    let s: ServiceloggerHappPreferences = ws
        .call_zome(
            core_app_id,
            "core-app",
            "hha",
            "get_happ_preferences",
            happ_id,
        )
        .await?;

    if s.price_compute == Fuel::new(0)
        && s.price_storage == Fuel::new(0)
        && s.price_bandwidth == Fuel::new(0)
    {
        Ok(Some(HostingPlan::Free))
    } else {
        Ok(Some(HostingPlan::Paid))
    }
}

pub async fn count_instances(happ_id: ActionHashB64, ws: &mut Ws) -> Result<Option<u16>> {
    // What filter shall I use in list_happs()? Is None correct?
    Ok(Some(
        ws.admin
            .list_apps(None)
            .await
            .map_err(|err| anyhow!("{:?}", err))?
            .iter()
            .fold(0, |acc, info| {
                if info.installed_app_id.contains(&format!("{}:uhCA", happ_id)) {
                    acc + 1
                } else {
                    acc
                }
            }),
    ))
}

// TODO: average_weekly still needs to be calculated - from total and days_hosted?
pub async fn count_earnings(transactions: Vec<Transaction>) -> Result<Option<Earnings>> {
    let mut e = Earnings::default();
    for p in transactions.iter() {
        let amount_fuel = Fuel::from_str(&p.amount)?;
        e.total = (e.total + amount_fuel)?;
        // if completed_date is within last week then add fuel to last_7_days, too
        let week = Duration::from_secs(7 * 24 * 60 * 60);
        if (Timestamp::now() - week)? < p.completed_date.unwrap() {
            e.last_7_days = (e.last_7_days + amount_fuel)?
        };
    }
    Ok(Some(e))
}

pub fn count_days_hosted(since: Timestamp) -> Result<Option<u16>> {
    Ok(Some((Timestamp::now() - since)?.num_days() as u16))
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

// --------servicelogger data types---------
// https://github.com/Holo-Host/servicelogger-rsm/blob/develop/zomes/service_integrity/src/entries/mod.rs

// Possible Servicelogger entry types
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes)]
pub enum LogEntry {
    DiskUsageLog(DiskUsageLog),
    ActivityLog(Box<ActivityLog>),
}

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes)]
pub struct ActivityLog {
    pub request: ClientRequest,
    pub response: HostResponse,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
// Corresponds to service logger ClientRequest
pub struct ClientRequest {
    pub agent_id: AgentPubKey, // This is the public key of the web user who issued this service request
    pub request: RequestPayload,
    pub request_signature: Signature,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HostResponse {
    pub host_metrics: HostMetrics, // All the metrics we want to record from the perspective of the Host
    // things needed to be able to generate weblog compatible output
    pub weblog_compat: ExtraWebLogData,
}

// cpu and bandwidth metrics that the host collects resulting from the zome call
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HostMetrics {
    pub cpu: u64,
    pub bandwidth: u64,
}

// All the extra data that may be needed to produce weblog compatible exports/outputs
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtraWebLogData {
    pub source_ip: String,
    pub status_code: i16, // 200, 401, 403, 404, etc...
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestPayload {
    pub host_id: String, // This should be the holoport pubkey as encoded in the url (ie Base36)
    pub timestamp: Timestamp, // time according to the web user agent (client-side)
    pub hha_pricing_pref: ActionHash,
    pub call_spec: CallSpec,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CallSpec {
    #[serde(with = "serde_bytes")]
    pub args_hash: Vec<u8>, // hash of the arguments
    pub function: String,     // function name being called
    pub zome: String,         // zome name of the function being called
    pub role_name: String,    // DNA alias/handle
    pub hha_hash: ActionHash, // happ_id
}

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes)]
pub struct DiskUsageLog {
    pub files: Vec<File>,
    pub source_chain_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct File {
    pub associated_dna: DnaHashB64,
    /// Typically .sqlite3, .sqlite3-shm, or .sqlite3-wal
    pub extension: String,
    /// File size in bytes
    pub size: u64,
}

#[derive(Serialize, Deserialize, Debug, SerializedBytes, Clone)]
pub struct RedemptionState {
    pub earnings: Fuel,
    pub redeemed: Fuel,
    pub available: Fuel,
}

#[derive(Serialize, Deserialize, Debug, SerializedBytes, Clone)]
pub struct HolofuelPaidUnpaid {
    pub date: String,
    pub paid: Fuel,
    pub unpaid: Fuel,
}

#[derive(Serialize, Deserialize, Debug, SerializedBytes, Clone)]
pub struct RedemableHolofuelHistogramResponse {
    pub dailies: Vec<HolofuelPaidUnpaid>,
    pub redeemed: Fuel,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct ZomeCallRequest {
    pub app_id: String,
    pub role_id: String,
    pub zome_name: String,
    pub fn_name: String,
    pub payload: serde_json::Value,
}

#[derive(Responder)]
#[response(status = 200, content_type = "binary")]
pub struct ZomeCallResponse(pub &'static [u8]);

#[cfg(test)]
mod test {
    use holochain_types::dna::ActionHashB64;

    #[test]
    fn decode_hash() {
        let str = "uhCkklkJVx4u17eCaaKg_phRJsHOj9u57v_4cHQR-Bd9tb-vePRyC";
        ActionHashB64::from_b64_str(str).unwrap();
    }
}
