use anyhow::{anyhow, Context, Result};

use crate::common::hbs::HBS;
use hpos_config_core::*;
use hpos_config_seed_bundle_explorer::unlock;
use rocket::{
    get,
    http::Status,
    serde::{json::serde_json, json::Json, Deserialize, Serialize},
};

use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs::File};

#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct HoloClientAuth {
    id: String,
    email: String,
    access_token: String,
    permissions: String,
    profile_image: String,
    display_name: String,
    kyc: String,
    jurisdiction: String,
    public_key: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct HostingCriteriaResponse {
    id: String,
    kyc: String,
    jurisdiction: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct AuthPayload {
    pub email: String,
    pub timestamp: u64,
    pub pub_key: String,
}

impl AuthPayload {
    pub fn new(email: String, pub_key: String) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        AuthPayload {
            email,
            timestamp,
            pub_key,
        }
    }

    // Method to convert the struct into bytes
    pub fn into_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        bytes.extend(self.email.as_bytes());
        bytes.extend(&self.timestamp.to_be_bytes());
        bytes.extend(self.pub_key.as_bytes());

        bytes
    }
}

async fn from_config() -> Result<(String, String, String)> {
    let config_path =
        env::var("HPOS_CONFIG_PATH").context("Cannot read HPOS_CONFIG_PATH from env var")?;

    let password = env::var("DEVICE_SEED_DEFAULT_PASSWORD")
        .context("Cannot read bundle password from env var")?;

    let config_file =
        File::open(&config_path).context(format!("Failed to open config file {}", config_path))?;

    match serde_json::from_reader(config_file)? {
        Config::V2 {
            device_bundle,
            settings,
            ..
        } => {
            // take in password
            let public = unlock(&device_bundle, Some(password))
                .await
                .context(format!(
                    "unable to unlock the device bundle from {}",
                    &config_path
                ))?
                .verifying_key();
            Ok((
                public_key::to_holochain_encoded_agent_key(&public),
                device_bundle,
                settings.admin.email,
            ))
        }
        _ => Err(anyhow!("Unsupported version of hpos config")),
    }
}



/// Returns the hosting criteria of the holoport admin user as a json object
/// {
///     "id": "string",
///     "kyc": "string",
///     "jurisdiction": "string"
/// }
#[get("/hosting_criteria")]
pub async fn hosting_criteria() -> Result<Json<HostingCriteriaResponse>, (Status, String)> {
    let hosting_criteria_response = handle_hosting_criteria()
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    Ok(Json(hosting_criteria_response))
}

async fn handle_hosting_criteria() -> Result<HostingCriteriaResponse> {
    let hbs_holo_client = HBS::download_holo_client().await?;

    Ok(HostingCriteriaResponse {
        id: hbs_holo_client.id,
        kyc: hbs_holo_client.kyc,
        jurisdiction: hbs_holo_client.jurisdiction,
    })
}

/// Returns the kyc level of the holoport admin user as a string
#[get("/kyc_level")]
pub async fn kyc_level() -> Result<String, (Status, String)> {
    let kyc_level = handle_kyc_level()
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    Ok(kyc_level)
}

async fn handle_kyc_level() -> Result<String> {
    Ok(HBS::download_holo_client().await?.kyc)
}
