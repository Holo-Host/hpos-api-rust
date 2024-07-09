use anyhow::anyhow;
use core::fmt::Debug;
use holochain_client::AppInfo;
use holochain_conductor_api::CellInfo;
use holochain_types::{
    app::AppBundleSource,
    dna::AgentPubKey,
    prelude::{MembraneProof, RoleName},
};

use holofuel_types::fuel::Fuel;
use rocket::{
    data::{self, Data, FromData},
    http::Status,
    outcome::Outcome,
    request::Request,
    serde::{Deserialize, Serialize},
};
use std::collections::HashMap;
use std::time::Duration;

pub enum SuccessfulInstallResult {
    New(AppInfo),
    AlreadyInstalled,
}

pub type CellInfoMap = HashMap<RoleName, Vec<CellInfo>>;

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
pub struct RawInstallAppPayload {
    pub source: AppBundleSource,
    pub agent_key: AgentPubKey,
    pub installed_app_id: String,
    pub membrane_proofs: HashMap<RoleName, MembraneProof>,
    pub uid: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct HappPreferences {
    pub max_fuel_before_invoice: Fuel,
    pub max_time_before_invoice: Duration,
    pub price_compute: Fuel,
    pub price_storage: Fuel,
    pub price_bandwidth: Fuel,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct ServiceLoggerTimeBucket {
    pub version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct InstallHappBody {
    pub happ_id: String,
    pub membrane_proofs: HashMap<String, MembraneProof>,
}

#[rocket::async_trait]
impl<'r> FromData<'r> for InstallHappBody {
    type Error = anyhow::Error;

    async fn from_data(_request: &'r Request<'_>, data: Data<'r>) -> data::Outcome<'r, Self> {
        let byte_unit_data = data.open(data::ByteUnit::max_value());
        let decoded_data = byte_unit_data.into_bytes().await.unwrap();
        let install_payload: InstallHappBody = match rocket::serde::json::serde_json::from_slice(&decoded_data.value) {
            Ok(payload) => payload,
            Err(e) => {
                return Outcome::Error((
                    Status::UnprocessableEntity,
                    anyhow!("Provided payload to `apps/hosted/install` does not match expected payload. Error: {:?}", e),
                ))
            }
        };

        Outcome::Success(install_payload)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct CheckServiceLoggersResult {
    pub service_loggers_cloned: Vec<String>,
    pub service_loggers_deleted: Vec<String>,
}
