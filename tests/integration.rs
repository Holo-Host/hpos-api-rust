mod utils;

use std::collections::HashMap;
use std::time::Duration;

use holochain_types::dna::ActionHashB64;
use holochain_types::prelude::ExternIO;
use holofuel_types::fuel::Fuel;
use hpos_api_rust::handlers::hosted_apps::register;
use hpos_api_rust::rocket;
use hpos_api_rust::routes::apps::call_zome::ZomeCallRequest;

use hpos_api_rust::handlers::hosted_apps::install::{self, HappPreferences};
use hpos_hc_connect::hha_agent::HHAAgent;
use hpos_hc_connect::AppConnection;
use log::{debug, info};
use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use rocket::serde::json::{serde_json, Value};
use rocket::tokio;
use utils::core_apps::{Happ, HHA_URL};
use utils::{publish_and_enable_hosted_happ, HappInput, Test};

#[tokio::test]
async fn install_components() {
    env_logger::init();

    let mut test = Test::init().await;

    // Install hha
    let _ = test.install_app(Happ::HHA, None).await;

    // Connect to hha
    let mut hha = HHAAgent::spawn(None).await.unwrap();

    let hha_app_id = hha.app.id();

    // publish test happ to hha
    let hosted_happ_payload = HappInput::default();
    let test_hosted_happ_id = publish_and_enable_hosted_happ(&mut hha, hosted_happ_payload)
        .await
        .unwrap();

    // Install SL for hosted happ with host_agent key
    let sl_app_info = test
        .install_app(Happ::SL, Some(test_hosted_happ_id.clone()))
        .await;
    debug!("sl_app_info: {:#?}", &sl_app_info);

    // Open ws connection to servicelogger instance for hosted happ
    let mut sl_ws = AppConnection::connect(
        &mut test.admin_ws,
        test.hc_env.keystore.clone(),
        sl_app_info.installed_app_id,
    )
    .await
    .unwrap();

    // Generate some SL activity
    for _ in 1..10 {
        let payload = test.generate_sl_payload(&mut sl_ws).await;
        let sl_response: ActionHashB64 = sl_ws
            .zome_call_typed(
                "servicelogger".into(),
                "service".into(),
                "log_activity".into(),
                payload,
            )
            .await
            .unwrap();
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
    let path = format!("/apps/hosted?usage_interval=5");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    assert!(response_body.contains(&format!("{}", &test_hosted_happ_id)));

    // disable test_hosted_happ_id
    let path = format!("/apps/hosted/{}/disable", &test_hosted_happ_id);
    info!("calling {}", &path);
    let response = client.post(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    debug!("body: {:#?}", response.into_string().await);

    // get one hosted happ
    let path = format!("/apps/hosted/{}", &test_hosted_happ_id);
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    assert!(response_body.contains(&format!("{}", &test_hosted_happ_id)));

    // enable test_hosted_happ_id
    let path = format!("/apps/hosted/{}/enable", &test_hosted_happ_id);
    info!("calling {}", &path);
    let response = client.post(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    debug!("body: {:#?}", response.into_string().await);

    // get one hosted happ
    let path = format!("/apps/hosted/{}", &test_hosted_happ_id);
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    assert!(response_body.contains(&format!("{}", &test_hosted_happ_id)));

    // get service logs for happ
    let path = format!("/apps/hosted/{}/logs", &test_hosted_happ_id);
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);

    // get holofuel transaction history for 1 week
    let path = format!("/host/redeemable_histogram");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);

    // make zome call
    let path = "/apps/call_zome";
    info!("calling {}", &path);

    // Create correct zome call payload in form of a clear
    let mut payload = HappInput::default();
    payload.name = "Test123".to_string();
    payload.bundle_url = "Url123".to_string();

    let request = ZomeCallRequest {
        app_id: hha_app_id,
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
    debug!("decoded response body: {:?}", bundle);
    // Check if deserialized zome call result is correct
    assert_eq!(&bundle["name"], "Test123");
    assert_eq!(&bundle["bundle_url"], "Url123");

    // get core happ version
    let path = format!("/apps/core/version");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    assert_eq!(response_body, Happ::HHA.to_string());

    // Test installing a second hosted happ
    // publish second hosted happ
    let mut hosted_happ_payload = HappInput::default();
    hosted_happ_payload.name = "Hosted Happ 2".to_string();
    let second_test_hosted_happ_id = publish_and_enable_hosted_happ(&mut hha, hosted_happ_payload)
        .await
        .unwrap();

    // install new hosted happ on host's hp
    let path = format!("/apps/hosted/install");
    info!("calling {}", &path);
    let install_payload = install::InstallHappBody {
        happ_id: second_test_hosted_happ_id.to_string(),
        membrane_proofs: HashMap::new(),
        preferences: HappPreferences {
            max_fuel_before_invoice: Fuel::new(0),
            max_time_before_invoice: Duration::MAX,
            price_bandwidth: Fuel::new(0),
            price_compute: Fuel::new(0),
            price_storage: Fuel::new(0),
        },
    };
    let response = client
        .post(path)
        .body(serde_json::to_string(&install_payload).unwrap())
        .header(ContentType::JSON)
        .dispatch()
        .await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);

    // get second hosted happ
    let path = format!("/apps/hosted/{}", &test_hosted_happ_id);
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    assert!(response_body.contains(&format!("{}", &second_test_hosted_happ_id)));

    // Test registering with a third hosted happ
    // register a third hosted happ
    let path = format!("/apps/hosted/register");
    info!("calling {}", &path);
    let register_payload = register::HappInput {
        hosted_urls: ["test_happ_3_host_url".to_string()],
        bundle_url: HHA_URL.to_string(),
        special_installed_app_id: None,
        name: "Test Happ 3".to_string(),
        dnas: vec![HHA_URL],
        exclude_jurisdictions: false,
    };
    let response = client
        .post(path)
        .body(serde_json::to_string(&register_payload).unwrap())
        .header(ContentType::JSON)
        .dispatch()
        .await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);

    // get second hosted happ
    let path = format!("/apps/hosted/{}", &test_hosted_happ_id);
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    assert!(response_body.contains(&format!("{}", &second_test_hosted_happ_id)));
}
