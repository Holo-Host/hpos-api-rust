use anyhow::anyhow;
use rocket::{
    data::{self, Data, FromData},
    http::Status,
    outcome::Outcome,
    request::Request,
};

use crate::common::types::HappInput;

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
