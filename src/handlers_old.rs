use std::{collections::HashMap, str::FromStr};

use crate::{
    hpos::Ws,
    types::{
        HolofuelPaidUnpaid, PendingTransactions,
        RedemptionState, Transaction, TransactionDirection,
    },
};
use anyhow::Result;
use chrono::{DateTime, Days, NaiveDateTime, Utc};
use holofuel_types::fuel::Fuel;
use log::debug;



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

    for transaction in transactions {
        if transaction.direction == TransactionDirection::Outgoing {
            let created_date = timestamp_to_date(transaction.created_date.as_millis());
            let created_date_key = created_date.format("%Y-%m-%d").to_string();
            match grouped_transactions.get(&created_date_key.clone()) {
                Some(grouped_transaction) => {
                    let amount = Fuel::from_str(&transaction.amount).unwrap_or(Fuel::new(0));
                    grouped_transactions.insert(
                        created_date_key.clone(),
                        HolofuelPaidUnpaid {
                            date: created_date_key,
                            unpaid: (grouped_transaction.unpaid + amount)
                                .unwrap_or(grouped_transaction.paid),
                            paid: grouped_transaction.paid,
                        },
                    );
                }
                None => {
                    let amount = Fuel::from_str(&transaction.amount).unwrap_or(Fuel::new(0));
                    grouped_transactions.insert(
                        created_date_key.clone(),
                        HolofuelPaidUnpaid {
                            date: created_date_key,
                            unpaid: amount,
                            paid: Fuel::new(0),
                        },
                    );
                }
            }

            match transaction.completed_date {
                Some(completed_date) => {
                    let completed_date_date = timestamp_to_date(completed_date.as_millis());
                    let completed_date_key = completed_date_date.format("%Y-%m-%d").to_string();
                    match grouped_transactions.get(&completed_date_key.clone()) {
                        Some(grouped_transaction) => {
                            let amount =
                                Fuel::from_str(&transaction.amount).unwrap_or(Fuel::new(0));
                            grouped_transactions.insert(
                                completed_date_key.clone(),
                                HolofuelPaidUnpaid {
                                    date: completed_date_key,
                                    unpaid: grouped_transaction.unpaid,
                                    paid: (grouped_transaction.paid + amount)
                                        .unwrap_or(grouped_transaction.paid),
                                },
                            );
                        }
                        None => {
                            let amount =
                                Fuel::from_str(&transaction.amount).unwrap_or(Fuel::new(0));
                            grouped_transactions.insert(
                                completed_date_key.clone(),
                                HolofuelPaidUnpaid {
                                    date: completed_date_key,
                                    unpaid: Fuel::new(0),
                                    paid: amount,
                                },
                            );
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

    let mut sorted_keys: Vec<&String> = grouped_transactions.keys().collect();
    sorted_keys.sort_by(|a, b| b.cmp(a));

    sorted_keys
        .iter_mut()
        .map(|key| grouped_transactions.get(*key).unwrap())
        .cloned()
        .collect()
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use holochain_types::{
        dna::{AgentPubKeyB64, EntryHashB64},
        prelude::Timestamp,
    };
    use holofuel_types::fuel::Fuel;
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

    fn generate_mock_transaction(
        transaction_created_days_ago: u64,
        transaction_completed_days_ago: Option<u64>,
    ) -> Transaction {
        let created_date = chrono::Utc::now()
            .checked_sub_days(chrono::Days::new(transaction_created_days_ago))
            .unwrap_or_default();

        Transaction {
            id: EntryHashB64::from_b64_str("uhCEkKqo0z5b7ltuekF9p0iJPcfL2ghQXjhj8XnOPBYRbXMycLJfn")
                .unwrap(),
            amount: Fuel::from_str("100").unwrap_or(Fuel::new(0)).to_string(), // Example amount
            fee: "10".to_string(),                                             // Example fee
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
                "uhCAkrgeEL7acIyh_1CkeC9-BuwxUBKI318Aq9vUz4HJaZ4icKnTu",
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
            generate_mock_transaction(3, None),
            generate_mock_transaction(5, Some(2)),
            generate_mock_transaction(5, Some(3)),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn test_group_transactions_by_day() {
        let transactions = generate_mock_transactions();
        let result = group_transactions_by_day(transactions.clone());
        assert_eq!(result.len(), 3);

        let two_days_ago = chrono::Utc::now()
            .checked_sub_days(chrono::Days::new(2))
            .unwrap_or_default()
            .format("%Y-%m-%d")
            .to_string();

        let three_days_ago = chrono::Utc::now()
            .checked_sub_days(chrono::Days::new(3))
            .unwrap_or_default()
            .format("%Y-%m-%d")
            .to_string();

        let five_days_ago = chrono::Utc::now()
            .checked_sub_days(chrono::Days::new(5))
            .unwrap_or_default()
            .format("%Y-%m-%d")
            .to_string();

        // used unwrap because it should fail if it can't get fuel from string
        let fuel_100 = Fuel::from_str("100").unwrap();
        let fuel_200 = Fuel::from_str("200").unwrap();

        assert_eq!(result[0].date, two_days_ago);
        assert_eq!(result[0].paid, fuel_100);
        assert_eq!(result[0].unpaid, fuel_100);

        assert_eq!(result[1].date, three_days_ago);
        assert_eq!(result[1].paid, fuel_100);
        assert_eq!(result[1].unpaid, fuel_100);

        assert_eq!(result[2].date, five_days_ago);
        assert_eq!(result[2].paid, Fuel::new(0));
        assert_eq!(result[2].unpaid, fuel_200);
    }
}
