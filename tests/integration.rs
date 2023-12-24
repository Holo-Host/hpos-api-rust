mod utils;

use holochain_types::dna::ActionHashB64;
// use log::{debug, info};
use hpos_api_rust::rocket;
use hpos_api_rust::types::{HappAndHost, PresentedHappBundle, ZomeCallRequest};
use log::{debug, info};
use rocket::http::{Status, ContentType};
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


    // make zome call
    let path = "/zome_call";
    info!("calling {} hha create draft", &path);

    // Create correct zome call payload in form of a clear
    let mut payload = HappInput::default();
    payload.name = "Test123".to_string();
    payload.bundle_url = "Url123".to_string();

    let request = ZomeCallRequest {
        app_id: hha_installed_app_id.clone(),
        role_id: "core-app".to_string(),
        zome_name: "hha".to_string(),
        fn_name: "create_draft".to_string(),
        payload: serde_json::from_str(&serde_json::to_string(&payload).unwrap()).unwrap(),
    };

    debug!("request: {:?}", request);

    let response = client
        .post(path)
        .body(serde_json::to_string(&request).unwrap())
        .header(ContentType::JSON)
        .dispatch()
        .await;

    debug!("response: {:?}", response);

    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    // Check if deserialized zome call result is correct
    let bundle: Value = serde_json::from_str(&response_body).unwrap();
    assert_eq!(&bundle["name"], "Test123");
    assert_eq!(&bundle["bundle_url"], "Url123");

    // make zome call
    let path = "/zome_call";
    info!("calling {} hha ", &path);

    let request = ZomeCallRequest {
        app_id: hha_installed_app_id,
        role_id: "holofuel".to_string(),
        zome_name: "profile".to_string(),
        fn_name: "get_my_profile".to_string(),
        payload: serde_json::from_str("").unwrap(),
    };

    debug!("request: {:?}", request);

    let response = client
        .post(path)
        .body(serde_json::to_string(&request).unwrap())
        .header(ContentType::JSON)
        .dispatch()
        .await;

    debug!("response: {:?}", response);

    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    // Check if deserialized zome call result is correct
    // let bundle: Value = serde_json::from_str(&response_body).unwrap();
    // assert_eq!(&bundle["name"], "Test123");
    // assert_eq!(&bundle["bundle_url"], "Url123");
}
