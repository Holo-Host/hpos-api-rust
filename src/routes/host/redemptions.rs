use anyhow::Result;
use holochain_types::{
    dna::{ActionHashB64, AgentPubKeyB64, EntryHashB64},
    prelude::Timestamp,
};
use hpos_hc_connect::{app_connection::CoreAppRoleName, AppConnection};
use rocket::{
    get,
    http::Status,
    serde::{json::Json, Deserialize, Serialize},
    State,
};

use crate::{
    common::types::{Transaction, TransactionDirection, TransactionStatus, TransactionType, POS},
    hpos::Ws,
};
use crate::{
    common::{
        hbs::HBS,
        types::{ProcessingStage, RedemptionRecord},
    },
    hpos::WsMutex,
};

use crate::routes::host::shared::PendingResponse;

/// ??
#[get("/redemptions")]
pub async fn redemptions(
    wsm: &State<WsMutex>,
) -> Result<Json<RedemptionsResponse>, (Status, String)> {
    let mut ws = wsm.lock().await;

    Ok(Json(handle_redemptions(&mut ws).await.map_err(|e| {
        (Status::InternalServerError, e.to_string())
    })?))
}

async fn handle_redemptions(ws: &mut Ws) -> Result<RedemptionsResponse> {
    let core_app_connection: &mut AppConnection =
        ws.get_connection(ws.core_app_id.clone()).await.unwrap();

    fn is_redemption(transaction: &Transaction) -> bool {
        if let Some(pos) = &transaction.proof_of_service {
            match pos {
                POS::Redemption(_) => true,
                POS::Hosting(_) => false,
            }
        } else {
            false
        }
    }

    let completed_redemption_transaction: Vec<Transaction> = core_app_connection
        .zome_call_typed::<(), Vec<Transaction>>(
            CoreAppRoleName::Holofuel.into(),
            "transactor".into(),
            "get_completed_transactions".into(),
            (),
        )
        .await?
        .into_iter()
        .filter(is_redemption)
        .collect();

    let completed_redemption_ids: Vec<EntryHashB64> = completed_redemption_transaction
        .clone()
        .into_iter()
        .map(|tx| tx.id)
        .collect();

    let completed_redemption_records: Vec<RedemptionRecord> =
        HBS::get_redemption_records(completed_redemption_ids).await?;

    let completed_transaction_with_redemptions: Vec<TransactionWithRedemption> =
        completed_redemption_transaction
            .into_iter()
            .map(|redemption_transaction| {
                let matching_record = completed_redemption_records
                    .clone()
                    .into_iter()
                    .find(|record| record.redemption_id == redemption_transaction.id);
                if let Some(matching_record) = matching_record {
                    let mut transaction_with_redemption: TransactionWithRedemption =
                        redemption_transaction.into();
                    transaction_with_redemption.holofuel_acceptance_hash =
                        Some(matching_record.holofuel_acceptance_hash);
                    transaction_with_redemption.ethereum_transaction_hash =
                        Some(matching_record.ethereum_transaction_hash);

                    // I don't understand why the logic is this way in the rest of this function. Just directly copying from the js
                    if matching_record.processing_stage == ProcessingStage::Finished {
                        return transaction_with_redemption;
                    }

                    transaction_with_redemption.status =
                        TransactionWithRedemptionStatus::HfTransferred;

                    transaction_with_redemption
                } else {
                    redemption_transaction.into()
                }
            })
            .collect();

    let PendingResponse {
        promise_pending,
        promise_declined,
        accepted,
        ..
    } = core_app_connection
        .zome_call_typed::<(), PendingResponse>(
            CoreAppRoleName::Holofuel.into(),
            "transactor".into(),
            "get_pending_transactions".into(),
            (),
        )
        .await?;

    let pending_redemption_transactions: Vec<Transaction> =
        promise_pending.into_iter().filter(is_redemption).collect();

    let declined_redemption_transactions: Vec<Transaction> =
        promise_declined.into_iter().filter(is_redemption).collect();

    let accepted_redemption_transactions: Vec<Transaction> =
        accepted.into_iter().filter(is_redemption).collect();

    Ok(RedemptionsResponse {
        pending: pending_redemption_transactions,
        declined: declined_redemption_transactions,
        accepted: accepted_redemption_transactions,
        completed: completed_transaction_with_redemptions,
    })
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct RedemptionsResponse {
    pending: Vec<Transaction>,
    declined: Vec<Transaction>,
    accepted: Vec<Transaction>,
    completed: Vec<TransactionWithRedemption>,
}

// This type is annoying, an artefact of translating directly from js (where variations on types is cheap) to rust.
// We might want to rethink the output of this endpoint now that we're in rust, so that we can clean up some of these types.

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct TransactionWithRedemption {
    pub id: EntryHashB64,
    pub amount: String,
    pub fee: String,
    pub created_date: Timestamp,
    pub completed_date: Option<Timestamp>,
    pub transaction_type: TransactionType, // The type returned will be the type of the initial transaction
    pub counterparty: AgentPubKeyB64,
    pub direction: TransactionDirection,
    pub status: TransactionWithRedemptionStatus,
    pub note: Option<String>,
    pub proof_of_service: Option<POS>,
    pub url: Option<String>,
    pub expiration_date: Option<Timestamp>,
    pub holofuel_acceptance_hash: Option<ActionHashB64>,
    pub ethereum_transaction_hash: Option<String>,
}

impl From<Transaction> for TransactionWithRedemption {
    fn from(transaction: Transaction) -> Self {
        TransactionWithRedemption {
            id: transaction.id,
            amount: transaction.amount,
            fee: transaction.fee,
            created_date: transaction.created_date,
            completed_date: transaction.completed_date,
            transaction_type: transaction.transaction_type,
            counterparty: transaction.counterparty,
            direction: transaction.direction,
            status: transaction.status.into(),
            note: transaction.note,
            proof_of_service: transaction.proof_of_service,
            url: transaction.url,
            expiration_date: transaction.expiration_date,
            holofuel_acceptance_hash: None,
            ethereum_transaction_hash: None,
        }
    }
}

// This is just TransactionStatus with one additional option.
// We might want to rethink the output of this endpoint now that we're in rust, so that we can clean up some of these types.

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub enum TransactionWithRedemptionStatus {
    Actionable, // tx that is create by 1st instance and waiting for counterparty to complete the tx
    Pending,    // tx that was created by 1st instance and second instance
    Accepted,   // tx that was accepted by counterparty but has yet to complete countersigning.
    Completed,
    Declined,
    Expired,
    HfTransferred,
}

impl From<TransactionStatus> for TransactionWithRedemptionStatus {
    fn from(status: TransactionStatus) -> Self {
        match status {
            TransactionStatus::Actionable => TransactionWithRedemptionStatus::Actionable,
            TransactionStatus::Pending => TransactionWithRedemptionStatus::Pending,
            TransactionStatus::Accepted => TransactionWithRedemptionStatus::Accepted,
            TransactionStatus::Completed => TransactionWithRedemptionStatus::Completed,
            TransactionStatus::Declined => TransactionWithRedemptionStatus::Declined,
            TransactionStatus::Expired => TransactionWithRedemptionStatus::Expired,
        }
    }
}
