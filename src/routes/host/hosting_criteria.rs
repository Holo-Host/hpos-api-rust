use anyhow::Result;

use crate::common::hbs::{HbSMutex, HBS};
use rocket::{
    get,
    http::Status,
    serde::{json::Json, Deserialize, Serialize},
    State,
};

#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct HostingCriteriaResponse {
    id: String,
    kyc: String,
    jurisdiction: String,
}

/// Returns the hosting criteria of the holoport admin user as a json object
/// {
///     "id": "string",
///     "kyc": "string",
///     "jurisdiction": "string"
/// }
#[get("/hosting_criteria")]
pub async fn hosting_criteria(
    hbsm: &State<HbSMutex>,
) -> Result<Json<HostingCriteriaResponse>, (Status, String)> {
    let mut hbs = hbsm.lock().await;

    let hosting_criteria_response = handle_hosting_criteria(&mut hbs)
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    Ok(Json(hosting_criteria_response))
}

async fn handle_hosting_criteria(hbs: &mut HBS) -> Result<HostingCriteriaResponse> {
    let hbs_holo_client = hbs.download_holo_client().await?.clone();

    Ok(HostingCriteriaResponse {
        id: hbs_holo_client.id,
        kyc: hbs_holo_client.kyc,
        jurisdiction: hbs_holo_client.jurisdiction,
    })
}

/// Returns the kyc level of the holoport admin user as a string
#[get("/kyc_level")]
pub async fn kyc_level(hbsm: &State<HbSMutex>) -> Result<String, (Status, String)> {
    let mut hbs = hbsm.lock().await;
    let kyc_level = handle_kyc_level(&mut hbs)
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    Ok(kyc_level)
}

async fn handle_kyc_level(hbs: &mut HBS) -> Result<String> {
    Ok(hbs.download_holo_client().await?.kyc)
}
