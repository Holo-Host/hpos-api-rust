use std::collections::HashMap;

use crate::{
    hpos::Ws,
    types::{HappDetails, InvoiceNote, PresentedHappBundle, Transaction, POS},
};
use anyhow::Result;
use holochain_types::dna::ActionHashB64;
use log::debug;

type AllTransactions = HashMap<ActionHashB64, Vec<Transaction>>;

pub async fn handle_get_all(
    usage_interval: i64,
    quantity: Option<usize>,
    ws: &mut Ws,
) -> Result<Vec<HappDetails>> {
    let core_app_id = ws.core_app_id.clone();

    debug!("calling zome hha/get_happs");
    let all_hosted_happs: Vec<PresentedHappBundle> = ws
        .call_zome(core_app_id, "core-app", "hha", "get_happs", ())
        .await?;

    // Ask holofuel for all transactions so that I can calculate earings - isn't it ridiculous?
    let mut all_transactions = get_all_transactions(ws).await?;

    let mut result: Vec<HappDetails> = vec![];
    for happ in all_hosted_happs.iter() {
        result.push(
            HappDetails::init(
                happ,
                all_transactions.remove(&happ.id).unwrap_or(vec![]),
                usage_interval,
                ws,
            )
            .await,
        );
    }

    // sort vec by earnings.last_7_days in decreasing order
    result.sort_by(|a, b| {
        let a = a.earnings.clone().unwrap_or_default();
        let b = b.earnings.clone().unwrap_or_default();
        a.last_7_days.cmp(&b.last_7_days)
    });

    // take first `quantity` only
    if let Some(q) = quantity {
        result.truncate(q);
    }

    Ok(result)
}

pub async fn handle_get_one(
    id: ActionHashB64,
    usage_interval: i64,
    ws: &mut Ws,
) -> Result<HappDetails> {
    let core_app_id = ws.core_app_id.clone();

    debug!("calling zome hha/get_happs");
    let happ: PresentedHappBundle = ws
        .call_zome(core_app_id, "core-app", "hha", "get_happ", id)
        .await?;

    // Ask holofuel for all transactions so that I can calculate earings - isn't it ridiculous?
    let mut all_transactions = get_all_transactions(ws).await?;

    Ok(HappDetails::init(
        &happ,
        all_transactions.remove(&happ.id).unwrap_or(vec![]),
        usage_interval,
        ws,
    )
    .await)
}

/// get all holofuel transactions and organize in HashMap by happ_id extracted from invoice's note
pub async fn get_all_transactions(ws: &mut Ws) -> Result<AllTransactions> {
    let core_app_id = ws.core_app_id.clone();
    let mut return_map: AllTransactions = HashMap::new();

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
        if let Some(POS::Hosting(_)) = tx.proof_of_service.clone() {
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

        let _: Out = serde_yaml::from_str(&string).unwrap();
    }
}
