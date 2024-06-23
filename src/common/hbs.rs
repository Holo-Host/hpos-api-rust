use anyhow::{Context, Result};
use holochain_types::dna::EntryHashB64;
use log::debug;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::HoloClientAuth;

use super::{keypair::Keys, types::RedemptionRecord};

pub struct HBS {
    url: Option<String>,
    token: Option<String>
}

impl HBS {
    pub async fn token(self) -> Result<String> {
        if let Some(token) =  self.token {
            // Check token expiry

            return Ok(token);
        }
        Ok("abba".into())
    }

    fn url(mut self) -> Result<String> {
        match self.url {
            Some(s) => Ok(s),
            None => {
                self.url = Some(std::env::var("HBS_URL").context("Cannot read HBS_URL from env var")?);
                Ok(self.url.unwrap())
            }
        }
    }

    pub async fn download_holo_client() -> Result<HoloClientAuth> {
        Ok("abba".into())
    }

    pub async fn get_redemption_records(ids: Vec<EntryHashB64>) -> Result<Vec<RedemptionRecord>> {
        call_hbs("/reserve/api/v2/redemptions/get".to_owned(), ids).await
    }
}

pub async fn call_hbs<T: Serialize, U: for<'a> Deserialize<'a> + for<'de> Deserialize<'de>>(
    path: String,
    payload: T,
) -> Result<U> {
    let hbs_base = std::env::var("HBS_URL").context("Cannot read HBS_URL from env var")?;

    let full_path = format!("{}{}", hbs_base, path);

    let hpos_key = Keys::new().await?;

    let signature = hpos_key.sign(&payload).await?;
    debug!("Signature: '{:?}'", &signature);

    let client = Client::new();
    let res = client
        .post(url::Url::parse(&full_path)?)
        .json(&payload)
        .header("X-Signature", signature)
        .send()
        .await?;

    debug!("API response: {:?}", res);

    let parsed_response: U = res.json().await.context("Failed to parse response body")?;

    Ok(parsed_response)
}


// let (agent_string, _device_bundle, email) = from_config().await.unwrap();

// let payload = AuthPayload::new(email, agent_string);

// call_hbs("/auth/api/v1/holo-client".to_owned(), payload).await