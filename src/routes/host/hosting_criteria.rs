
use super::keypair::Keys;

use rocket::{
    http::Status,
    serde::json::Json,
    {get, State},
};

use crate::hpos::WsMutex;

use std::env;
use std::fs;
use std::io::Read;
use std::path::Path;
use serde_json::Value;
use std::io::Error;

pub fn get_hpos_config() -> Result<Value, Error> {
    let hpos_config_path = env::var("HPOS_CONFIG_PATH").unwrap();

    if !Path::new(&hpos_config_path).exists() {
        return Err(Error::new(std::io::ErrorKind::NotFound, format!("HPOS config not found at {}", hpos_config_path)));
    }

    let mut file = fs::File::open(&hpos_config_path)?;
    let mut config = String::new();
    file.read_to_string(&mut config)?;

    let config_global: Value = serde_json::from_str(&config)?;

    Ok(config_global)
}

/// ???
#[get("/hosting_criteria")]
pub async fn hosting_criteria(wsm: &State<WsMutex>) -> Result<Json<()>, (Status, String)> {
    let mut ws = wsm.lock().await;

    Ok(Json(()))
}

/// ???
#[get("/kyc_level")]
pub async fn kyc_level(wsm: &State<WsMutex>) -> Result<Json<()>, (Status, String)> {
    let mut ws = wsm.lock().await;

    let response = reqwest::get("https://api.example.com/hbs/kyc_level")
        .await
        .map_err(|err| (Status::InternalServerError, err.to_string()))?;
    
    if response.status().is_success() {
        let body = response.text().await.map_err(|err| (Status::InternalServerError, err.to_string()))?;
        // Process the response body here
        Ok(Json(()))
    } else {
        let response_status = response.status();
        let status_code = response_status.as_u16();
        let status = Status::from_code(status_code).unwrap_or(Status::InternalServerError);
        let error_message = response.text().await.map_err(|err| (Status::InternalServerError, err.to_string()))?;
        Err((status, error_message))
    }
}