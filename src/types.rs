use core::fmt::Debug;
use holochain_types::{
    dna::{AgentPubKeyB64, EntryHashB64},
    prelude::{holochain_serial, CapSecret, SerializedBytes, Timestamp},
};
use holofuel_types::fuel::Fuel;
use rocket::serde::{ Deserialize, Serialize};

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
