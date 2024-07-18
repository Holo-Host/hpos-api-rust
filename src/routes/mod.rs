use crate::common::types::HappAndHost;
use crate::hpos::WsMutex;
use holochain_types::dna::ActionHashB64;
use rocket::{get, State};

pub mod apps;
pub mod holoport;
pub mod host;

/// Returns holoport id - used mostly as an I'm alive ping endpoint
#[get("/")]
pub async fn index(wsm: &State<WsMutex>) -> String {
    let mut ws = wsm.lock().await;

    // Construct sample HappAndHost just to retrieve holoport_id
    let sample = HappAndHost::init(
        ActionHashB64::from_b64_str("uhCkklkJVx4u17eCaaKg_phRJsHOj9u57v_4cHQR-Bd9tb-vePRyC")
            .unwrap(),
        &mut ws,
    )
    .await
    .unwrap();

    format!("🤖 I'm your holoport {}", sample.holoport_id)
}
