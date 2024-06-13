pub mod common;
pub mod handlers;
mod hpos;
pub mod routes;

use hpos::Ws;
use log::debug;
use rocket::{self, Build, Rocket, routes};

use routes::index;
use routes::apps::hosted::*;
use routes::apps::call_zome::*;
use routes::apps::core::*;
use routes::host::earnings::*;
use routes::host::invoices::*;
use routes::host::redeemable_histogram::*;

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
            get_all,
            get_by_id,
            enable,
            disable,
            call_zome,
            logs,
            version
        ],
    ).mount(
        "/host",
        routes![
            earnings,
            invoices,
            redeemable_histogram,
            disable,
            call_zome,
            logs,
            version
        ],
    )
}
