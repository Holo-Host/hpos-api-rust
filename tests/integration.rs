mod utils;

use holochain_types::dna::ActionHashB64;
use holochain_types::prelude::ExternIO;
use hpos_api_rust::common::types::HappAndHost;
// use log::{debug, info};
use hpos_api_rust::rocket;
use hpos_api_rust::routes::apps::call_zome::ZomeCallRequest;
use hpos_api_rust::routes::apps::hosted::{HostedRegisterRequestBody, PresentedHappBundle};
use hpos_hc_connect::app_connection::CoreAppRoleName;
use hpos_hc_connect::hha_agent::HHAAgent;
use hpos_hc_connect::AppConnection;
use log::{debug, info};
use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use rocket::serde::json::{serde_json, Value};
use rocket::tokio;
use utils::core_apps::{Happ, HHA_URL};
use utils::HappInput;
use utils::Test;

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
    // howto: https://github.com/Holo-Host/holo-hosting-app-rsm/blob/develop/tests/unit-test/provider-init.ts#L52
    let payload = HappInput::default();
    let draft_hha_bundle: PresentedHappBundle = hha
        .app
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            "hha".into(),
            "create_draft".into(),
            payload,
        )
        .await
        .unwrap();

    let payload = draft_hha_bundle.id;
    let hha_bundle: PresentedHappBundle = hha
        .app
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            "hha".into(),
            "publish_happ".into(),
            payload,
        )
        .await
        .unwrap();

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

    debug!("payload: {:?}", payload);
    let _: () = hha
        .app
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            "hha".into(),
            "enable_happ".into(),
            payload,
        )
        .await
        .unwrap();

    info!("Hosted happ enabled in hha - OK");

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

    // get earnings report
    let path = format!("/host/earnings");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    assert_eq!(response_body, "{\"earnings\":{\"last30days\":\"0\",\"last7days\":\"0\",\"lastday\":\"0\"},\"holofuel\":{\"redeemable\":\"0\",\"balance\":\"0\",\"available\":\"0\"},\"recentPayments\":[]}");

    // get invoices report
    let path = format!("/host/invoices");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    assert_eq!(response_body, "[]");

    //  get usage report
    let path = format!("/holoport/usage?usage_interval=5");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    assert_eq!(response_body, "{\"totalHostedAgents\":0,\"currentTotalStorage\":0,\"totalHostedHapps\":1,\"totalUsage\":{\"cpu\":108,\"bandwidth\":108}}");

    // apps/hosted/register
    let payload = HostedRegisterRequestBody {
        name: "Hosted Happ 2".to_string(),
        bundle_url: HHA_URL.to_string(),
        hosted_urls: Vec::new(),
        dnas: Vec::new(),
        network_seed: Some("random-uid".to_string()),
        special_installed_app_id: None,
    };
    let path = format!("/apps/hosted/register");
    info!("calling {}", &path);
    let response = client
        .post(path)
        .body(serde_json::to_string(&payload).unwrap())
        .header(ContentType::JSON)
        .dispatch()
        .await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
}
