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
use serde::{Deserialize, Serialize};
use utils::core_apps::Happ;
use utils::Test;
use utils::{to_cell, HappInput};

#[derive(Debug, Serialize)]
pub struct SignalPayload {
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestObj {
    pub value: String,
}

#[tokio::test]
async fn install_components() {
    env_logger::init();

    let mut test = Test::init().await;

    // hha is installed here only to show that installation and zome calls work before calling signals
    // in test happ
    let hha_app_info = test.install_app(Happ::HHA, None).await;
    let hha_installed_app_id = hha_app_info.installed_app_id.clone();
    // let hha_cell = to_cell(hha_app_info, "core-app");

    // Install dummy dna so that we can call signal emitting endpoint
    let dummy_dna_info = test.install_app(Happ::DummyDna, None).await;
    let dummy_dna_cell = to_cell(dummy_dna_info, "test");

    let payload = TestObj {
        value: "some_value".into(),
    };

    let response: TestObj = test
        .call_zome(&dummy_dna_cell, "test", "pass_obj", payload)
        .await;

    println!("{:?}", response);

    let payload = SignalPayload {
        value: "some_signal".into(),
    };

    let response: () = test
        .call_zome(&dummy_dna_cell, "test", "signal_loopback", payload)
        .await;

    println!("{:?}", response);

    // API calls are here only to check if it is possible to connect to holochain's websocket
    // interface after calling signals
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
