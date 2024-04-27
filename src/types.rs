use core::fmt::Debug;
use holochain_client::AgentPubKey;
use holochain_types::{
    dna::{ActionHash, AgentPubKeyB64, DnaHashB64, EntryHashB64},
    prelude::{holochain_serial, CapSecret, SerializedBytes, Signature, Timestamp},
};
use holofuel_types::fuel::Fuel;
use rocket::{
    serde::{json::serde_json, Deserialize, Serialize},
    Responder,
};

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
