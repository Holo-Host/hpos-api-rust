use crate::common::types::{HappInput, PresentedHappBundle};
use anyhow::Result;
use hpos_hc_connect::app_connection::CoreAppRoleName;

pub mod types;
use crate::hpos::Ws;

pub async fn handle_register_app(
    ws: &mut Ws,
    payload: HappInput,
) -> Result<PresentedHappBundle> {
    log::debug!("calling zome hosted/register with payload: {:?}", &payload);
    let app_connection = ws.get_connection(ws.core_app_id.clone()).await?;

    let happ = app_connection
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            "hha".into(),
            "register_happ".into(),
            payload,
        )
        .await?;

    Ok(happ)
}
