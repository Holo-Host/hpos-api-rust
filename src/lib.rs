pub mod common;
pub mod handlers;
mod hpos;
pub mod routes;
mod types;

use hpos::Ws;
use log::debug;
use rocket::{self, Build, Rocket, routes};

use routes::index;
use routes::apps::hosted::*;
use crate::routes::apps::zome_call::*;
use crate::routes::apps::core::*;

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
        routes![
            index,
        ],
    ).mount(
        "/apps",
        routes![
            get_all_hosted_happs,
            get_hosted_happ_by_id,
            enable_happ,
            disable_happ,
            zome_call,
            get_service_logs,
            core_app_version
        ],
    )
}
