use hpos_hc_connect::AppConnection;
use rocket::{
    get, http::Status, serde::json::Json, State
};
use anyhow::Result;

use crate::hpos::WsMutex;
use crate::hpos::Ws;
use crate::routes::host::shared::{InvoiceSet, TransactionAndInvoiceDetails, HostingInvoicesResponse, get_hosting_invoices};

/// Returns list of all host invoices as needed for the host-console-ui invoice page
/// -- includes optional invoice set param to allow querying the invoices by their status
#[get("/invoices?<invoice_set>")]
pub async fn invoices(wsm: &State<WsMutex>, invoice_set: Option<InvoiceSet>) -> Result<Json<Vec<TransactionAndInvoiceDetails>>, (Status, String)> {

    let invoice_set = invoice_set.unwrap_or(InvoiceSet::All);

    let mut ws = wsm.lock().await;

    Ok(Json(handle_invoices(&mut ws, invoice_set)
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?)
    )
}

async fn handle_invoices(ws: &mut Ws, invoice_set: InvoiceSet) -> Result<Vec<TransactionAndInvoiceDetails>> {
    let core_app_connection: &mut AppConnection = ws.get_connection(ws.core_app_id.clone()).await.unwrap();

    let HostingInvoicesResponse {
        transaction_and_invoice_details,
        ..
    } = get_hosting_invoices(core_app_connection.to_owned(), invoice_set).await?;

    Ok(transaction_and_invoice_details)
}
