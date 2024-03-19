mod utils;

use holochain_types::dna::ActionHashB64;
use holochain_types::prelude::ExternIO;
// use log::{debug, info};
use hpos_api_rust::rocket;
use hpos_api_rust::types::{HappAndHost, PresentedHappBundle, ZomeCallRequest};
use log::{debug, info};
use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use rocket::serde::json::{serde_json, Value};
use rocket::tokio;
use utils::core_apps::Happ;
use utils::Test;
use utils::{to_cell, HappInput};

#[tokio::test]
async fn install_components() {
    env_logger::init();

    let mut test = Test::init().await;

    let hha_app_info = test.install_app(Happ::HHA, None).await;
    let hha_installed_app_id = hha_app_info.installed_app_id.clone();
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
        "Published hosted happ in hha with id {}",
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
    let sl_app_info = test
        .install_app(Happ::SL, Some(test_hosted_happ_id.clone()))
        .await;
    debug!("sl_app_info: {:#?}", &sl_app_info);

    // Generate some SL activity
    let sl_cell = to_cell(sl_app_info, "servicelogger");
    for _ in 1..10 {
        let payload = test.generate_sl_payload(&sl_cell).await;
        let sl_response: ActionHashB64 = test
            .call_zome(&sl_cell, "service", "log_activity", payload)
            .await;
        debug!("logged activity: {}", sl_response);
    }

    // Test API
    let client = Client::tracked(rocket().await)
        .await
        .expect("valid rocket instance");

    // List all avail routes
    debug!("available routes:");
    for route in client.rocket().routes() {
        debug!("{}", route);
    }

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

    // get service logs for happ
    let path = format!("/hosted_happs/{}/logs", &test_hosted_happ_id);
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);

    // get holofuel transaction history for 1 week
    let path = format!("/holofuel_redeemable_for_last_week");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);

    // make zome call
    let path = "/zome_call";
    info!("calling {}", &path);

    // Create correct zome call payload in form of a clear
    let mut payload = HappInput::default();
    payload.name = "Test123".to_string();
    payload.bundle_url = "Url123".to_string();

    let request = ZomeCallRequest {
        app_id: hha_installed_app_id,
        role_id: "core-app".to_string(),
        zome_name: "hha".to_string(),
        fn_name: "create_draft".to_string(),
        payload: serde_json::from_str(&serde_json::to_string(&payload).unwrap()).unwrap(),
    };

    let response = client
        .post(path)
        .body(serde_json::to_string(&request).unwrap())
        .header(ContentType::JSON)
        .dispatch()
        .await;

    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);

    let response_body = response.into_bytes().await.unwrap();
    debug!("raw response body: {:?}", response_body);
    // decode with ExternIO
    let bundle: Value = ExternIO::decode(&ExternIO::from(response_body)).unwrap();
    // Check if deserialized zome call result is correct
    assert_eq!(&bundle["name"], "Test123");
    assert_eq!(&bundle["bundle_url"], "Url123");
}
