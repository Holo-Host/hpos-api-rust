use crate::hpos::WsMutex;
use holochain_types::prelude::{holochain_serial, SerializedBytes};
use holofuel_types::fuel::Fuel;
use rocket::{
    http::Status,
    serde::{json::Json, Deserialize, Serialize},
    {get, State},
};

use crate::handlers::holofuel_redeemable_for_last_week::*;

#[get("/redeemable_histogram")]
pub async fn redeemable_histogram(
    wsm: &State<WsMutex>,
) -> Result<Json<RedemableHolofuelHistogramResponse>, (Status, String)> {
    let mut ws = wsm.lock().await;
    let holofuel = get_redeemable_holofuel(&mut ws)
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;
    let dailies = get_last_weeks_redeemable_holofuel(&mut ws)
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;
    Ok(Json(RedemableHolofuelHistogramResponse {
        dailies,
        redeemed: holofuel.available,
    }))
}

#[derive(Serialize, Deserialize, Debug, SerializedBytes, Clone)]
pub struct HolofuelPaidUnpaid {
    pub date: String,
    pub paid: Fuel,
    pub unpaid: Fuel,
}

#[derive(Serialize, Deserialize, Debug, SerializedBytes, Clone)]
pub struct RedemableHolofuelHistogramResponse {
    pub dailies: Vec<HolofuelPaidUnpaid>,
    pub redeemed: Fuel,
}
