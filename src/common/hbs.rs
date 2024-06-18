
use anyhow::{Context, Result};
use log::debug;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::keypair::Keys;

pub async fn call_hbs<T: Serialize, U: for<'a> Deserialize<'a> + for<'de> Deserialize<'de>>(path: String, payload: T) -> Result<U> {
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
