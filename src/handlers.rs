use crate::{hpos::Ws, types::HappDetails};
use anyhow::Result;

pub async fn handle_get_all(
    usage_interval: u32,
    quantity: Option<u32>,
    ws: &mut Ws,
) -> Result<Vec<HappDetails>> {
    // Ask for all this data from hha, holofuel and service logger and compose an answer

    Ok(vec![])
}
