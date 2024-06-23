use anyhow::{Context, Result};
use holochain_types::{dna::EntryHashB64, prelude::Timestamp};
use log::debug;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{keypair::Keys, types::RedemptionRecord};

#[derive(Clone, Debug)]
pub struct HBS {
    url: Option<String>,
    token: Option<String>,
    token_created: Timestamp,
}

impl Default for HBS {
    fn default() -> Self {
        HBS {
            url: None,
            token: None,
            token_created: Timestamp::from_micros(0),
        }
    }
}

impl HBS {
    /// Returns autorizarion token that is used by HBS
    /// which is obtained from HBS /auth/api/v1/holo-client endpoint
    /// Caches result for `EXPIERY` seconds
    pub async fn token(mut self) -> Result<String> {
        const EXPIERY: i64 = 24 * 60 * 60;
        if let Some(token) = &self.token {
            // Check token expiry
            if (Timestamp::now() - self.token_created)?.num_seconds() < EXPIERY {
                return Ok(token.clone());
            }
        }
        // Get new token and save with expiery
        self.token = Some(self.download_holo_client().await?.access_token);
        self.token_created = Timestamp::now();
        Ok(self.token.unwrap())
    }

    /// Returns HBS base url which is read from env var HBS_URL
    fn url(&mut self) -> Result<String> {
        match self.url.clone() {
            Some(s) => Ok(s),
            None => {
                self.url =
                    Some(std::env::var("HBS_URL").context("Cannot read HBS_URL from env var")?);
                Ok(self.url.clone().unwrap())
            }
        }
    }

    /// Handles post requerst to HBS server under /auth/api/v1/holo-client path
    /// Creates signature from agent's key that is verified by HBS
    /// Returns `HoloClientAuth` struct
    pub async fn download_holo_client(&mut self) -> Result<HoloClientAuth> {
        let email = "abba".into();
        let pub_key = "abba".into();

        let payload = AuthPayload {
            email,
            timestamp: Timestamp::now()
                .as_seconds_and_nanos()
                .0
                .try_into()
                .unwrap(),
            pub_key,
        };

        let signature = "abba";

        let client = Client::new();
        let res = client
            .post(format!("{}/auth/api/v1/holo-client", self.url()?))
            .json(&payload)
            .header("X-Signature", signature)
            .send()
            .await?;

        debug!("API response: {:?}", res);

        res.json().await.context("Failed to parse response body")
    }

    /// Handles post requerst to HBS server under /reserve/api/v2/redemptions/get path
    /// Creates authorization header from HBS.token
    /// Returns `Vec<RedemptionRecord>`
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

#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct HoloClientAuth {
    pub id: String,
    email: String,
    access_token: String,
    permissions: String,
    profile_image: String,
    display_name: String,
    pub kyc: String,
    pub jurisdiction: String,
    public_key: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct AuthPayload {
    pub email: String,
    pub timestamp: u64,
    pub pub_key: String,
}

impl AuthPayload {
    // Method to convert the struct into bytes
    pub fn into_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        bytes.extend(self.email.as_bytes());
        bytes.extend(&self.timestamp.to_be_bytes());
        bytes.extend(self.pub_key.as_bytes());

        bytes
    }
}

// let (agent_string, _device_bundle, email) = from_config().await.unwrap();
