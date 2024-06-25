use crate::rocket::serde::json::serde_json;
use anyhow::{anyhow, Context, Result};
use base64::encode_config;
use ed25519_dalek::*;
use holochain_types::prelude::ExternIO;
use hpos_config_core::public_key;
use hpos_config_core::Config;
use hpos_config_seed_bundle_explorer::unlock;
use std::env;
use std::fs::File;

pub struct Keys {
    pub email: String,
    keypair: SigningKey,
    pub pubkey_base36: String,
}

impl Keys {
    pub async fn new() -> Result<Self> {
        let (keypair, email) = from_config().await?;
        let pubkey_base36 = public_key::to_holochain_encoded_agent_key(&keypair.verifying_key());
        Ok(Self {
            email,
            keypair,
            pubkey_base36,
        })
    }

    pub async fn sign(&self, payload: ExternIO) -> Result<String> {
        let signature = self
            .keypair
            .try_sign(payload.as_bytes())
            .context("Failed to sign payload")?;

        Ok(encode_config(
            &signature.to_bytes()[..],
            base64::STANDARD_NO_PAD,
        ))
    }
}

async fn from_config() -> Result<(SigningKey, String)> {
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
            let signing_key = unlock(&device_bundle, Some(password))
                .await
                .context(format!(
                    "unable to unlock the device bundle from {}",
                    &config_path
                ))?;
            Ok((signing_key, settings.admin.email))
        }
        _ => Err(anyhow!("Unsupported version of hpos config")),
    }
}
