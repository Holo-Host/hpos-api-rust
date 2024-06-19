use anyhow::anyhow;
use core::fmt::Debug;
use holochain_types::prelude::{holochain_serial, SerializedBytes};
use holofuel_types::fuel::Fuel;
use rocket::{
    data::{self, Data, FromData},
    http::Status,
    outcome::Outcome,
    request::Request,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Serialize, Deserialize, SerializedBytes, Clone, Default)]
#[serde(crate = "rocket::serde")]
pub struct LoginConfig {
    pub display_publisher_name: bool,
    pub registration_info_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes, Clone)]
#[serde(crate = "rocket::serde")]
pub struct DnaResource {
    pub hash: String, // hash of the dna, not a stored dht address
    pub src_url: String,
    pub nick: String,
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes, Clone)]
#[serde(crate = "rocket::serde")]
pub struct PublisherPricingPref {
    pub cpu: Fuel,
    pub storage: Fuel,
    pub bandwidth: Fuel,
}
impl Default for PublisherPricingPref {
    fn default() -> Self {
        PublisherPricingPref {
            cpu: Fuel::new(0),
            storage: Fuel::new(0),
            bandwidth: Fuel::new(0),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes, Clone)]
#[serde(crate = "rocket::serde")]
pub struct HappInput {
    pub hosted_urls: Vec<String>,
    pub bundle_url: String,
    #[serde(default)]
    pub ui_src_url: Option<String>,
    #[serde(default)]
    pub special_installed_app_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub logo_url: Option<String>,
    pub dnas: Vec<DnaResource>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub jurisdictions: Vec<String>,
    pub exclude_jurisdictions: bool,
    #[serde(default)]
    pub publisher_pricing_pref: PublisherPricingPref,
    #[serde(default)]
    pub login_config: LoginConfig,
    #[serde(default)]
    pub uid: Option<String>,
}

#[rocket::async_trait]
impl<'r> FromData<'r> for HappInput {
    type Error = anyhow::Error;

    async fn from_data(_request: &'r Request<'_>, data: Data<'r>) -> data::Outcome<'r, Self> {
        let byte_unit_data = data.open(data::ByteUnit::max_value());
        let decoded_data = byte_unit_data.into_bytes().await.unwrap();
        let register_payload: HappInput = match rocket::serde::json::serde_json::from_slice(&decoded_data.value) {
            Ok(payload) => payload,
            Err(e) => {
                return Outcome::Error((
                    Status::UnprocessableEntity,
                    anyhow!("Provided payload to `apps/hosted/register` does not match expected payload. Error: {:?}", e),
                ))
            }
        };

        Outcome::Success(register_payload)
    }
}
