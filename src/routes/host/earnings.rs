use holochain_types::{dna::{ActionHashB64, AgentPubKeyB64, EntryHashB64}, prelude::{CapSecret, Timestamp}};
use holofuel_types::fuel::Fuel;
use hpos_hc_connect::{holofuel_types::{Transaction, TransactionDirection, TransactionStatus, TransactionType}, AppConnection};
use log::warn;
use rocket::{
    http::Status,
    serde::{json::Json, Deserialize, Serialize},
    {get, State},
};
use anyhow::{anyhow, Result};

use crate::hpos::WsMutex;

/// Returns overview of host earnings as needed for the host-console-ui dashboard page
/// -- includes optional cutoff quantity param to control the volume of recent hosting payments to return to client
#[get("/earnings?<quantity>")]
pub async fn earnings(wsm: &State<WsMutex>, quantity: u16) -> Result<Json<()>, (Status, String)> {
    let mut ws = wsm.lock().await;

    let core_app_connection = ws.get_connection(ws.core_app_id.clone()).await.unwrap();

    let happs_by_invoice_details = get_hosted_happ_invoice_details(vec![]).unwrap();

    Ok(Json(()))
}

async fn get_hosting_invoices(core_app_connection: AppConnection) -> Result<()> {
    Ok(())
}

fn get_hosted_happ_invoice_details(transactions: Vec<Transaction>) -> Result<Vec<TransactionAndInvoiceDetails>> {
    let mut transaction_and_invoice_details = transactions.into_iter()    
    .map(|transaction| {
        let Transaction {
            id,
            amount,
            fee,
            created_date,
            completed_date,
            transaction_type,
            counterparty,
            direction,
            status,
            note,
            proof_of_service_token,
            url,
            expiration_date,
        } = transaction;

        let happ_id = "fkjdsf";

        if let Some(parsed_note) = parse_note(note) {

        } else {
            None // in the js code, this value is "undefined"
        }
    })
    .collect();
}

fn parse_note(unparsed_note: Option<String>) -> Option<Note> {
    if let Some(note) = unparsed_note {
        let parsed_note: Note = match serde_yaml::from_str(&note) {
            Ok(parsed_note) => {
                if is_valid_hosting_note(parsed_note) {
                    parsed_note
                } else {
                    return None
                }
            },
            Err(_) => {
                warn!("Failed to parse invoice note: {}", note);
                return None
            },
        };
        Some(parsed_note)
    } else {
        None
    }
}

// In the js code this does some additional checking that we get for free from serde
fn is_valid_hosting_note(note: Note) -> bool {
    let Note(description, _) = note;
    return description.contains("Holo Hosting Invoice for")
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct HostEarningsResponse {
    earnings: LastEarnings,
    holofuel: HolofuelBalances,
    recent_payments: RecentPaymentsDetails,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct LastEarnings {
    last30days: Fuel,
    last7days: Fuel,
    lastday: Fuel,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct HolofuelBalances {
    redeemable: Fuel,
    balance: Fuel,
    available: Fuel,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct RecentPaymentsDetails {
    // id: 1,
    // amount: Fuel,
    // status: 'received',
    // updatedAt: Date.now(),
    // happ: {
    // name: 'HoloFuel',
    // id: 123
    // },
    // invoiceDetails: {
    // start: '',
    // end: '',
    // bandwidth: {
    //     price: 1234, // hosting bandwidth prices hf/mb
    //     quantity: 1 // traffic serviced (should be in mb) - to calculate bandwidth,
    // },
    // compute: {
    //     price: 12,
    //     quantity: 234
    // },
    // storage: {
    //     price: 432
    // }
    // }
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct TransactionAndInvoiceDetails {
    id: EntryHashB64,
    amount: Fuel,
    status: TransactionStatus,
    r#type: TransactionType,
    direction: TransactionDirection,
    created_date: Timestamp,
    completed_date: Option<Timestamp>,
    expiration_date: Option<Timestamp>,
    counterparty: AgentPubKeyB64,
    note: String,
    proof_of_service: Option<CapSecret>,
    url: String,
    happ: HappNameAndId,
    invoice_details: InvoiceDetails,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct HappNameAndId {
    name: String,
    id: ActionHashB64,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct InvoiceDetails {
    start: String,
    end: String,    
    bandwidth: InvoicedItem,
    compute: InvoicedItem,
    storage: InvoicedItem,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")] // TODO ?
pub struct ParsedNote {
    quantity: String,
    price: Fuel,
}

// The deserialized type that is represented as a serialized string in an invoice Note
// We should probably move this to the holofuel_types crate
#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct InvoiceNote {
    hha_id: ActionHashB64,
    invoice_period_start: Timestamp,
    invoice_period_end: Timestamp,
    // This can be commented back in when the chc can support larger entries [#78](https://github.com/Holo-Host/servicelogger-rsm/pull/78)
    // activity_logs_range: Vec<ActionHashB64>,
    // disk_usage_logs_range: Vec<ActionHashB64>,
    #[serde(flatten)]
    invoiced_items: InvoicedItem,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct InvoicedItem {
    quantity: String, // we're using serde_yaml to convert the struct into a string
    prices: String,   // we're using serde_yaml to convert the struct into a string
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Note(String, InvoiceNote);