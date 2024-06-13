use rocket::{
    http::Status,
    serde::json::Json,
    {get, State},
};

use crate::hpos::WsMutex;

/// ??
#[get("/redemptions")]
pub async fn redemptions(wsm: &State<WsMutex>) -> Result<Json<()>, (Status, String)> {
    let mut ws = wsm.lock().await;

    Ok(Json(()))
}
