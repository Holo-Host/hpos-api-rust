use crate::{hpos::Ws, types::HappDetails};
use anyhow::Result;

pub async fn handle_get_all(
    usage_interval: u32,
    quantity: Option<u32>,
    ws: &mut Ws,
) -> Result<Vec<HappDetails>> {
    // get all the hosted happs from this holoport
    // core-app hha/get_happs

    // for each happ
    // HappDetails::new(happ, ws) - async won't work because ws is &mut
    let h = HappDetails {
        id: todo!(), // from hha
        name: todo!(), // from hha
        description: todo!(), // from hha
        categories: todo!(), // from hha
        enabled: todo!(), // from hha
        is_paused: todo!(), // from hha
        source_chains: todo!(), // counting instances of a given happ by it's name (id)
        days_hosted: todo!(), // timestamp on a link of enable happ
        earnings: todo!(), // From holofuel
        usage: todo!(), // from SL
        hosting_plan: todo!(), // in hha - settings set to 0 (get happ preferences, all 3 == 0)
    };

    // order vec by ???

    // take first quantity only

    // return vec
    Ok(vec![])
}
