use holochain_types::{dna::{ActionHashB64, AgentPubKeyB64, EntryHashB64}, prelude::{CapSecret, Timestamp}};
use holofuel_types::fuel::Fuel;
use hpos_hc_connect::AppConnection;
use log::warn;
use rocket::{
    serde::{Deserialize, Serialize}, FromFormField
};
use anyhow::Result;

pub async fn get_hosting_invoices(mut core_app_connection: AppConnection, invoice_set: InvoiceSet) -> Result<HostingInvoicesResponse> {
    fn is_hosting_invoice (transaction: &Transaction) -> bool {
        if let Some(proof_of_service) = transaction.proof_of_service.clone() { 
            match proof_of_service {
                POS::Hosting(_) => true,
                POS::Redemption(_) => false,
            }
        } else {
            false
        }
    }

    let paid_hosting_invoices: Vec<Transaction> = if invoice_set.includes_paid() {
        core_app_connection.zome_call_typed::<(), Vec<Transaction>>(
            "holofuel".into(), 
            "transactor".into(), 
            "get_completed_transactions".into(), 
            ()
        ).await?
        .into_iter()
        .filter(is_hosting_invoice)
        .collect()
    } else {
        Vec::new()
    };

    let pending_txs: Vec<Transaction> = if invoice_set.includes_unpaid() {
        core_app_connection.zome_call_typed::<(), PendingResponse>(
            "holofuel".into(), 
            "transactor".into(), 
            "get_pending_transactions".into(), 
            ()
        ).await?
        .flatten()
        .into_iter()
        .filter(is_hosting_invoice)
        .collect()
    } else {
        Vec::new()
    };

    let actionable_txs: Vec<Transaction> = if invoice_set.includes_unpaid() {
        core_app_connection.zome_call_typed::<(), ActionableResponse>(
            "holofuel".into(), 
            "transactor".into(), 
            "get_actionable_transactions".into(), 
            ()
        ).await?
        .flatten()
        .into_iter()
        .filter(is_hosting_invoice)
        .collect()
    } else {
        Vec::new()
    };

    let unpaid_hosting_invoices: Vec<Transaction> = pending_txs
    .into_iter()
    .chain(actionable_txs.into_iter())
    .filter(is_hosting_invoice)
    .collect();

    let transaction_and_invoice_details = get_hosted_happ_invoice_details(
        paid_hosting_invoices.clone().into_iter().chain(unpaid_hosting_invoices.clone().into_iter()).collect()
    )?;

    Ok(HostingInvoicesResponse{
        paid_hosting_invoices,
        unpaid_hosting_invoices,
        transaction_and_invoice_details,
    })
}

fn get_hosted_happ_invoice_details(transactions: Vec<Transaction>) -> Result<Vec<TransactionAndInvoiceDetails>> {
    let mut transaction_and_invoice_details: Vec<TransactionAndInvoiceDetails> = transactions.into_iter()    
    .map(|transaction| {
        let Transaction {
            id,
            amount,
            created_date,
            completed_date,
            transaction_type,
            counterparty,
            direction,
            status,
            note,
            proof_of_service,
            url,
            expiration_date,
            ..
        } = transaction;

        if let Some(parsed_note) = parse_note(note) {
            let Note (human_readable_note, invoice_note) = parsed_note;

            let happ_name = read_happ_name(&human_readable_note).to_owned();

            let InvoiceNote {
                hha_id,
                invoice_period_start,
                invoice_period_end,
                invoiced_items,
            } = invoice_note;
            
            // I think we can simplify the structure of the invoice note so that we don't need these nested parses, but that would require a change to servicelogger
            // and potential some other components, so leaving as is for now.
            let (invoice_usage, invoice_prices) = match parse_invoiced_items(&invoiced_items) {
                Ok(parsed) => parsed,
                Err(e) => {
                    warn!("Failed to parse invoiced_items {:?} with error {:?}", invoiced_items, e);
                    return None
                },
            };

            return Some(TransactionAndInvoiceDetails {
                id,
                amount,
                status,
                r#type: transaction_type,
                direction,
                created_date,
                completed_date,
                expiration_date,
                counterparty,
                note: human_readable_note,
                proof_of_service,
                url,
                happ: HappNameAndId {
                    name: happ_name,
                    id: hha_id,
                },
                invoice_details: InvoiceDetails {
                    start: invoice_period_start,
                    end: invoice_period_end,
                    bandwidth: QuantityAndPrice {
                        quantity: invoice_usage.bandwidth,
                        price: invoice_prices.bandwidth
                    },
                    compute: QuantityAndPrice {
                        quantity: invoice_usage.cpu,
                        price: invoice_prices.cpu
                    },
                    storage: QuantityAndPrice {
                        quantity: invoice_usage.storage,
                        price: invoice_prices.storage
                    },
                },
            })
        } else {
            None // in the js code, this value is "undefined"
        }
    })
    .filter_map(|x| x)
    .collect();

    transaction_and_invoice_details.sort_by_key(|transaction| transaction.completed_date);

    Ok(transaction_and_invoice_details)
}

fn parse_note(unparsed_note: Option<String>) -> Option<Note> {
    if let Some(note) = unparsed_note {
        let parsed_note: Note = match serde_yaml::from_str(&note) {
            Ok(parsed_note) => {
                if is_valid_hosting_note(&parsed_note) {
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
fn is_valid_hosting_note(note: &Note) -> bool {
    let Note(human_readable_note, _) = note;
    return human_readable_note.contains("Holo Hosting Invoice for")
}

fn read_happ_name(human_readable_note: &str) -> String {
    if let Some(happ) = human_readable_note.split("Holo Hosting Invoice for ").nth(1) {
        let happ_name_part = happ.split("(...").next().unwrap_or("");
        let name = happ_name_part.replace("\"", "").trim().to_string();
        return name;
    }

    // As in the js code, we assume the above goes well, and return empty string otherwise
    "".to_string()
}


fn parse_invoiced_items(invoiced_items: &InvoicedItems) -> Result<(InvoiceUsage, InvoicePrices)> {
    let invoice_usage = serde_yaml::from_str(&invoiced_items.quantity)?;
    let invoice_prices = serde_yaml::from_str(&invoiced_items.prices)?;

    Ok((invoice_usage, invoice_prices))
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct HostingInvoicesResponse {
    pub paid_hosting_invoices: Vec<Transaction>,
    pub unpaid_hosting_invoices: Vec<Transaction>,
    pub transaction_and_invoice_details: Vec<TransactionAndInvoiceDetails>,
}

#[derive(Serialize, Deserialize, FromFormField)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub enum InvoiceSet {
    All,
    Paid,
    Unpaid
}

impl InvoiceSet {
    pub fn includes_paid(&self) -> bool {
        match &self {
            InvoiceSet::Unpaid => false,
            _ => true
        }
    }

    pub fn includes_unpaid(&self) -> bool {
        match &self {
            InvoiceSet::Paid => false,
            _ => true
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct TransactionAndInvoiceDetails {
    id: EntryHashB64,
    amount: String,
    status: TransactionStatus,
    r#type: TransactionType,
    direction: TransactionDirection,
    created_date: Timestamp,
    completed_date: Option<Timestamp>,
    expiration_date: Option<Timestamp>,
    counterparty: AgentPubKeyB64,
    note: String,
    proof_of_service: Option<POS>,
    url: Option<String>,
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
    start: Timestamp,
    end: Timestamp,    
    bandwidth: QuantityAndPrice,
    compute: QuantityAndPrice,
    storage: QuantityAndPrice,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct QuantityAndPrice {
    quantity: u64,
    price: Fuel,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct ActionableResponse {
    invoice_actionable: Vec<Transaction>,
    promise_actionable: Vec<Transaction>,
}

impl ActionableResponse {
    // It doesn't look quite right to me to include both of these Vecs in the host_earnings, but this reproduces the logic of the js code
    // See also the PendingResponse::flatten method
    pub fn flatten(&self) -> Vec<Transaction> {
        self.invoice_actionable.clone()
        .into_iter()
        .chain(self.promise_actionable.clone().into_iter())
        .collect()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct PendingResponse {
    pub invoice_pending: Vec<Transaction>,
    pub promise_pending: Vec<Transaction>,
    pub invoice_declined: Vec<Transaction>,
    pub promise_declined: Vec<Transaction>,
    pub accepted: Vec<Transaction>,
}

impl PendingResponse {
    // See comment in the ActionableResponse::flatten
    pub fn flatten(&self) -> Vec<Transaction> {
        self.invoice_pending.clone()
        .into_iter()
        .chain(self.promise_pending.clone().into_iter())
        .chain(self.invoice_declined.clone().into_iter())
        .chain(self.promise_declined.clone().into_iter())
        .chain(self.accepted.clone().into_iter())
        .collect()
    }
}

// START OF HOLOFUEL TYPES (copied from Holofuel)

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
    pub id: EntryHashB64,
    pub amount: String,
    pub fee: String,
    pub created_date: Timestamp,
    pub completed_date: Option<Timestamp>,
    pub transaction_type: TransactionType, // The type returned will be the type of the initial transaction
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum POS {
    Hosting(CapSecret),
    Redemption(String), // Contains wallet address
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Ledger {
    pub balance: Fuel,
    pub promised: Fuel,
    pub fees: Fuel,
    pub available: Fuel,
}

// END OF HOLOFUEL TYPES

// START OF SERVICELOGGER TYPES (copied from Servicelogger, we should probably move these types in `holofuel_types` to avoid drift)

// The deserialized type that is represented as a serialized string in an the `note` field of an invoice
#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Note(String, InvoiceNote);

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct InvoiceNote {
    hha_id: ActionHashB64,
    invoice_period_start: Timestamp,
    invoice_period_end: Timestamp,
    #[serde(flatten)]
    invoiced_items: InvoicedItems,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct InvoicedItems {
    quantity: String, // a yaml encoding of an instance of InvoiceUsage
    prices: String,   // a yaml encoding of an instance of InvoicePrices
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct InvoiceUsage {
    pub bandwidth: u64,
    pub storage: u64,
    pub cpu: u64,
}

#[derive(Serialize, Deserialize, Debug)]
struct InvoicePrices {
    pub bandwidth: Fuel,
    pub storage: Fuel,
    pub cpu: Fuel,
}

// END OF SERVICELOGGER TYPES
