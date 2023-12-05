use holochain_client::AgentPubKey;
use holochain_types::dna::ActionHash;
use holochain_types::prelude::{SerializedBytes, Signature, Timestamp};
pub use holochain_types::{
    dna::{ActionHashB64, AgentPubKeyB64},
    prelude::holochain_serial,
};
use holofuel_types::fuel::Fuel;
use serde::{Deserialize, Serialize};
use std::fmt;

// --------servicelogger data types---------
// https://github.com/Holo-Host/servicelogger-rsm/blob/develop/zomes/service_integrity/src/entries/activity_log.rs#L6

#[derive(Debug, Serialize, Deserialize)]
pub struct ActivityLog {
    pub request: ClientRequest,
    pub response: HostResponse,
}

#[derive(Debug, Serialize, Deserialize)]
// Corresponds to service logger ClientRequest
pub struct ClientRequest {
    pub agent_id: AgentPubKey, // This is the public key of the web user who issued this service request
    pub request: RequestPayload,
    pub request_signature: Signature,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HostResponse {
    pub host_metrics: HostMetrics, // All the metrics we want to record from the perspective of the Host
    // things needed to be able to generate weblog compatible output
    pub weblog_compat: ExtraWebLogData,
}

// cpu and bandwidth metrics that the host collects resulting from the zome call
#[derive(Debug, Serialize, Deserialize)]
pub struct HostMetrics {
    pub cpu: u64,
    pub bandwidth: u64,
}

// All the extra data that may be needed to produce weblog compatible exports/outputs
#[derive(Debug, Serialize, Deserialize)]
pub struct ExtraWebLogData {
    pub source_ip: String,
    pub status_code: i16, // 200, 401, 403, 404, etc...
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestPayload {
    pub host_id: String, // This should be the holoport pubkey as encoded in the url (ie Base36)
    pub timestamp: Timestamp, // time according to the web user agent (client-side)
    pub hha_pricing_pref: ActionHash,
    pub call_spec: CallSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallSpec {
    #[serde(with = "serde_bytes")]
    pub args_hash: Vec<u8>, // hash of the arguments
    pub function: String,     // function name being called
    pub zome: String,         // zome name of the function being called
    pub role_name: String,    // DNA alias/handle
    pub hha_hash: ActionHash, // happ_id
}
