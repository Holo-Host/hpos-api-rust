use crate::{
    common::types::{PendingTransactions, RedemptionState, Transaction, TransactionDirection},
    hpos::WsMutex,
};
use anyhow::Result;
use chrono::{DateTime, Days, Utc};
use holochain_types::prelude::{holochain_serial, SerializedBytes};
use holofuel_types::fuel::Fuel;
use hpos_hc_connect::app_connection::CoreAppRoleName;
use log::debug;
use rocket::{
    http::Status,
    serde::{json::Json, Deserialize, Serialize},
    {get, State},
};
use std::{collections::HashMap, str::FromStr};

use crate::hpos::Ws;

#[get("/redeemable_histogram")]
pub async fn redeemable_histogram(
    wsm: &State<WsMutex>,
) -> Result<Json<RedemableHolofuelHistogramResponse>, (Status, String)> {
    let mut ws = wsm.lock().await;
    let holofuel = get_redeemable_holofuel(&mut ws)
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;
    let dailies = get_last_weeks_redeemable_holofuel(&mut ws)
        .await
        .map_err(|e| (Status::InternalServerError, e.to_string()))?;
    Ok(Json(RedemableHolofuelHistogramResponse {
        dailies,
        redeemed: holofuel.available,
    }))
}

#[derive(Serialize, Deserialize, Debug, SerializedBytes, Clone)]
pub struct HolofuelPaidUnpaid {
    pub date: String,
    pub paid: Fuel,
    pub unpaid: Fuel,
}

#[derive(Serialize, Deserialize, Debug, SerializedBytes, Clone)]
pub struct RedemableHolofuelHistogramResponse {
    pub dailies: Vec<HolofuelPaidUnpaid>,
    pub redeemed: Fuel,
}

// get current redemable holofuel
pub async fn get_redeemable_holofuel(ws: &mut Ws) -> Result<RedemptionState> {
    let app_connection = ws.get_connection(ws.core_app_id.clone()).await?;

    debug!("calling zome holofuel/transactor/get_redeemable");
    let result = app_connection
        .zome_call_typed::<(), RedemptionState>(
            CoreAppRoleName::Holofuel.into(),
            "transactor".into(),
            "get_redeemable".into(),
            (),
        )
        .await?;

    Ok(result)
}

// get holofuel paid/unpaid by day for the last week
pub async fn get_last_weeks_redeemable_holofuel(ws: &mut Ws) -> Result<Vec<HolofuelPaidUnpaid>> {
    let app_connection = ws.get_connection(ws.core_app_id.clone()).await?;

    let one_week_ago = Utc::now()
        .checked_sub_days(Days::new(7))
        .unwrap_or_default()
        .timestamp();

    debug!("calling zome holofuel/transactor/get_completed_transactions");
    let completed_transactions = app_connection
        .zome_call_typed::<(), Vec<Transaction>>(
            CoreAppRoleName::Holofuel.into(),
            "transactor".into(),
            "get_completed_transactions".into(),
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
    let pending_transactions = app_connection
        .zome_call_typed::<(), PendingTransactions>(
            CoreAppRoleName::Holofuel.into(),
            "transactor".into(),
            "get_pending_transactions".into(),
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

    let grouped_transactions = group_transactions_by_day(filtered_transactions);
    Ok(add_missing_days(grouped_transactions))
}

fn timestamp_to_date(timestamp: i64) -> DateTime<Utc> {
    let date_time: DateTime<Utc> = DateTime::from_timestamp_millis(timestamp).unwrap();
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

fn add_missing_days(
    mut grouped_transactions_by_day: Vec<HolofuelPaidUnpaid>,
) -> Vec<HolofuelPaidUnpaid> {
    for i in 0..7 {
        let date = Utc::now()
            .checked_sub_days(Days::new(i))
            .unwrap_or_default()
            .format("%Y-%m-%d")
            .to_string();

        if !grouped_transactions_by_day.iter().any(|t| t.date == date) {
            grouped_transactions_by_day.push(HolofuelPaidUnpaid {
                date,
                paid: Fuel::new(0),
                unpaid: Fuel::new(0),
            });
        }
    }
    grouped_transactions_by_day
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use chrono::{Days, Utc};
    use holochain_types::{
        dna::{AgentPubKeyB64, EntryHashB64},
        prelude::Timestamp,
    };
    use holofuel_types::fuel::Fuel;
    use serde::{Deserialize, Serialize};
    use serde_yaml;

    use crate::{
        common::types::{Transaction, TransactionDirection, TransactionStatus, TransactionType},
        HolofuelPaidUnpaid,
    };

    use super::{add_missing_days, group_transactions_by_day};

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

    #[test]
    fn test_add_missing_days() {
        let result = add_missing_days(Vec::new());
        assert_eq!(result.len(), 7);

        let mut five_days_missing: Vec<HolofuelPaidUnpaid> = Vec::new();
        for i in 0..5 {
            let date = Utc::now()
                .checked_sub_days(Days::new(i))
                .unwrap_or_default()
                .format("%Y-%m-%d")
                .to_string();
            five_days_missing.push(HolofuelPaidUnpaid {
                date,
                paid: Fuel::new(1),
                unpaid: Fuel::new(1),
            });
        }

        let result = add_missing_days(five_days_missing);
        assert_eq!(result.len(), 7);
        let mut total_paid_len = 0;
        for i in result {
            if i.paid > Fuel::new(0) {
                total_paid_len = total_paid_len + 1;
            }
        }
        assert_eq!(total_paid_len, 5);
    }
}
