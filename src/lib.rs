pub mod common;
pub mod handlers;
mod hpos;
pub mod routes;

use hpos::Ws;
use log::debug;
use rocket::{self, Build, Rocket};

use routes::holofuel_redeemable_for_last_week::*;
use routes::hosted_happs::*;
use routes::zome_call::*;
use routes::core_app_version::*;

pub async fn rocket() -> Rocket<Build> {
    if let Err(e) = env_logger::try_init() {
        debug!(
            "Looks like env logger is already initialized {}. Maybe in testing harness?",
            e
        );
    };

    let ws = Ws::connect()
        .await
        .expect("Failed to connect to lair kystore or holochain");

    rocket::build().manage(ws).mount(
        "/",
        rocket::routes![
            index,
            get_all_hosted_happs,
            get_hosted_happ,
            enable_happ,
            disable_happ,
            zome_call,
            get_service_logs,
            get_redeemable_holofuel_request,
            core_app_version
        ],
    )
}
