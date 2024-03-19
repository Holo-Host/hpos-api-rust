use std::collections::HashMap;

use crate::{
    hpos::Ws,
    types::{
        HappDetails, HolofuelPaidUnpaid, InvoiceNote, PendingTransactions, PresentedHappBundle,
        RedemptionState, Transaction, TransactionDirection, POS,
    },
};
use anyhow::Result;
use chrono::{DateTime, Days, NaiveDateTime, Utc};
use holochain_types::dna::ActionHashB64;
use log::debug;

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

    let one_week_ago = Utc::now()
        .checked_sub_days(Days::new(7))
        .unwrap_or_default()
        .timestamp();

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

    debug!("filtering get_completed_transactions");
    let filtered_completed_transactions: Vec<Transaction> = completed_transactions
        .iter()
        .filter(|&transaction| match transaction.completed_date {
            Some(completed_date) => completed_date.as_millis() > one_week_ago,
            None => false,
        })
        .cloned()
        .collect();

    debug!("calling zome holofuel/transactor/get_pending_transactions");
    let pending_transactions = ws
        .call_zome::<(), PendingTransactions>(
            core_app_id,
            "holofuel",
            "transactor",
            "get_pending_transactions",
            (),
        )
        .await?;

    debug!("filtering get_pending_transactions");
    let filtered_pending_transactions: Vec<Transaction> = pending_transactions
        .invoice_pending
        .iter()
        .filter(|&transaction| transaction.created_date.as_millis() > one_week_ago)
        .cloned()
        .collect();

    debug!("grouping transactions by day");
    let filtered_transactions: Vec<Transaction> = [
        filtered_completed_transactions,
        filtered_pending_transactions,
    ]
    .concat();
    Ok(group_transactions_by_day(filtered_transactions))
}

fn timestamp_to_date(timestamp: i64) -> DateTime<Utc> {
    let naive = NaiveDateTime::from_timestamp_millis(timestamp).unwrap();
    let date_time: DateTime<Utc> = DateTime::from_naive_utc_and_offset(naive, Utc);
    date_time
}

// groups transactions by day for last 7 days. The grouped transactions have paid and unpaid amounts
// this is used to build histogram in front-end
fn group_transactions_by_day(transactions: Vec<Transaction>) -> Vec<HolofuelPaidUnpaid> {
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

    for transaction in transactions {
        if transaction.direction == TransactionDirection::Outgoing {
            let created_date = timestamp_to_date(transaction.created_date.as_millis());
            let created_date_key = created_date.format("%Y-%m-%d").to_string();
            match grouped_transactions.get(&created_date_key) {
                Some(grouped_transaction) => {
                    grouped_transactions.insert(
                        created_date_key,
                        HolofuelPaidUnpaid {
                            date: grouped_transaction.date,
                            unpaid: grouped_transaction.unpaid,
                            paid: grouped_transaction.paid
                                + transaction.amount.parse::<u32>().unwrap(),
                        },
                    );
                }
                None => {
                    debug!("Could not match date {}", &created_date_key)
                }
            }

            match transaction.completed_date {
                Some(completed_date) => {
                    let date = timestamp_to_date(completed_date.as_millis());
                    let key = date.format("%Y-%m-%d").to_string();
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
                            debug!("Could not match date {}", &completed_date)
                        }
                    }
                }
                None => {
                    debug!(
                        "could not find completed date for transaction {}",
                        transaction.id
                    )
                }
            }
        }
    }

    grouped_transactions.into_values().collect()
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
    use holochain_types::{
        dna::{encode::holo_dht_location_bytes, AgentPubKeyB64, EntryHashB64},
        prelude::Timestamp,
    };
    use log::debug;
    use rand::{distributions::Alphanumeric, thread_rng, Rng};
    use serde::{Deserialize, Serialize};
    use serde_yaml;

    use crate::types::{Transaction, TransactionDirection, TransactionStatus, TransactionType};

    use super::group_transactions_by_day;

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

    fn generate_transaction_id() -> String {
        let prefix = "u";
        let id = "00000000000000000000000000000000000";
        let config = base64::URL_SAFE_NO_PAD;
        let suffix = holo_dht_location_bytes(&id.as_bytes()[3..35]);
        let suffix_str = String::from_utf8(suffix).unwrap();

        let str = format!("{}{}", id, &suffix_str);
        let encoded_str = base64::encode_config(str, config);
        format!("{}{}", prefix, encoded_str)
    }

    fn generate_mock_transaction(
        transaction_created_days_ago: u64,
        transaction_completed_days_ago: Option<u64>,
    ) -> Transaction {
        let created_date = chrono::Utc::now()
            .checked_sub_days(chrono::Days::new(transaction_created_days_ago))
            .unwrap_or_default();

        let transaction_id = generate_transaction_id();
        Transaction {
            id: EntryHashB64::from_b64_str(&transaction_id).unwrap(),
            amount: "100".to_string(), // Example amount
            fee: "10".to_string(),     // Example fee
            created_date: Timestamp::from_micros(created_date.timestamp_micros()),
            completed_date: match transaction_completed_days_ago {
                Some(days_ago) => Some(Timestamp::from_micros(
                    chrono::Utc::now()
                        .checked_sub_days(chrono::Days::new(days_ago))
                        .unwrap_or_default()
                        .timestamp_micros(),
                )),
                None => None,
            },
            transaction_type: TransactionType::Request,
            counterparty: AgentPubKeyB64::from_b64_str(
                "dWhDQWtyZ2VFTDdhY0l5aF8xQ2tlQzktQnV3eFVCS0kzMThBcTl2VXo0SEphWjRpY0tuVHU=",
            )
            .unwrap(), // Replace with actual agent pub key
            direction: TransactionDirection::Outgoing,
            status: TransactionStatus::Completed,
            note: None,
            url: None,
            expiration_date: None,
            proof_of_service: None,
        }
    }

    fn generate_mock_transactions() -> Vec<Transaction> {
        [
            generate_mock_transaction(2, None),
            generate_mock_transaction(5, Some(2)),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn test_group_transactions_by_day() {
        let transactions = generate_mock_transactions();
        let result = group_transactions_by_day(transactions);
        assert_eq!(result.len(), 2);
    }
}
