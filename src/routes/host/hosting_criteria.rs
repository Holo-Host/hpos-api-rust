use rocket::{
    http::Status,
    serde::json::Json,
    {get, State},
};

use crate::hpos::WsMutex;
use crate::common::hbs::call_hbs;
use crate::routes::host::auth_payload::auth_payload;

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

    let email = String::from("example@example.com");
    let pub_key = String::from("public_key_example");

    let payload = auth_payload::new(email, pub_key);

    Ok(Json(()))
}
