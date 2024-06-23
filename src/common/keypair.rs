use crate::rocket::serde::json::serde_json;
use anyhow::{Context, Result};
use base64::encode_config;
use ed25519_dalek::*;
use hpos_config_core::{public_key::to_base36_id, Config};
use hpos_config_seed_bundle_explorer::holoport_key;
use serde::Serialize;
use std::env;
use std::fs::File;

pub struct Keys {
    keypair: SigningKey,
    pub pubkey_base36: String,
}

impl Keys {
    pub async fn new() -> Result<Self> {
        let keypair = keypair_from_config().await?;
        let pubkey_base36 = to_base36_id(&keypair.verifying_key());
        Ok(Self {
            keypair,
            pubkey_base36,
        })
    }

    pub async fn sign<T: Serialize>(&self, payload: T) -> Result<String> {
        let signature = self
            .keypair
            .try_sign(&into_bytes(payload)?)
            .context("Failed to sign payload")?;
        Ok(encode_config(
            &signature.to_bytes()[..],
            base64::STANDARD_NO_PAD,
        ))
    }
}

fn into_bytes<T: Serialize>(payload: T) -> Result<Vec<u8>> {
    serde_json::to_vec(&payload).context("Failed to convert payload to bytes")
}

async fn keypair_from_config() -> Result<SigningKey> {
    let config_path =
        env::var("HPOS_CONFIG_PATH").context("Cannot read HPOS_CONFIG_PATH from env var")?;

    let password = env::var("DEVICE_SEED_DEFAULT_PASSWORD")
        .context("Cannot read bundle password from env var")?;

    let config_file =
        File::open(&config_path).context(format!("Failed to open config file {}", config_path))?;

    let config: Config = serde_json::from_reader(config_file)
        .context(format!("Failed to read config from file {}", &config_path))?;

    holoport_key(&config, Some(password)).await.context(format!(
        "Failed to obtain holoport signing key from file {}",
        config_path
    ))
}


// async fn from_config() -> Result<(String, String, String)> {
//     let config_path =
//         env::var("HPOS_CONFIG_PATH").context("Cannot read HPOS_CONFIG_PATH from env var")?;

//     let password = env::var("DEVICE_SEED_DEFAULT_PASSWORD")
//         .context("Cannot read bundle password from env var")?;

//     let config_file =
//         File::open(&config_path).context(format!("Failed to open config file {}", config_path))?;

//     match serde_json::from_reader(config_file)? {
//         Config::V2 {
//             device_bundle,
//             settings,
//             ..
//         } => {
//             // take in password
//             let public = unlock(&device_bundle, Some(password))
//                 .await
//                 .context(format!(
//                     "unable to unlock the device bundle from {}",
//                     &config_path
//                 ))?
//                 .verifying_key();
//             Ok((
//                 public_key::to_holochain_encoded_agent_key(&public),
//                 device_bundle,
//                 settings.admin.email,
//             ))
//         }
//         _ => Err(anyhow!("Unsupported version of hpos config")),
//     }
// }



// #[derive(Serialize, Deserialize, Clone)]
// #[serde(crate = "rocket::serde")]
// pub struct AuthPayload {
//     pub email: String,
//     pub timestamp: u64,
//     pub pub_key: String,
// }

// impl AuthPayload {
//     pub fn new(email: String, pub_key: String) -> Self {
//         let timestamp = SystemTime::now()
//             .duration_since(UNIX_EPOCH)
//             .expect("Time went backwards")
//             .as_secs();

//         AuthPayload {
//             email,
//             timestamp,
//             pub_key,
//         }
//     }

//     // Method to convert the struct into bytes
//     pub fn into_bytes(&self) -> Vec<u8> {
//         let mut bytes = Vec::new();

//         bytes.extend(self.email.as_bytes());
//         bytes.extend(&self.timestamp.to_be_bytes());
//         bytes.extend(self.pub_key.as_bytes());

//         bytes
//     }
// }
