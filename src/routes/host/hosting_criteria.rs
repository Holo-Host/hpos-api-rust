use rocket::{
    http::Status,
    serde::json::Json,
    {get, State},
};

use crate::hpos::WsMutex;

/// ???
#[get("/hosting_criteria")]
pub async fn hosting_criteria(wsm: &State<WsMutex>) -> Result<Json<()>, (Status, String)> {
    let mut ws = wsm.lock().await;

    Ok(Json(()))
}

/// ???
#[get("/kyc_level")]
pub async fn kyc_level(wsm: &State<WsMutex>) -> Result<Json<()>, (Status, String)> {
    let mut ws = wsm.lock().await;

    Ok(Json(()))
}
