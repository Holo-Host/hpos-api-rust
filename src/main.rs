mod types;

use rocket::{self, get, launch, State, post};
use rocket::serde::json::Json;
use types::{ApiError, Result, HappDetails};

#[get("/")]
async fn index() -> &'static str {
    "I'm your holoport 🤖"
}

// Rocket will return 400 if query params are of a wrong type
#[get("/hosted_happs?<usage_interval>&<quantity>")]
async fn get_all_hosted_happs(usage_interval: u32, quantity: Option<u32>, hpos: &State<HPOS>) -> Result<Json<Vec<HappDetails>>, ApiError> {
    // Any anyhow error results in a 500 Respons Code
    Ok(Json(vec![]))
}

#[get("/hosted_happs/<id>")]
async fn get_hosted_happ(id: String, hpos: &State<HPOS>) -> Result<Json<HappDetails>, ApiError> {
    // Any anyhow error results in a 500 Respons Code
    // 404 if <id> not found
    // 400 for malformatted <id>
    "I'm your holoport 🤖"
}

#[post("/hosted_happs/<id>/enable", format = "application/json", data = "<data>")]
async fn enable_happ(id: String, hpos: &State<HPOS>, data: &str) -> Result<&'static str, ApiError> {
    // Zome call to HHA's /enable_happ
    // Any anyhow error results in a 500 Respons Code
    Ok("")
}

#[post("/hosted_happs/<id>/disable", format = "application/json", data = "<data>")]
async fn disable_happ(id: String, hpos: &State<HPOS>, data: &str) -> Result<&'static str, ApiError> {
    // Zome call to HHA's /disable_happ
    // Any anyhow error results in a 500 Respons Code
    Ok("")
}

#[launch]
async fn rocket() -> _ {
    // Initialize HPOS struct
    // allows calling zomes in HHA (enable, disable), SL and Holofuel (hosted_happs details)
    let hpos = "";

    rocket::build().manage(hpos)
        .mount("/", rocket::routes![index, get_all_hosted_happs, get_hosted_happ, enable_happ, disable_happ])
}
