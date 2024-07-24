mod utils;

use std::collections::HashMap;
use std::env;

use holochain_types::dna::{ActionHashB64, DnaHash, DnaHashB64};
use holochain_types::prelude::ExternIO;
use hpos_api_rust::rocket;
use hpos_api_rust::routes::apps::call_zome::ZomeCallRequest;

use holochain_types::prelude::holochain_serial;
use holochain_types::prelude::SerializedBytes;
use hpos_api_rust::common::types::{
    DnaResource, HappInput, LoginConfig, PresentedHappBundle, PublisherPricingPref,
};
use hpos_api_rust::handlers::install;
use hpos_api_rust::handlers::install::helpers::handle_install_sl_clone;
use hpos_hc_connect::app_connection::CoreAppRoleName;
use hpos_hc_connect::hha_agent::CoreAppAgent;
use hpos_hc_connect::sl_utils::{
    sl_clone_name, sl_get_current_time_bucket, SlCloneSpec, SL_BUCKET_SIZE_DAYS,
};
use hpos_hc_connect::AppConnection;
use log::{debug, info};
use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use rocket::serde::json::{serde_json, Value};
use rocket::serde::{Deserialize, Serialize};
use rocket::tokio;
use utils::core_apps::{Happ, HHA_URL};
use utils::{publish_and_enable_hosted_happ, sample_sl_props, Test};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CheckServiceLoggersResult {
    pub service_loggers_cloned: Vec<(String, String)>,
    pub service_loggers_deleted: Vec<(String, String)>,
}

#[tokio::test]
async fn install_components() {
    env_logger::init();
    let mut test = Test::init().await;

    // Install hha
    let _ = test.install_app(Happ::HHA, None).await;

    // Connect to hha
    let mut hha = CoreAppAgent::spawn(None).await.unwrap();

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

    // create two time buckets into which sampl activity is logged.

    let time_bucket: u32 = sl_get_current_time_bucket(SL_BUCKET_SIZE_DAYS);
    debug!("get_current_time_bucket {}", time_bucket);
    let previous_time_bucket = time_bucket - 1;
    debug!("previous_time_bucket {}", previous_time_bucket);

    for bucket in vec![previous_time_bucket.clone(), time_bucket.clone()] {
        let props = sample_sl_props(SL_BUCKET_SIZE_DAYS, bucket);
        debug!("cloning sl: {:#?}", &props);
        let cloned_cell = handle_install_sl_clone(&mut sl_ws, &props, bucket)
            .await
            .unwrap();
        debug!("sl_cloned_cell: {:#?}", &cloned_cell);
    }
    for bucket in vec![previous_time_bucket, time_bucket] {
        // Generate some SL activity
        for _ in 1..=5 {
            debug!("BUCKET {}", bucket);
            let payload = test.generate_sl_payload(&mut sl_ws).await;
            let sl_response: ActionHashB64 = sl_ws
                .clone_zome_call_typed(
                    "servicelogger".into(),
                    sl_clone_name(SlCloneSpec {
                        days_in_bucket: SL_BUCKET_SIZE_DAYS,
                        time_bucket: bucket,
                    }),
                    "service".into(),
                    "log_activity".into(),
                    payload,
                )
                .await
                .unwrap();
            debug!("logged activity: {}", sl_response);
        }
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
    assert!(response_body.contains("3wzfdfbwd4q0ct01sfnux3jsz4sygef5dhjm2a43eij2iqt5cj"));

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
    let path = format!("/apps/hosted/{}/logs?days=30", &test_hosted_happ_id);
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
        role_id: CoreAppRoleName::HHA.into(),
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
    let body_str = response
        .into_string()
        .await
        .expect("Failed to read response body");
    let json: serde_json::Value = serde_json::from_str(&body_str).expect("Failed to parse JSON");
    let version = json["version"].as_str().ok_or("Version not found").unwrap();
    debug!("body: {:#?}", body_str);
    debug!("version: {:#?}", version);
    assert_eq!(version.to_string(), Happ::HHA.to_string());

    // get earnings report
    let path = format!("/host/earnings");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    assert_eq!(response_body, "{\"earnings\":{\"last30days\":\"0\",\"last7days\":\"0\",\"lastday\":\"0\"},\"holofuel\":{\"redeemable\":\"0\",\"balance\":\"0\",\"available\":\"0\"},\"recentPayments\":[]}");

    // get kyc_level
    let path = format!("/host/kyc_level");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);

    // get hosting_criteria
    let path = format!("/host/hosting_criteria");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);

    // get invoices report
    let path = format!("/host/invoices");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    assert_eq!(response_body, "[]");

    // // get redemptions
    // let path = format!("/host/redemptions");
    // info!("calling {}", &path);
    // let response = client.get(path).dispatch().await;
    // debug!("status: {}", response.status());
    // assert_eq!(response.status(), Status::Ok);

    //  get usage report for 6 days
    let path = format!("/holoport/usage?usage_interval=6");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    assert_eq!(response_body, "{\"totalHostedAgents\":0,\"currentTotalStorage\":0,\"totalHostedHapps\":1,\"totalUsage\":{\"cpu\":60,\"bandwidth\":60}}");

    //  get usage report for 15 days
    let path = format!("/holoport/usage?usage_interval=15");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    assert_eq!(response_body, "{\"totalHostedAgents\":0,\"currentTotalStorage\":0,\"totalHostedHapps\":1,\"totalUsage\":{\"cpu\":120,\"bandwidth\":120}}");

    // Test installing a second hosted happ
    // Publish second hosted happ
    let mut hosted_happ_payload = HappInput::default();
    hosted_happ_payload.name = "Hosted Happ 2".to_string();
    hosted_happ_payload.bundle_url = HHA_URL.to_string(); // install with reference to actual core-app/hha bundle url
    hosted_happ_payload.special_installed_app_id = None;
    hosted_happ_payload.uid = Some("random-uid".to_string());
    let second_test_hosted_happ_id = publish_and_enable_hosted_happ(&mut hha, hosted_happ_payload)
        .await
        .unwrap();

    // Install second hosted happ on host's hp
    let path = format!("/apps/hosted/install");
    info!("calling {}", &path);
    let install_payload = install::InstallHappBody {
        happ_id: second_test_hosted_happ_id.clone(),
        membrane_proofs: HashMap::new(),
    };

    let response = client
        .post(path)
        .body(serde_json::to_string(&install_payload).unwrap())
        .header(ContentType::JSON)
        .dispatch()
        .await;

    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    assert!(response_body.contains(&format!("{}", &second_test_hosted_happ_id)));

    // Test ability to call the second hosted happ:
    // Open ws connection to servicelogger instance for hosted happ
    let mut second_hosted_happ_ws = AppConnection::connect(
        &mut test.admin_ws,
        test.hc_env.keystore.clone(),
        second_test_hosted_happ_id.to_string(),
    )
    .await
    .unwrap();
    let get_hosted_happs: Vec<PresentedHappBundle> = second_hosted_happ_ws
        .zome_call_typed("core-app".into(), "hha".into(), "get_happs".into(), ())
        .await
        .unwrap();
    // Note: This is just an assertion to make sure we get a successful call with a valid response
    // fyi: There should not yet be any hosted happs, but that is not the point of this call
    debug!("get_hosted_happs: {:#?}", get_hosted_happs);
    assert!(get_hosted_happs.is_empty());

    // Test registering with a third hosted happ
    // register a third hosted happ
    let path = format!("/apps/hosted/register");
    info!("calling {}", &path);
    let place_holder_dna: DnaHashB64 =
        DnaHash::try_from("uhC0kGNBsMPAi8Amjsa5tEVsRHZWaK-E7Fl8kLvuBvNuYtfuG1gkP")
            .unwrap()
            .into();
    let register_payload = HappInput {
        hosted_urls: vec!["test_happ_3_host_url".to_string()],
        bundle_url: HHA_URL.to_string(),
        special_installed_app_id: None,
        name: "Test Happ 3".to_string(),
        dnas: vec![DnaResource {
            hash: place_holder_dna.to_string(),
            src_url: "hosted_happ_test_3.dna".to_string(),
            nick: "happ test 3 dna".to_string(),
        }],
        exclude_jurisdictions: false,
        ui_src_url: None,
        logo_url: None,
        description: "Testing registration for dna of hosted happ 3".to_string(),
        categories: vec![],
        jurisdictions: vec![],
        publisher_pricing_pref: PublisherPricingPref::default(),
        login_config: LoginConfig::default(),
        uid: None,
    };
    let response = client
        .post(path)
        .body(serde_json::to_string(&register_payload).unwrap())
        .header(ContentType::JSON)
        .dispatch()
        .await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    #[derive(Debug, Serialize, Deserialize, SerializedBytes)]
    struct Bundle {
        id: String,
    }
    let third_test_hosted_happ = serde_json::from_str::<Bundle>(&response_body).unwrap();
    debug!("third_test_hosted_happ: {:#?}", third_test_hosted_happ);

    // enable test_hosted_happ_id
    let path = format!("/apps/hosted/{}/enable", &third_test_hosted_happ.id);
    info!("calling {}", &path);
    let response = client.post(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    debug!("body: {:#?}", response.into_string().await);

    // get third hosted happ
    let path = format!("/apps/hosted/{}", &third_test_hosted_happ.id);
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    assert!(response_body.contains(&format!("{}", &third_test_hosted_happ.id)));

    // get billing_preferences
    let path = format!("/host/billing_preferences");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    // matches the contents of './servicelogger_prefs'
    assert!(response_body.contains("\"max_fuel_before_invoice\":\"1\",\"price_compute\":\"0\",\"price_storage\":\"0\",\"price_bandwidth\":\"0\",\"max_time_before_invoice\":{\"secs\":18446744073709551615,\"nanos\":999999999},\"invoice_due_in_days\":7,\"jurisdiction_prefs\":null,\"categories_prefs\":null}"));

    // test cloning of service loggers
    let path = format!("/apps/hosted/sl-check");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    // no clones because still in same time bucket
    let r: CheckServiceLoggersResult = serde_json::from_str(&response_body).unwrap();
    assert_eq!(r.service_loggers_cloned.len(), 0);
    assert_eq!(r.service_loggers_deleted.len(), 0);

    env::set_var(
        "SL_TEST_TIME_BUCKET",
        format!("{}", sl_get_current_time_bucket(SL_BUCKET_SIZE_DAYS) + 1),
    );
    let path = format!("/apps/hosted/sl-check");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    // two clones because we are in the next time bucket
    let r: CheckServiceLoggersResult = serde_json::from_str(&response_body).unwrap();
    assert_eq!(r.service_loggers_cloned.len(), 2);
    assert_eq!(r.service_loggers_deleted.len(), 0);

    let path = format!("/apps/hosted/sl-check");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    // no more clones because we are in the next time bucket and they were just made
    let r: CheckServiceLoggersResult = serde_json::from_str(&response_body).unwrap();
    assert_eq!(r.service_loggers_cloned.len(), 0);
    assert_eq!(r.service_loggers_deleted.len(), 0);

    env::set_var("SL_TEST_IS_BEFORE_NEXT_BUCKET", format!("true"));
    let path = format!("/apps/hosted/sl-check");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    // two more clones because we are just before the next time bucket
    let r: CheckServiceLoggersResult = serde_json::from_str(&response_body).unwrap();
    assert_eq!(r.service_loggers_cloned.len(), 2);
    assert_eq!(r.service_loggers_deleted.len(), 0);

    let path = format!("/apps/hosted/sl-check");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);
    // no more clones because we already cloned them
    let r: CheckServiceLoggersResult = serde_json::from_str(&response_body).unwrap();
    assert_eq!(r.service_loggers_cloned.len(), 0);
    assert_eq!(r.service_loggers_deleted.len(), 0);

    // Move the current time-bucket forward 2 buckets to test deleting
    // And force the deleting window for the test because we can't set the clock
    env::set_var(
        "SL_TEST_TIME_BUCKET",
        format!("{}", sl_get_current_time_bucket(SL_BUCKET_SIZE_DAYS) + 2),
    );
    env::set_var("SL_TEST_IS_IN_DELETING_WINDOW", "true");
    let path = format!("/apps/hosted/sl-check");
    info!("calling {}", &path);
    let response = client.get(path).dispatch().await;
    debug!("status: {}", response.status());
    assert_eq!(response.status(), Status::Ok);
    let response_body = response.into_string().await.unwrap();
    debug!("body: {:#?}", response_body);

    // there should be two new clones for the new time buckets (13 & 14), and one deleted from
    // time bucket 10 because it never had any activity logged.
    let r: CheckServiceLoggersResult = serde_json::from_str(&response_body).unwrap();
    assert_eq!(r.service_loggers_cloned.len(), 3);
    assert_eq!(r.service_loggers_deleted.len(), 1);
    let x: Vec<&str> = r.service_loggers_deleted[0]
        .1
        .split(".")
        .into_iter()
        .collect();
    assert_eq!(x[0], "14"); // bucket size
    assert_eq!(x[1], "10"); // bucket deleted

    //TODO: find a way to run the invoicing & payment here to make the final test that clones are deleted.
}
