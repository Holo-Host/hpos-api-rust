use anyhow::Result;
use rocket::http::Status;

pub mod types;
use crate::hpos::Ws;
use types::*;

pub async fn handle_register_app(ws: &mut Ws, payload: types::HappInput) -> Result<Status> {
    log::debug!("calling zome hosted/register with payload: {:?}", &payload);
    let app_connection = ws.get_connection(ws.core_app_id.clone()).await?;

    app_connection
        .zome_call_typed(
            "core-app".into(),
            "hha".into(),
            "register_happ".into(),
            payload,
        )
        .await?;

    Ok(Status::Ok)
}
