use anyhow::{Context, Result};
use holochain_types::{
    dna::EntryHashB64,
    prelude::{ExternIO, Timestamp},
};

use log::{debug, trace};
use reqwest::Client;
use rocket::tokio::sync::Mutex;
use serde::{Deserialize, Serialize};

use super::{keypair::Keys, types::RedemptionRecord};

/// Mutex that guards state of HB
pub type HbSMutex = Mutex<HBS>;

#[derive(Clone, Debug)]
pub struct HBS {
    url: Option<String>,
    token: Option<String>,
    token_created: Timestamp,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Deserialize, Serialize)]
pub struct HBSRedemptionGetRequest {
    ids: Vec<EntryHashB64>,
}

impl HBS {
    pub fn new() -> HbSMutex {
        Mutex::new(HBS {
            url: None,
            token: None,
            token_created: Timestamp::from_micros(0),
        })
    }

    /// Returns autorizarion token that is used by HBS
    /// which is obtained from HBS /auth/api/v1/holo-client endpoint
    /// Caches result for `EXPIERY` seconds
    pub async fn token(&mut self) -> Result<String> {
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
        Ok(self.token.clone().unwrap())
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
        // create keypair that contains email from config, pubkey to_holochain_encoded_agent_key and signing_key
        let keys = Keys::new().await?;

        // extract email
        let email = keys.email.clone();

        // extrackt pub_key
        let pub_key = keys.pubkey_base36.clone();

        // format timestamp to the one with milisecs
        let now = Timestamp::now().as_seconds_and_nanos();
        let timestamp: u64 = <i64 as TryInto<u64>>::try_into(now.0 * 1000).unwrap()
            + <u32 as Into<u64>>::into(now.1 / 1_000_000);

        let payload = AuthPayload {
            email,
            timestamp,
            pub_key,
        };
        trace!("payload: {:?}", payload);

        // msgpack payload
        let encoded_payload = ExternIO::encode(&payload)?;

        // sign encoded_bytes
        let signature = keys.sign(encoded_payload).await?;
        trace!("signature: {:?}", signature);

        let client = Client::new();
        let res = client
            .post(format!("{}/auth/api/v1/holo-client", self.url()?))
            .json(&payload)
            .header("X-Signature", signature)
            .send()
            .await?;

        trace!("API response: {:?}", res);

        res.json().await.context("Failed to parse response body")
    }

    /// Handles post requerst to HBS server under /reserve/api/v2/redemptions/get path
    /// Creates authorization header from HBS.token
    /// Returns `Vec<RedemptionRecord>`
    pub async fn get_redemption_records(
        &mut self,
        ids: Vec<EntryHashB64>,
    ) -> Result<Vec<RedemptionRecord>> {
        let client = Client::new();
        let token = self.token().await?;
        let body = HBSRedemptionGetRequest { ids };
        let res = client
            .post(format!("{}/reserve/api/v2/redemptions/get", self.url()?))
            .json(&body)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        debug!("API response: {:?}", res);
        if res.status() != 200 {
            log::error!("got an invalid response from hbs: {}", res.status());
        }

        res.json().await.context("Failed to parse response body")
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct HoloClientAuth {
    pub id: String,
    email: String,
    access_token: String,
    permissions: Vec<String>,
    pub kyc: String,
    pub jurisdiction: String,
    public_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct AuthPayload {
    pub email: String,
    pub timestamp: u64,
    pub pub_key: String,
}

#[cfg(test)]
mod test {
    use std::env;

    use holochain_types::prelude::ExternIO;
    use rocket::tokio;

    use crate::common::{hbs::AuthPayload, keypair::Keys};

    #[tokio::test]
    // test if signature crteated from config file is valid which means that it
    // matches one created previously in hpos-holochain-api js version that passes HBS verification
    async fn create_valid_signatures() {
        // set up environment
        env::set_var("DEVICE_SEED_DEFAULT_PASSWORD", "pass");
        let manifets_path = env::var("CARGO_MANIFEST_DIR").unwrap();
        let hpos_config_path = format!("{}/resources/test/hpos-config.json", &manifets_path);
        env::set_var("HPOS_CONFIG_PATH", &hpos_config_path);

        let keys = Keys::new().await.unwrap();

        // extract email
        let email = keys.email.clone();
        assert_eq!(email, "alastair.ong@holo.host");

        // extract pub_key
        let pub_key = keys.pubkey_base36.clone();
        assert_eq!(
            pub_key,
            "uhCAknSCMGPEKHN6znj7RUOXjcvE-0qZkN5fusCRQb1Ir4VOL8Muw"
        );

        // use know timestamp for deteministic signature
        let timestamp: u64 = 1719348253188;

        let payload = AuthPayload {
            email,
            timestamp,
            pub_key,
        };

        // msgpack payload
        let encoded_payload = ExternIO::encode(&payload).unwrap();

        let expected_encoded_payload = vec![
            131, 165, 101, 109, 97, 105, 108, 182, 97, 108, 97, 115, 116, 97, 105, 114, 46, 111,
            110, 103, 64, 104, 111, 108, 111, 46, 104, 111, 115, 116, 169, 116, 105, 109, 101, 115,
            116, 97, 109, 112, 207, 0, 0, 1, 144, 81, 36, 82, 4, 166, 112, 117, 98, 75, 101, 121,
            217, 53, 117, 104, 67, 65, 107, 110, 83, 67, 77, 71, 80, 69, 75, 72, 78, 54, 122, 110,
            106, 55, 82, 85, 79, 88, 106, 99, 118, 69, 45, 48, 113, 90, 107, 78, 53, 102, 117, 115,
            67, 82, 81, 98, 49, 73, 114, 52, 86, 79, 76, 56, 77, 117, 119,
        ];

        assert_eq!(encoded_payload.clone().into_vec(), expected_encoded_payload);

        // sign encoded_bytes
        let signature = keys.sign(ExternIO::from(encoded_payload)).await.unwrap();
        let expected_signature = "JOy1vrrP+9P3DQ8hW5K9KKieN3V4dUKS95t8Nsb55ivD19kq8V0a1J0DqQ7m/8suhUmW7WY2NgqP3l38+lVaBA";

        assert_eq!(signature, expected_signature);
    }
}
