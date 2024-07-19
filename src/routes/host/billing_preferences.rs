use crate::hpos::WsMutex;
use anyhow::Result;
use hpos_hc_connect::{hha_agent::CoreAppAgent, hha_types::HappPreferences};
use rocket::{get, http::Status, serde::json::Json, State};

/// Returns list of all host invoices as needed for the host-console-ui invoice page
/// -- includes optional invoice set param to allow querying the invoices by their status
#[get("/billing_preferences")]
pub async fn billing_preferences(
    _wsm: &State<WsMutex>,
) -> Result<Json<HappPreferences>, (Status, String)> {
    Ok(Json(handle_billing_preferences().await.map_err(|e| {
        (Status::InternalServerError, e.to_string())
    })?))
}

async fn handle_billing_preferences() -> Result<HappPreferences> {
    // make a call to hha and get the default preferences
    let mut hha = CoreAppAgent::spawn(None).await?;
    let happ_preference = hha.get_host_preferences().await?;

    Ok(happ_preference)
}
