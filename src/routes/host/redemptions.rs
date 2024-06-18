use std::str::FromStr;

use holochain_types::prelude::Timestamp;
use holofuel_types::{error::FuelError, fuel::Fuel};
use hpos_hc_connect::AppConnection;
use rocket::{
    get, http::Status, serde::{json::Json, Deserialize, Serialize}, State
};
use anyhow::{anyhow, Result};

use crate::{common::types::RedemptionState, hpos::WsMutex};
use crate::hpos::Ws;

use crate::routes::host::shared::{get_hosting_invoices, HostingInvoicesResponse, InvoiceSet, Ledger, Transaction, TransactionAndInvoiceDetails, PendingResponse};

/// ??
#[get("/redemptions")]
pub async fn redemptions(wsm: &State<WsMutex>) -> Result<Json<RedemptionResponse>, (Status, String)> {
    let mut ws = wsm.lock().await;

    Ok(Json(handle_redemptions(&mut ws)
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?)
    )
}

async fn handle_redemptions(ws: &mut Ws) -> Result<RedemptionResponse> {
    let core_app_connection: &mut AppConnection = ws.get_connection(ws.core_app_id.clone()).await.unwrap();

    fn is_redemption (transaction: &Transaction) -> bool {
        if let Some(pos) = &transaction.proof_of_service {
            match pos {
                crate::routes::host::shared::POS::Redemption(_) => true,
                crate::routes::host::shared::POS::Hosting(_) => false,
            }
        } else {
            false
        }
    }

    let completed_redemptions = core_app_connection.zome_call_typed::<(), Vec<Transaction>>(
        "holofuel".into(), 
        "transactor".into(), 
        "get_completed_transactions".into(), 
        ()
    ).await?
    .into_iter()
    .filter(is_redemption);

    let pending_transactions = core_app_connection.zome_call_typed::<(), PendingResponse>(
        "holofuel".into(), 
        "transactor".into(), 
        "get_pending_transactions".into(), 
        ()
    ).await?;

    Ok(RedemptionResponse{
        pending: todo!(),
        declined: todo!(),
        accepted: todo!(),
        completed: todo!(),
    })
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct RedemptionResponse
{
    pending: Vec<Transaction>,
    declined: Vec<Transaction>,
    accepted: Vec<Transaction>,
    completed: Vec<Transaction>,    
}