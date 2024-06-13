use rocket::{
    http::Status,
    serde::json::Json,
    {get, State},
};

use crate::hpos::WsMutex;


/// Returns list of all host invoices as needed for the host-console-ui invoice page
/// -- includes optional invoice_set {all, unpaid, paid} param to allow querying the invoices by their status
#[get("/invoices?<invoice_set>")]
pub async fn get_host_invoices(
    wsm: &State<WsMutex>,
    invoice_set: String,
) -> Result<Json<()>, (Status, String)> {
    let mut ws = wsm.lock().await;

    Ok(Json(()))
}
