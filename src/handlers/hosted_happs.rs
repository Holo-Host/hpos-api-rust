use holochain_client::AgentPubKey;
use holochain_types::prelude::{
    holochain_serial, Entry, Record, RecordEntry, SerializedBytes, Signature, Timestamp,
};
use rocket::serde::{Deserialize, Serialize};

use crate::common::types::{Transaction, POS};
use crate::hpos::Ws;
use crate::types::{HappAndHost, HappDetails, PresentedHappBundle};
use anyhow::Result;
use holochain_types::dna::{ActionHash, ActionHashB64, DnaHashB64};
use log::debug;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

type AllTransactions = HashMap<ActionHashB64, Vec<Transaction>>;

// Simplified typeype for yaml::to_str to extract happ_id form Invoice Note
#[derive(Serialize, Deserialize, Debug)]
pub struct InvoiceNote {
    pub hha_id: ActionHashB64,
}

// fetch all transactions for every hApp
pub async fn handle_get_all(
    usage_interval: i64,
    quantity: Option<usize>,
    ws: &mut Ws,
) -> Result<Vec<HappDetails>> {
    let app_connection = ws.get_connection(ws.core_app_id.clone()).await?;

    debug!("calling zome hha/get_happs");
    let all_hosted_happs: Vec<PresentedHappBundle> = app_connection
        .zome_call_typed("core-app".into(), "hha".into(), "get_happs".into(), ())
        .await?;

    // Ask holofuel for all transactions so that I can calculate earings - isn't it ridiculous?
    let mut all_transactions = get_all_transactions(ws).await?;

    let mut result: Vec<HappDetails> = vec![];
    for happ in all_hosted_happs.iter() {
        result.push(
            HappDetails::init(
                happ,
                all_transactions.remove(&happ.id).unwrap_or(vec![]),
                usage_interval,
                ws,
            )
            .await,
        );
    }

    // sort vec by earnings.last_7_days in decreasing order
    result.sort_by(|a, b| {
        let a = a.earnings.clone().unwrap_or_default();
        let b = b.earnings.clone().unwrap_or_default();
        a.last_7_days.cmp(&b.last_7_days)
    });

    // take first `quantity` only
    if let Some(q) = quantity {
        result.truncate(q);
    }

    Ok(result)
}

// fetch all transactions for 1 happ
pub async fn handle_get_one(
    id: ActionHashB64,
    usage_interval: i64,
    ws: &mut Ws,
) -> Result<HappDetails> {
    let app_connection = ws.get_connection(ws.core_app_id.clone()).await?;

    debug!("calling zome hha/get_happs");
    let happ: PresentedHappBundle = app_connection
        .zome_call_typed("core-app".into(), "hha".into(), "get_happ".into(), id)
        .await?;

    // Ask holofuel for all transactions so that I can calculate earings - isn't it ridiculous?
    let mut all_transactions = get_all_transactions(ws).await?;

    Ok(HappDetails::init(
        &happ,
        all_transactions.remove(&happ.id).unwrap_or(vec![]),
        usage_interval,
        ws,
    )
    .await)
}

/// get all holofuel transactions and organize in HashMap by happ_id extracted from invoice's note
async fn get_all_transactions(ws: &mut Ws) -> Result<AllTransactions> {
    let app_connection = ws.get_connection(ws.core_app_id.clone()).await?;

    let mut return_map: AllTransactions = HashMap::new();

    debug!("calling zome holofuel/transactor/get_completed_transactions");
    let mut a = app_connection
        .zome_call_typed::<(), Vec<Transaction>>(
            "holofuel".into(),
            "transactor".into(),
            "get_completed_transactions".into(),
            (),
        )
        .await?;

    while let Some(tx) = a.pop() {
        // only add happ to list if it is a valid hosting invoice
        if let Some(POS::Hosting(_)) = tx.proof_of_service.clone() {
            if let Some(note) = tx.note.clone() {
                if let Ok((_, n)) = serde_yaml::from_str::<(String, InvoiceNote)>(&note) {
                    if let Some(mut vec) = return_map.remove(&n.hha_id) {
                        vec.push(tx);
                        return_map.insert(n.hha_id, vec);
                    } else {
                        return_map.insert(n.hha_id, vec![tx]);
                    }
                }
            }
        }
    }

    Ok(return_map)
}

/// Enable happ for hosting in core happ
pub async fn handle_enable(ws: &mut Ws, payload: HappAndHost) -> Result<()> {
    debug!("calling zome hha/enable_happ with payload: {:?}", &payload);
    let app_connection = ws.get_connection(ws.core_app_id.clone()).await?;

    app_connection
        .zome_call_typed(
            "core-app".into(),
            "hha".into(),
            "enable_happ".into(),
            payload,
        )
        .await?;

    Ok(())
}

/// Disable happ for hosting in core happ
pub async fn handle_disable(ws: &mut Ws, payload: HappAndHost) -> Result<()> {
    debug!("calling zome hha/disable_happ with payload: {:?}", &payload);
    let app_connection = ws.get_connection(ws.core_app_id.clone()).await?;

    app_connection
        .zome_call_typed(
            "core-app".into(),
            "hha".into(),
            "disable_happ".into(),
            payload,
        )
        .await?;

    Ok(())
}

/// Get service logs for last `days` days for happ with id `id`
pub async fn handle_get_service_logs(
    ws: &mut Ws,
    id: ActionHashB64,
    days: i32,
) -> Result<Vec<LogEntry>> {
    let filter = holochain_types::prelude::ChainQueryFilter::new().include_entries(true);

    let app_connection = ws.get_connection(format!("{}::servicelogger", id)).await?;

    log::debug!("getting logs for happ: {:?}::servicelogger", id);
    let result: Vec<Record> = app_connection
        .zome_call_typed(
            "servicelogger".into(),
            "service".into(),
            "querying_chain".into(),
            filter,
        )
        .await?;

    let four_weeks_ago = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
        - (days as u64 * 24 * 60 * 60)) as i64;

    log::debug!("filtering logs from {}", id);

    let filtered_result: Vec<LogEntry> = result
        .into_iter()
        .filter(|record| record.action().timestamp().as_seconds_and_nanos().0 > four_weeks_ago)
        // include only App Entries (those listed in #[hdk_entry_defs] in DNA code),
        // not holochain system entries
        // and deserialize them into service logger's entries
        .filter_map(|record| {
            if let RecordEntry::Present(Entry::App(bytes)) = record.entry() {
                if let Ok(log_entry) = ActivityLog::try_from(bytes.clone().into_sb()) {
                    return Some(LogEntry::ActivityLog(Box::new(log_entry)));
                } else if let Ok(log_entry) = DiskUsageLog::try_from(bytes.clone().into_sb()) {
                    return Some(LogEntry::DiskUsageLog(log_entry));
                }
            }
            None
        })
        .collect();

    Ok(filtered_result)
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
