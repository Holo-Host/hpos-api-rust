use rocket::{
    http::Status,
    {get, State},
};

use crate::hpos::WsMutex;

/// Return an installed_app_id of a core app
#[get("/core/version")]
pub async fn version(wsm: &State<WsMutex>) -> Result<String, (Status, String)> {
    let ws = wsm.lock().await;

    Ok(ws.core_app_id.clone())
}
