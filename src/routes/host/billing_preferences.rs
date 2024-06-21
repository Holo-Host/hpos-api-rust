use std::fs::File;
use std::io::Read;
use std::time::Duration;

use anyhow::Result;
use holofuel_types::fuel::Fuel;
use rocket::{get, http::Status, serde::json::Json, State};
use serde::{Deserialize, Serialize};

use crate::hpos::WsMutex;

/// Returns list of all host invoices as needed for the host-console-ui invoice page
/// -- includes optional invoice set param to allow querying the invoices by their status
#[get("/billing_preferences")]
pub async fn billing_preferences(
    _wsm: &State<WsMutex>,
) -> Result<Json<PartialLoggerSettings>, (Status, String)> {
    Ok(Json(handle_billing_preferences().await.map_err(
        |e| (Status::InternalServerError, e.to_string()),
    )?))
}

async fn handle_billing_preferences() -> Result<PartialLoggerSettings> {
    // Open the YAML file
    let mut file = File::open(std::env::var("SL_PREFS_PATH")?)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let billing_preferences: PartialLoggerSettings = serde_yaml::from_str(&contents)?;

    Ok(billing_preferences)
}


// Copied from servicelogger-rsm. servicelogger_prefs.yaml contains a yaml encoding of this struct

#[derive(Serialize, Deserialize, Debug)]
pub struct PartialLoggerSettings {
    pub max_fuel_before_invoice: Fuel, // how much holofuel to accumulate before sending invoice
    pub price_compute: Fuel,           // In HF per unit TBD
    pub price_storage: Fuel,           // In HF per unit TBD
    pub price_bandwidth: Fuel,         // In HF per Byte of bandwidth used
    pub max_time_before_invoice: Duration, // how much time to allow to pass before sending invoice even if fuel trigger not reached
}