mod utils;

// use log::{debug, info};
use hpos_api_rust::rocket;
use log::{debug, info};
use rocket::{local::blocking::Client, tokio};
use utils::Test;
use utils::{to_cell, Happ, HappAndHost, HappInput, PresentedHappBundle};

#[tokio::test]
async fn install_components() {
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

    let test_hosted_happ_id = hha_bundle.id;

    // enable test happ in hha
    let payload = HappAndHost {
        happ_id: test_hosted_happ_id,
        holoport_id: "my_holoport".to_string(),
    };
    let _: () = test
        .call_zome(&hha_cell, "hha", "enable_happ", payload)
        .await;

    info!("Hosted happ enabled in hha - OK");

    // Install SL for hosted happ with host_agent key
    let sl_app_info = test.install_app(Happ::SL).await;
    debug!("sl_app_info: {:#?}", &sl_app_info);

    // Test API
    let client = Client::tracked(rocket().await).expect("valid rocket instance");

    // Make some calls, starting with `/`
    let mut response = client.get("/").dispatch();

    info!("status: {}", response.status());
    info!("body: {:#?}", response.into_string());

    // enable test_hosted_happ_id

    // disable test_hosted_happ_id
}
