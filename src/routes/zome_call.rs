use crate::hpos::WsMutex;
use rocket::{
    http::Status,
    serde::{
        json::{serde_json, Json},
        Deserialize, Serialize,
    },
    Responder, {post, State},
};

#[post("/zome_call", format = "json", data = "<data>")]
pub async fn zome_call(
    data: Json<ZomeCallRequest>,
    wsm: &State<WsMutex>,
) -> Result<ZomeCallResponse, (Status, String)> {
    let mut ws = wsm.lock().await;

    // arguments of ws.zome_call require 'static lifetime and data is only temporary
    // so I need to extend lifetime with Box::leak
    let data = Box::leak(Box::new(data.into_inner()));

    let app_connection = ws
        .get_connection(data.app_id.clone())
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    let res: Vec<u8> = app_connection
        .zome_call_typed(
            data.role_id.clone(),
            data.zome_name.clone().into(),
            data.fn_name.clone().into(),
            data.payload.clone(),
        )
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;

    // same here as above - extending lifetime to 'static with Box::leak
    let res = Box::leak(Box::new(res));

    Ok(ZomeCallResponse(res))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct ZomeCallRequest {
    pub app_id: String,
    pub role_id: String,
    pub zome_name: String,
    pub fn_name: String,
    pub payload: serde_json::Value,
}

#[derive(Responder)]
#[response(status = 200, content_type = "binary")]
pub struct ZomeCallResponse(pub &'static [u8]);

#[cfg(test)]
mod test {
    use holochain_types::dna::ActionHashB64;

    #[test]
    fn decode_hash() {
        let str = "uhCkklkJVx4u17eCaaKg_phRJsHOj9u57v_4cHQR-Bd9tb-vePRyC";
        ActionHashB64::from_b64_str(str).unwrap();
    }
}
