use anyhow::{Context, Result};
use holochain_types::dna::EntryHashB64;
use log::debug;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{keypair::Keys, types::RedemptionRecord};

#[derive(Clone, Debug, Default)]
pub struct HBS {
    url: Option<String>,
    token: Option<String>
}

impl HBS {
    /// Returns autorizarion token that is used by HBS
    /// which is obtained from HBS /auth/api/v1/holo-client endpoint
    /// Caches result for `EXPIERY` period of time
    pub async fn token(self) -> Result<String> {
        if let Some(token) =  self.token {
            // Check token expiry

            return Ok(token);
        }
        Ok("abba".into())
    }

    /// Returns HBS base url which is read from env var HBS_URL
    fn url(mut self) -> Result<String> {
        match self.url {
            Some(s) => Ok(s),
            None => {
                self.url = Some(std::env::var("HBS_URL").context("Cannot read HBS_URL from env var")?);
                Ok(self.url.unwrap())
            }
        }
    }

    /// Handles post requerst to HBS server under /auth/api/v1/holo-client path
    /// Creates signature from agent's key that is verified by HBS
    /// Returns `HoloClientAuth` struct
    pub async fn download_holo_client(&self) -> Result<HoloClientAuth> {
        Ok("abba".into())
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
struct HoloClientAuth {
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


// let (agent_string, _device_bundle, email) = from_config().await.unwrap();

// let payload = AuthPayload::new(email, agent_string);

// call_hbs("/auth/api/v1/holo-client".to_owned(), payload).await