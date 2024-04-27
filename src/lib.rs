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
use rocket::serde::json::{Json, Value};
use rocket::{self, get, post, Build, Rocket, State};
use types::{RedemableHolofuelHistogramResponse, ZomeCallRequest, ZomeCallResponse};

use routes::hosted_happs::*;

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

#[post("/zome_call", format = "json", data = "<data>")]
async fn zome_call(
    data: Json<ZomeCallRequest>,
    wsm: &State<WsMutex>,
) -> Result<ZomeCallResponse, (Status, String)> {
    let mut ws = wsm.lock().await;

    // arguments of ws.zome_call require 'static lifetime and data is only temporary
    // so I need to extend lifetime with Box::leak
    let data = Box::leak(Box::new(data.into_inner()));

    let res = ws
        .call_zome_raw::<Value>(
            data.app_id.clone(),
            &data.role_id,
            &data.zome_name,
            &data.fn_name,
            data.payload.clone(),
        )
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    // same here as above - extending lifetime to 'static with Box::leak
    let res = Box::leak(Box::new(res));

    Ok(ZomeCallResponse(res.as_bytes()))
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
