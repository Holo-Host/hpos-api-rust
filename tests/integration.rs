mod utils;

// use log::{debug, info};
use rocket::tokio;
use utils::Test;
use log::{debug, info};

use crate::utils::{Happ, to_cell, PresentedHappBundle, HappInput, HappAndHost};

#[tokio::test]
async fn install_components() {
    env_logger::init();

    let mut test = Test::init().await;

    let hha_app_info = test.install_app(Happ::HHA).await;
    let hha_cell = to_cell(hha_app_info, "core-app");

    // publish test happ to hha
    // howto: https://github.com/Holo-Host/holo-hosting-app-rsm/blob/develop/tests/unit-test/provider-init.ts#L52
    let payload = HappInput::default();
    let draft_hha_bundle: PresentedHappBundle = test
        .call_zome(&hha_cell, "hha", "create_draft", payload)
        .await;
    let payload = draft_hha_bundle.id;
    let hha_bundle: PresentedHappBundle = test
        .call_zome(&hha_cell, "hha", "publish_happ", payload)
        .await;

    // enable test happ in hha
    let payload = HappAndHost {
        happ_id: hha_bundle.id,
        holoport_id: "my_holoport".to_string(),
    };
    let _: () = test
        .call_zome(&hha_cell, "hha", "enable_happ", payload)
        .await;

    info!("Hosted happ enabled in hha - OK");

    // I do not have to actually install hosted happ because I am insterested only in service logger records

    // Install SL for hosted happ with host_agent key
    let sl_app_info = test.install_app(Happ::SL).await;
    debug!("sl_app_info: {:#?}", &sl_app_info);

    // Start API

    // Make some calls, starting with `/`
}