use std::collections::HashMap;

use crate::{
    hpos::Ws,
    types::{
        Earnings, HappDetails, HostingPlan, PresentedHappBundle, RecentUsage,
        ServiceloggerHappPreferences,
    },
};
use anyhow::{anyhow, Result};
use holochain_client::AppInfo;
use holochain_types::dna::ActionHashB64;
use holofuel_types::fuel::Fuel;
use hpos_hc_connect::holofuel_types::Transaction;
use log::debug;

pub async fn handle_get_all(
    usage_interval: u32,
    quantity: Option<u32>,
    ws: &mut Ws,
) -> Result<Vec<HappDetails>> {
    // get all the hosted happs from this holoport
    let core_app_id = ws.core_app_id.clone();

    debug!("calling zome hha/get_happs");
    let all_hosted_happs: Vec<PresentedHappBundle> = ws
        .call_zome(core_app_id, "core-app", "hha", "get_happs", ())
        .await?;

    // Ask holofuel for all transactions so that I can calculate earings - isn't it ridiculous?

    let mut result: Vec<HappDetails> = vec![];
    // for each happ
    for happ in all_hosted_happs.iter() {
        // HappDetails::new(happ, ws) - async won't work because ws is &mut
        let h = HappDetails {
            id: happ.id.clone(),                                        // from hha
            name: happ.name.clone(),                                    // from hha
            description: happ.name.clone(),                             // from hha
            categories: happ.categories.clone(),                        // from hha
            enabled: happ.host_settings.is_enabled,                     // from hha
            is_paused: happ.is_paused,                                  // from hha
            source_chains: count_instances(happ.id.clone(), ws).await?, // counting instances of a given happ by it's name (id)
            days_hosted: 0, // TODO: how do I get timestamp on a link of enable happ?
            earnings: todo!(), // From holofuel
            usage: RecentUsage::default(), // from SL TODO: actually query SL for this value
            hosting_plan: get_plan(happ.id.clone(), ws).await?, // in hha - settings set to 0 (get happ preferences, all 3 == 0) - call get_happ_preferences
        };

        result.push(h);
    }

    // order vec by ???

    // take first quantity only

    // return vec
    Ok(vec![])
}

pub async fn get_plan(happ_id: ActionHashB64, ws: &mut Ws) -> Result<HostingPlan> {
    let core_app_id = ws.core_app_id.clone();

    let s: ServiceloggerHappPreferences = ws
        .call_zome(core_app_id, "core-app", "hha", "get_happ_preferences", ())
        .await?;

    if (s.price_compute == Fuel::new(0)
        && s.price_storage == Fuel::new(0)
        && s.price_bandwidth == Fuel::new(0))
    {
        Ok(HostingPlan::Free)
    } else {
        Ok(HostingPlan::Paid)
    }
}

pub async fn count_instances(happ_id: ActionHashB64, ws: &mut Ws) -> Result<u16> {
    // What filter shall I use in list_happs()? Is None correct?
    Ok(ws
        .admin
        .list_apps(None)
        .await
        .map_err(|err| anyhow!("{:?}", err))?
        .iter()
        .fold(0, |acc, info| {
            if info.installed_app_id.contains(&format!("{}:uhCA", happ_id)) {
                acc + 1
            } else {
                acc
            }
        }))
}

pub async fn count_earnings(ws: &mut Ws) -> () { //Result<Earnings> {
}

// pub async fn get_all_transactions(ws: &mut Ws) -> Result<(HashMap<ActionHashB64, Vec<Transaction>>)> {
//     let core_app_id = ws.core_app_id.clone();

//     debug!("calling zome hha/get_happs");
//     let mut return_map: HashMap<ActionHashB64, Vec<Transaction>> = HashMap::new();

//     Ok(ws
//         .call_zome::<(), Vec<Transaction>>(core_app_id, "holofuel", "transactor", "get_completed_transactions", ())
//         .await?
//         .iter()
//         .fold(return_map, |acc, tx| {
//             if let Some(note) = tx.note.clone() {
//                 let
//                 acc
//             } else {
//                 acc
//             }
//         }))
// }

#[cfg(test)]
mod test {
    use serde::{Deserialize, Serialize};
    use serde_yaml;

    #[test]
    // proves that parsed string can represent a struct much bigger than destination type
    fn partialy_decode_yaml() {
        #[derive(Debug, Serialize, Deserialize)]
        struct In {
            a: String,
            b: String,
            c: String,
        }

        #[derive(Debug, Serialize, Deserialize)]
        struct Out {
            a: String,
            b: String,
        }

        let string = serde_yaml::to_string(&In {
            a: "abba".into(),
            b: "bbba".into(),
            c: "cbba".into(),
        })
        .unwrap();

        println!("{}", &string);

        let out: Out = serde_yaml::from_str(&string).unwrap();

        println!("{:?}", out);
    }
}
