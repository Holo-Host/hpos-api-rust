use rocket::{
    http::Status,
    serde::json::Json,
    {get, State},
};
use serde::Serialize;
use crate::hpos::WsMutex;

#[derive(Serialize)]
pub struct VersionResponse {
    version: String,
}

/// Return an installed_app_id of a core app
#[get("/core/version")]
pub async fn version(wsm: &State<WsMutex>) -> Result<Json<VersionResponse>, (Status, String)> {
    let ws = wsm.lock().await;

    let response = VersionResponse {
        version: ws.core_app_id.clone(),
    };

    Ok(Json(response))
}