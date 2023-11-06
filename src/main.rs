mod consts;
mod hpos;
mod types;

use hpos::{Keystore, Ws, WsMutex};
use log::debug;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{self, get, launch, post, State};
use types::{HappAndHost, HappDetails};

#[get("/")]
async fn index() -> &'static str {
    "I'm your holoport 🤖"
}

// Rocket will return 400 if query params are of a wrong type
#[get("/hosted_happs?<usage_interval>&<quantity>")]
async fn get_all_hosted_happs(
    usage_interval: u32,
    quantity: Option<u32>,
    wsm: &State<WsMutex>,
) -> Result<Json<Vec<HappDetails>>, ApiError> {
    // Any anyhow error results in a 500 Respons Code
    Ok(Json(vec![]))
}

#[get("/hosted_happs/<id>")]
async fn get_hosted_happ(id: String, wsm: &State<WsMutex>) -> Result<&'static str, ApiError> {
    // Any anyhow error results in a 500 Respons Code
    // 404 if <id> not found
    // 400 for malformatted <id>
    Ok("I'm your holoport 🤖")
}

#[post("/hosted_happs/<id>/enable")]
async fn enable_happ(
    id: &'static str,
    wsm: &State<WsMutex>,
) -> Result<(), (Status, String)> {
    let mut ws = wsm.lock().await;
    let core_app_id = ws.core_app_id.clone();

    let payload = HappAndHost::init(id, &mut ws)
        .await
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    debug!("calling zome hha/enable_happ with payload: {:?}", &payload);
    let _: () = ws
        .call_zome(core_app_id, "hha", "hha", "enable_happ", payload)
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;
    Ok(())
}

#[post("/hosted_happs/<id>/disable")]
async fn disable_happ(
    id: &'static str,
    wsm: &State<WsMutex>,
) -> Result<(), (Status, String)> {
    let mut ws = wsm.lock().await;
    let core_app_id = ws.core_app_id.clone();

    let payload = HappAndHost::init(id, &mut ws)
        .await
        .map_err(|e| (Status::BadRequest, e.to_string()))?;

    debug!("calling zome hha/disable_happ with payload: {:?}", &payload);
    let _: () = ws
        .call_zome(core_app_id, "hha", "hha", "disable_happ", payload)
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;
    Ok(())
}

#[launch]
async fn rocket() -> _ {
    env_logger::init();

    let keystore = Keystore::init().await.unwrap();
    let wsm = WsMutex::new(Ws::connect(&keystore).await.unwrap());

    rocket::build().manage(wsm).mount(
        "/",
        rocket::routes![index, get_all_hosted_happs, get_hosted_happ, enable_happ, disable_happ],
    )
}
