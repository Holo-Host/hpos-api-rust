pub mod consts;
mod handlers;
mod hpos;
pub mod routes;
pub mod types;

mod handlers_old;
use handlers_old::{get_last_weeks_redeemable_holofuel, get_redeemable_holofuel};

use hpos::{Keystore, Ws, WsMutex};
use log::debug;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{self, get, Build, Rocket, State};

use routes::hosted_happs::*;
use routes::zome_call::*;
use types::RedemableHolofuelHistogramResponse;

#[get("/")]
async fn index(wsm: &State<WsMutex>) -> String {
    let mut ws = wsm.lock().await;

    // Construct sample HappAndHost just to retrieve holoport_id
    let sample = HappAndHost::init(
        "uhCkklkJVx4u17eCaaKg_phRJsHOj9u57v_4cHQR-Bd9tb-vePRyC",
        &mut ws,
    )
    .await
    .unwrap();

    format!("🤖 I'm your holoport {}", sample.holoport_id)
}

#[get("/holofuel_redeemable_for_last_week")]
async fn get_redeemable_holofuel_request(
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

pub async fn rocket() -> Rocket<Build> {
    if let Err(e) = env_logger::try_init() {
        debug!(
            "Looks like env logger is already initialized {}. Maybe in testing harness?",
            e
        );
    };

    let keystore = Keystore::init().await.unwrap();
    let wsm = WsMutex::new(Ws::connect(&keystore).await.unwrap());

    rocket::build().manage(wsm).mount(
        "/",
        rocket::routes![
            index,
            get_all_hosted_happs,
            get_hosted_happ,
            enable_happ,
            disable_happ,
            zome_call,
            get_service_logs,
            get_redeemable_holofuel_request
        ],
    )
}
