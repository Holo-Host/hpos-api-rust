use rocket::{self, get, launch, State, post};

#[get("/")]
async fn index() -> &'static str {
    "I'm your holoport 🤖"
}

#[get("/hosted_happs?<usage_interval>")]
async fn get_all_hosted_happs(usage_interval: u64, hpos: &State<HPOS>) -> &'static str {
    "I'm your holoport 🤖"
}

#[get("/hosted_happs/<id>")]
async fn get_hosted_happ(id: String, hpos: &State<HPOS>) -> &'static str {
    "I'm your holoport 🤖"
}

#[post("/hosted_happs/<id>/enable", format = "application/json", data = "<data>")]
async fn enable_happ(id: String, hpos: &State<HPOS>, data: &str) -> &'static str {
    "I'm your holoport 🤖"
}

#[post("/hosted_happs/<id>/disable", format = "application/json", data = "<data>")]
async fn disable_happ(id: String, hpos: &State<HPOS>, data: &str) -> &'static str {
    "I'm your holoport 🤖"
}

#[launch]
async fn rocket() -> _ {
    // Initialize HPOS struct
    // allows calling zomes in HHA (enable, disable), SL and Holofuel (hosted_happs details)
    let hpos = "";

    rocket::build().manage(hpos)
        .mount("/", rocket::routes![index, get_all_hosted_happs, get_hosted_happ])
}
