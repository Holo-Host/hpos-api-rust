mod utils;

// use log::{debug, info};
use hpos_api_rust::rocket;
use hpos_api_rust::types::{HappAndHost, PresentedHappBundle};
use log::{debug, info};
use rocket::http::Status;
use rocket::local::asynchronous::Client;
use rocket::tokio;
use utils::core_apps::Happ;
use utils::Test;
use utils::{to_cell, HappInput};

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

    let test_hosted_happ_id = hha_bundle.id;
    info!(
        "Publushed hosted happ in hha with id {}",
        &test_hosted_happ_id
    );

    // enable test happ in hha
    let payload = HappAndHost {
        happ_id: test_hosted_happ_id.clone(),
        holoport_id: "5z1bbcrtjrcgzfm26xgwivrggdx1d02tqe88aj8pj9pva8l9hq".to_string(),
    };
    info!("payload: {:?}", payload);
    let _: () = test
        .call_zome(&hha_cell, "hha", "enable_happ", payload)
        .await;

    info!("Hosted happ enabled in hha - OK");

    // Install SL for hosted happ with host_agent key
    let sl_app_info = test.install_app(Happ::SL).await;
    debug!("sl_app_info: {:#?}", &sl_app_info);

    // Test API
    let client = Client::tracked(rocket().await)
        .await
        .expect("valid rocket instance");

    // Make some calls, starting with `/`
    info!("calling /");
    let response = client.get("/").dispatch().await;
    debug!("status: {}", &response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", &response_body);
    assert!(response_body.contains("5z1bbcrtjrcgzfm26xgwivrggdx1d02tqe88aj8pj9pva8l9hq"));

    // get all hosted happs
    let path = format!("/hosted_happs?usage_interval=5");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    assert!(response_body.contains(&format!("{}", &test_hosted_happ_id)));

    // disable test_hosted_happ_id
    let path = format!("/hosted_happs/{}/disable", &test_hosted_happ_id);
    info!("calling {}", &path);
    let response = client.post(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    debug!("body: {:#?}", response.into_string().await);

    // get one hosted happ
    let path = format!("/hosted_happs/{}", &test_hosted_happ_id);
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    assert!(response_body.contains(&format!("{}", &test_hosted_happ_id)));

    // enable test_hosted_happ_id
    let path = format!("/hosted_happs/{}/enable", &test_hosted_happ_id);
    info!("calling {}", &path);
    let response = client.post(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    debug!("body: {:#?}", response.into_string().await);

    // get one hosted happ
    let path = format!("/hosted_happs/{}", &test_hosted_happ_id);
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    assert!(response_body.contains(&format!("{}", &test_hosted_happ_id)));
}
