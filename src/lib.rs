pub mod consts;
mod handlers;
mod hpos;
pub mod types;

use handlers::{handle_get_all, handle_get_one};
use holochain_types::dna::ActionHashB64;
use holochain_types::prelude::{Entry, Record, RecordEntry};
use hpos::{Keystore, Ws, WsMutex};
use log::debug;
use rocket::http::Status;
use rocket::serde::json::{Json, Value};
use rocket::{self, get, post, Build, Rocket, State};
use std::time::{SystemTime, UNIX_EPOCH};
use types::{HappAndHost, HappDetails, ZomeCallRequest, ZomeCallResponse};

use crate::types::{ActivityLog, DiskUsageLog, LogEntry};

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

// Rocket will return 400 if query params are of a wrong type
#[get("/hosted_happs?<usage_interval>&<quantity>")]
async fn get_all_hosted_happs(
    usage_interval: i64,
    quantity: Option<usize>,
    wsm: &State<WsMutex>,
) -> Result<Json<Vec<HappDetails>>, (Status, String)> {
    let mut ws = wsm.lock().await;

    Ok(Json(
        handle_get_all(usage_interval, quantity, &mut ws)
            .await
            .map_err(|e| (Status::InternalServerError, e.to_string()))?,
    ))
}

#[get("/hosted_happs/<id>?<usage_interval>")]
async fn get_hosted_happ(
    id: String,
    usage_interval: Option<i64>,
    wsm: &State<WsMutex>,
) -> Result<Json<HappDetails>, (Status, String)> {
    let mut ws = wsm.lock().await;

    // Validate format of happ id
    let id = ActionHashB64::from_b64_str(&id).map_err(|e| (Status::BadRequest, e.to_string()))?;
    let usage_interval = usage_interval.unwrap_or(7); // 7 days
    Ok(Json(
        handle_get_one(id, usage_interval, &mut ws)
            .await
            .map_err(|e| (Status::InternalServerError, e.to_string()))?,
    ))
}

#[post("/hosted_happs/<id>/enable")]
async fn enable_happ(id: &str, wsm: &State<WsMutex>) -> Result<(), (Status, String)> {
    let mut ws = wsm.lock().await;
    let core_app_id = ws.core_app_id.clone();

    let payload = HappAndHost::init(id, &mut ws)
        .await
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    debug!("calling zome hha/enable_happ with payload: {:?}", &payload);
    ws.call_zome(core_app_id, "core-app", "hha", "enable_happ", payload)
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;
    Ok(())
}

#[post("/hosted_happs/<id>/disable")]
async fn disable_happ(id: &str, wsm: &State<WsMutex>) -> Result<(), (Status, String)> {
    let mut ws = wsm.lock().await;
    let core_app_id = ws.core_app_id.clone();

    let payload = HappAndHost::init(id, &mut ws)
        .await
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    debug!("calling zome hha/disable_happ with payload: {:?}", &payload);
    ws.call_zome(core_app_id, "core-app", "hha", "disable_happ", payload)
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;
    Ok(())
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

#[get("/hosted_happs/<id>/logs?<days>")]
async fn get_service_logs(
    id: &str,
    days: Option<i32>,
    wsm: &State<WsMutex>,
) -> Result<Json<Vec<LogEntry>>, (Status, String)> {
    let mut ws = wsm.lock().await;

    // Validate format of happ id
    let id = ActionHashB64::from_b64_str(id).map_err(|e| (Status::BadRequest, e.to_string()))?;
    let days = days.unwrap_or(7); // 7 days
    let filter = holochain_types::prelude::ChainQueryFilter::new().include_entries(true);

    log::debug!("getting logs for happ: {}::servicelogger", id);
    let result: Vec<Record> = ws
        .call_zome(
            format!("{}::servicelogger", id),
            "servicelogger",
            "service",
            "querying_chain",
            filter,
        )
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let four_weeks_ago = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
        - (days as u64 * 24 * 60 * 60)) as i64;

    log::debug!("filtering logs from {}", id);

    let filtered_result: Vec<LogEntry> = result
        .into_iter()
        .filter(|record| record.action().timestamp().as_seconds_and_nanos().0 > four_weeks_ago)
        // include only App Entries (those listed in #[hdk_entry_defs] in DNA code),
        // not holochain system entries
        // and deserialize them into service logger's entries
        .filter_map(|record| {
            if let RecordEntry::Present(Entry::App(bytes)) = record.entry() {
                if let Ok(log_entry) = ActivityLog::try_from(bytes.clone().into_sb()) {
                    return Some(LogEntry::ActivityLog(Box::new(log_entry)));
                } else if let Ok(log_entry) = DiskUsageLog::try_from(bytes.clone().into_sb()) {
                    return Some(LogEntry::DiskUsageLog(log_entry));
                }
            }
            None
        })
        .collect();

    Ok(Json(filtered_result))
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
            get_service_logs
        ],
    )
}
