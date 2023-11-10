use std::{collections::HashMap, str::FromStr, time::Duration};

use crate::{
    hpos::Ws,
    types::{
        Earnings, HappDetails, HostingPlan, InvoiceNote, PresentedHappBundle, RecentUsage,
        ServiceloggerHappPreferences, Transaction, POS,
    },
};
use anyhow::{anyhow, Result};
use holochain_types::{dna::ActionHashB64, prelude::Timestamp};
use holofuel_types::fuel::Fuel;
use log::debug;

type AllTransactions = HashMap<ActionHashB64, Vec<Transaction>>;

pub async fn handle_get_all(
    usage_interval: u32,
    quantity: Option<usize>,
    ws: &mut Ws,
) -> Result<Vec<HappDetails>> {
    // get all the hosted happs from this holoport
    let core_app_id = ws.core_app_id.clone();

    debug!("calling zome hha/get_happs");
    let all_hosted_happs: Vec<PresentedHappBundle> = ws
        .call_zome(core_app_id, "core-app", "hha", "get_happs", ())
        .await?;

    // Ask holofuel for all transactions so that I can calculate earings - isn't it ridiculous?
    let mut all_transactions = get_all_transactions(ws).await?;

    let mut result: Vec<HappDetails> = vec![];
    // for each happ
    for happ in all_hosted_happs.iter() {
        // HappDetails::new(happ, ws) - async won't work because ws is &mut
        let h = HappDetails {
            id: happ.id.clone(),
            name: happ.name.clone(),
            description: happ.name.clone(),
            categories: happ.categories.clone(),
            enabled: happ.host_settings.is_enabled,
            is_paused: happ.is_paused,
            source_chains: count_instances(happ.id.clone(), ws).await?,
            days_hosted: 1, // TODO: how do I get timestamp on a link of enable happ?
            earnings: count_earnings(&mut all_transactions, happ.id.clone()).await?,
            usage: RecentUsage::default(), // from SL TODO: actually query SL for this value
            hosting_plan: get_plan(happ.id.clone(), ws).await?,
        };

        result.push(h);
    }

    // sort vec by earnings.last_7_days in decreasing order
    result.sort_by(|a, b| a.earnings.last_7_days.cmp(&b.earnings.last_7_days));

    // take first quantity only
    if let Some(q) = quantity {
        result.truncate(q);
    }

    Ok(vec![])
}

pub async fn get_plan(happ_id: ActionHashB64, ws: &mut Ws) -> Result<HostingPlan> {
    let core_app_id = ws.core_app_id.clone();

    let s: ServiceloggerHappPreferences = ws
        .call_zome(
            core_app_id,
            "core-app",
            "hha",
            "get_happ_preferences",
            happ_id,
        )
        .await?;

    if s.price_compute == Fuel::new(0)
        && s.price_storage == Fuel::new(0)
        && s.price_bandwidth == Fuel::new(0)
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

// TODO: average_weekly still needs to be calculated - from total and days_hosted?
pub async fn count_earnings(
    all_transactions: &mut AllTransactions,
    happ_id: ActionHashB64,
) -> Result<Earnings> {
    let mut e = Earnings::default();
    if let Some(payments) = all_transactions.remove(&happ_id) {
        for p in payments.iter() {
            let amount_fuel = Fuel::from_str(&p.amount)?;
            e.total = (e.total + amount_fuel)?;
            // if completed_date is within last week then add fuel to last_7_days, too
            let week = Duration::from_secs(7 * 24 * 60 * 60);
            if (Timestamp::now() - week)? < p.completed_date.unwrap() {
                e.last_7_days = (e.last_7_days + amount_fuel)?
            };
        }
        Ok(e)
    } else {
        Ok(e)
    }
}

/// get all holofuel transactions and organize in HashMap by happ_id extracted from invoice's note
pub async fn get_all_transactions(ws: &mut Ws) -> Result<AllTransactions> {
    let core_app_id = ws.core_app_id.clone();
    let mut return_map: HashMap<ActionHashB64, Vec<Transaction>> = HashMap::new();

    debug!("calling zome holofuel/transactor/get_completed_transactions");
    let mut a = ws
        .call_zome::<(), Vec<Transaction>>(
            core_app_id,
            "holofuel",
            "transactor",
            "get_completed_transactions",
            (),
        )
        .await?;

    while let Some(tx) = a.pop() {
        // only add happ to list if it is a valid hosting invoice
        if let Some(pos) = tx.proof_of_service.clone() {
            if let POS::Hosting(_) = pos {
                if let Some(note) = tx.note.clone() {
                    if let Ok((_, n)) = serde_yaml::from_str::<(String, InvoiceNote)>(&note) {
                        if let Some(mut vec) = return_map.remove(&n.hha_id) {
                            vec.push(tx);
                            return_map.insert(n.hha_id, vec);
                        } else {
                            return_map.insert(n.hha_id, vec![tx]);
                        }
                    }
                }
            }
        }
    }

    Ok(return_map)
}

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
