use crate::common::keypair::Keys;
use rocket::get;

pub mod apps;
pub mod holoport;
pub mod host;

/// Returns holoport id - used mostly as an I'm alive ping endpoint
#[get("/")]
pub async fn index() -> String {
    // Construct sample HappAndHost just to retrieve holoport_id
    let keys = Keys::new().await.unwrap();

    format!("🤖 I'm your holoport {}", keys.holoport_id)
}
