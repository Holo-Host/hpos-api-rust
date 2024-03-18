use crate::{
    hpos::Ws,
    types::{
        HappDetails, HolofuelPaidUnpaid, InvoiceNote, PresentedHappBundle, RedemptionState,
        Transaction, TransactionDirection, POS,
    },
};
use anyhow::Result;
use chrono::{DateTime, Days, NaiveDateTime, Utc};
use holochain_types::dna::ActionHashB64;
use log::debug;
use std::collections::HashMap;

type AllTransactions = HashMap<ActionHashB64, Vec<Transaction>>;

// fetch all transactions for every hApp
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

// fetch all transactions for 1 happ
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

// get current redemable holofuel
pub async fn get_redeemable_holofuel(ws: &mut Ws) -> Result<RedemptionState> {
    let core_app_id = ws.core_app_id.clone();

    debug!("calling zome holofuel/transactor/get_redeemable");
    let result = ws
        .call_zome::<(), RedemptionState>(
            core_app_id,
            "holofuel",
            "transactor",
            "get_redeemable",
            (),
        )
        .await?;

    Ok(result)
}

// get holofuel paid/unpaid by day for the last week
pub async fn get_last_weeks_redeemable_holofuel(ws: &mut Ws) -> Result<Vec<HolofuelPaidUnpaid>> {
    let core_app_id = ws.core_app_id.clone();

    // build grouped transactions
    let mut grouped_transactions: HashMap<String, HolofuelPaidUnpaid> = HashMap::new();
    for day in 1..7 {
        let date = Utc::now()
            .checked_sub_days(Days::new(day))
            .unwrap_or_default();

        grouped_transactions.insert(
            date.clone().format("%Y-%m-%d").to_string(),
            HolofuelPaidUnpaid {
                date,
                paid: 0,
                unpaid: 0,
            },
        );
    }

    debug!("calling zome holofuel/transactor/get_completed_transactions");
    let completed_transactions = ws
        .call_zome::<(), Vec<Transaction>>(
            core_app_id.clone(),
            "holofuel",
            "transactor",
            "get_completed_transactions",
            (),
        )
        .await?;

    let one_week_ago = Utc::now()
        .checked_sub_days(Days::new(7))
        .unwrap_or_default()
        .timestamp();
    let filtered_completed_transactions =
        completed_transactions
            .iter()
            .filter(|&transaction| match transaction.completed_date {
                Some(completed_date) => completed_date.as_millis() > one_week_ago,
                None => false,
            });

    for transaction in filtered_completed_transactions {
        if transaction.direction == TransactionDirection::Outgoing {
            let date = timestamp_to_date(transaction.created_date.as_millis());
            let key = date.format("%Y-%m-%d").to_string();
            if grouped_transactions.contains_key(&key) {
                match grouped_transactions.get(&key) {
                    Some(grouped_transaction) => {
                        grouped_transactions.insert(
                            key,
                            HolofuelPaidUnpaid {
                                date: grouped_transaction.date,
                                unpaid: grouped_transaction.unpaid,
                                paid: grouped_transaction.paid
                                    + transaction.amount.parse::<u32>().unwrap(),
                            },
                        );
                    }
                    None => {
                        debug!("Could not match date {}", &key)
                    }
                }
            }

            if transaction.completed_date.is_some() {
                let date = timestamp_to_date(transaction.completed_date.unwrap().as_millis());
                let key = date.format("%Y-%m-%d").to_string();
                if grouped_transactions.contains_key(&key) {
                    let grouped_transaction = grouped_transactions.get(&key).unwrap();
                    grouped_transactions.insert(
                        key,
                        HolofuelPaidUnpaid {
                            date: grouped_transaction.date,
                            unpaid: grouped_transaction.unpaid,
                            paid: grouped_transaction.paid
                                + transaction.amount.parse::<u32>().unwrap(),
                        },
                    );
                }
            }
        }
    }

    debug!("calling zome holofuel/transactor/get_pending_transactions");
    let pending_transactions = ws
        .call_zome::<(), Vec<Transaction>>(
            core_app_id,
            "holofuel",
            "transactor",
            "get_pending_transactions",
            (),
        )
        .await?;

    let filtered_pending_transactions = pending_transactions
        .iter()
        .filter(|&transaction| transaction.created_date.as_millis() > one_week_ago);

    for transaction in filtered_pending_transactions {
        if transaction.direction == TransactionDirection::Outgoing {
            let date: DateTime<Utc> = timestamp_to_date(transaction.created_date.as_millis());
            let key = date.format("%Y-%m-%d").to_string();
            if grouped_transactions.contains_key(&key) {
                match grouped_transactions.get(&key) {
                    Some(grouped_transaction) => {
                        grouped_transactions.insert(
                            key,
                            HolofuelPaidUnpaid {
                                date: grouped_transaction.date,
                                unpaid: grouped_transaction.unpaid
                                    + transaction.amount.parse::<u32>().unwrap(),
                                paid: grouped_transaction.paid,
                            },
                        );
                    }
                    None => {
                        debug!("could not match date {}", &key);
                    }
                }
            }
        }
    }

    Ok(grouped_transactions.into_values().collect())
}

fn timestamp_to_date(timestamp: i64) -> DateTime<Utc> {
    let naive = NaiveDateTime::from_timestamp_millis(timestamp).unwrap();
    let date_time: DateTime<Utc> = DateTime::from_naive_utc_and_offset(naive, Utc);
    date_time
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
