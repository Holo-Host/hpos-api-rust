pub mod common;
pub mod handlers;
mod hpos;
pub mod routes;

use hpos::Ws;
use log::debug;
use rocket::{self, routes, Build, Rocket};

use routes::apps::call_zome::*;
use routes::apps::core::*;
use routes::apps::hosted::*;
use routes::holoport::usage::*;
use routes::host::earnings::*;
use routes::host::hosting_criteria::*;
use routes::host::invoices::*;
use routes::host::redeemable_histogram::*;
use routes::host::redemptions::*;
use routes::index;

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

    rocket::build()
        .manage(ws)
        .mount(
            "/",
            routes![
                index,     // done
            ],
        )
        .mount(
            "/apps",
            routes![
                get_all,      // done
                get_by_id,    // done
                enable,       // done
                disable,      // done
                call_zome,    // done
                logs,         // done
                version,      // done
                install_app,  // done
                register_app  // done
            ],
        )
        .mount(
            "/host",
            routes![
                earnings,             // done
                invoices,             // done
                redeemable_histogram, // done
                kyc_level,            // TODO!!
                hosting_criteria,     // TODO!!
                redemptions           // TODO!!
            ],
        )
        .mount(
            "/holoport",
            routes![
                usage,     // TODO!!
            ],
        )
}
