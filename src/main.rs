mod types;
mod hpos;
mod consts;

use rocket::{self, get, launch, State, post};
use rocket::serde::json::Json;
use types::{ApiError, Result, HappDetails, HappAndHost};
use hpos::{Keystore, Ws};

#[get("/")]
async fn index() -> &'static str {
    "I'm your holoport 🤖"
}

// Rocket will return 400 if query params are of a wrong type
#[get("/hosted_happs?<usage_interval>&<quantity>")]
async fn get_all_hosted_happs(usage_interval: u32, quantity: Option<u32>, keystore: &State<Keystore>) -> Result<Json<Vec<HappDetails>>, ApiError> {
    // Any anyhow error results in a 500 Respons Code
    Ok(Json(vec![]))
}

#[get("/hosted_happs/<id>")]
async fn get_hosted_happ(id: String, keystore: &State<Keystore>) -> Result<&'static str, ApiError> {
    // Any anyhow error results in a 500 Respons Code
    // 404 if <id> not found
    // 400 for malformatted <id>
    Ok("I'm your holoport 🤖")
}

#[post("/hosted_happs/<id>/enable", format = "application/json", data = "<data>")]
// Json will try to deserialize data to HappAndHost and if it fails it will return 400
async fn enable_happ(id: String, keystore: &State<Keystore>, data: Json<HappAndHost>) -> Result<String, ApiError> {
    let mut a = keystore;
    // Any anyhow error results in a 500 Response Code
    let mut ws = Ws::connect(keystore).await.unwrap();

    let payload = HappAndHost {
        happ_id: todo!(),
        holoport_id: todo!(),
    };
    let _: () = ws.call_zome(ws.core_app_id.clone(), "hha", "hha", "enable_happ", ()).await.unwrap();
    Ok("".into())
}

#[post("/hosted_happs/<id>/disable", format = "application/json", data = "<data>")]
async fn disable_happ(id: String, keystore: &State<Keystore>, data: &str) -> Result<&'static str, ApiError> {
    // Check format of id first, return 400 if not ok

    // Any anyhow error results in a 500 Response Code
    let mut ws = Ws::connect(keystore).await.unwrap();
    let _: () = ws.call_zome(ws.core_app_id.clone(), "hha", "hha", "disable_happ", ()).await.unwrap();
    Ok("")
}

#[launch]
async fn rocket() -> _ {
    let keystore = Keystore::init();

    rocket::build().manage(keystore.await.unwrap())
        .mount("/", rocket::routes![index, get_all_hosted_happs, get_hosted_happ, enable_happ, disable_happ])
}
