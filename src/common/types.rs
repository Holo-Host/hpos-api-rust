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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ErrorResponse {
    pub message: String,
}
