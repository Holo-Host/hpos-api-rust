use anyhow::{anyhow, Context, Result};

use rocket::{
    http::Status,
    serde::{json::Json, json::serde_json, Deserialize, Serialize},
    {get, State},
};

use holochain_types::prelude::AgentPubKey;
use crate::{common::hbs::call_hbs, hpos::WsMutex, hpos::Ws};
use hpos_config_core::*;
use hpos_config_seed_bundle_explorer::unlock;

use std::{collections::HashMap, env, fs::File, path::PathBuf, sync::Arc};
use std::time::{SystemTime, UNIX_EPOCH};

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
    jurisdiction: String
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
        Config::V2 { device_bundle, settings, .. } => {
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
                settings.admin.email
            ))
        }
        _ => Err(anyhow!("Unsupported version of hpos config")),
    }
}

async fn get_holo_client_auth(payload: AuthPayload) -> Result<HoloClientAuth> {
    Ok(call_hbs("/auth/api/v1/holo-client".to_owned(), payload).await?)
}

/// Returns the hosting criteria of the holoport admin user as a json object
/// {
///     "id": "string",
///     "kyc": "string",
///     "jurisdiction": "string"
/// }
#[get("/hosting_criteria")]
pub async fn hosting_criteria(wsm: &State<WsMutex>) -> Result<Json<(HostingCriteriaResponse)>, (Status, String)> {
    let mut ws = wsm.lock().await;

    let hosting_criteria_response = handle_hosting_criteria(&mut ws).await.map_err(|e| {
        (Status::InternalServerError, e.to_string())
    })?;

    Ok(Json(hosting_criteria_response))    
}

async fn handle_hosting_criteria(ws: &mut Ws) -> Result<HostingCriteriaResponse> {
    let (agent_string, _device_bundle, email) = from_config()
    .await
    .unwrap();

    let payload = AuthPayload::new(email, agent_string);

    let auth_result = get_holo_client_auth(payload).await?;

    Ok(HostingCriteriaResponse {
        id: auth_result.id,
        kyc: auth_result.kyc,
        jurisdiction: auth_result.jurisdiction
    })        
}

/// Returns the kyc level of the holoport admin user as a string
#[get("/kyc_level")]
pub async fn kyc_level(wsm: &State<WsMutex>) -> Result<String, (Status, String)> {
    let mut ws = wsm.lock().await;

    let kyc_level = handle_kyc_level(&mut ws).await.map_err(|e| {
        (Status::InternalServerError, e.to_string())
    })?;

    Ok(kyc_level)
}

async fn handle_kyc_level(ws: &mut Ws) -> Result<String> {
    let (agent_string, _device_bundle, email) = from_config()
    .await
    .unwrap();

    let payload = AuthPayload::new(email, agent_string);

    let auth_result = get_holo_client_auth(payload).await?;

    Ok(auth_result.kyc)        
}