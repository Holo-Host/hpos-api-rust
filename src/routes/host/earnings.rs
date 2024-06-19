use std::str::FromStr;

use anyhow::{anyhow, Result};
use holochain_types::prelude::Timestamp;
use holofuel_types::{error::FuelError, fuel::Fuel};
use hpos_hc_connect::{app_connection::CoreAppRoleName, AppConnection};
use rocket::{
    get,
    http::Status,
    serde::{json::Json, Deserialize, Serialize},
    State,
};

use crate::{common::types::RedemptionState, hpos::WsMutex};
use crate::{
    common::types::{Ledger, Transaction},
    hpos::Ws,
};

use crate::routes::host::shared::{
    get_hosting_invoices, HostingInvoicesResponse, InvoiceSet, TransactionAndInvoiceDetails,
};

/// Returns overview of host earnings as needed for the host-console-ui dashboard page
/// -- includes optional cutoff quantity param to control the volume of recent hosting payments to return to client
#[get("/earnings?<quantity>")]
pub async fn earnings(
    wsm: &State<WsMutex>,
    quantity: Option<u16>,
) -> Result<Json<HostEarningsResponse>, (Status, String)> {
    let quantity = quantity.unwrap_or(0);

    let mut ws = wsm.lock().await;

    Ok(Json(handle_earnings(&mut ws, quantity).await.map_err(
        |err| {
            dbg!(&err);
            (Status::InternalServerError, err.to_string())
        },
    )?))
}

async fn handle_earnings(ws: &mut Ws, quantity: u16) -> Result<HostEarningsResponse> {
    let core_app_connection: &mut AppConnection =
        ws.get_connection(ws.core_app_id.clone()).await.unwrap();

    let HostingInvoicesResponse {
        paid_hosting_invoices,
        transaction_and_invoice_details,
        ..
    } = get_hosting_invoices(core_app_connection.to_owned(), InvoiceSet::All).await?;

    let transaction_and_invoice_details = if quantity > 0 {
        transaction_and_invoice_details
            .into_iter()
            .take(quantity.into())
            .collect()
    } else {
        transaction_and_invoice_details
    };

    let earnings = calculate_earnings(paid_hosting_invoices)?;

    let ledger: Ledger = core_app_connection
        .zome_call_typed(
            CoreAppRoleName::Holofuel.into(),
            "transactor".into(),
            "get_ledger".into(),
            (),
        )
        .await?;

    let redemption_state: RedemptionState = core_app_connection
        .zome_call_typed(
            CoreAppRoleName::Holofuel.into(),
            "transactor".into(),
            "get_redeemable".into(),
            (),
        )
        .await?;

    Ok(HostEarningsResponse {
        earnings,
        holofuel: HolofuelBalances {
            redeemable: redemption_state.available,
            balance: ledger.balance,
            available: ledger.available,
        },
        recent_payments: transaction_and_invoice_details,
    })
}

fn calculate_earnings(transactions: Vec<Transaction>) -> Result<Earnings> {
    // this is ineffecient, we loop over `transactions` 3 times. If we care we could speed this up
    Ok(Earnings {
        last30days: calculate_earnings_in_days(30, &transactions)?,
        last7days: calculate_earnings_in_days(7, &transactions)?,
        lastday: calculate_earnings_in_days(1, &transactions)?,
    })
}

fn calculate_earnings_in_days(days: u64, transactions: &Vec<Transaction>) -> Result<Fuel> {
    let days_ago = (Timestamp::now() - core::time::Duration::new(days * 24 * 60 * 60, 0))?;

    let result_of_vec_of_fuels: Result<Vec<Fuel>, FuelError> = transactions
        .into_iter()
        .filter(|tx| {
            if let Some(completed_date) = tx.completed_date {
                completed_date > days_ago
            } else {
                false
            }
        })
        .map(|tx| Fuel::from_str(&tx.amount))
        .collect();

    let vec_of_fuels = result_of_vec_of_fuels.map_err(|e| {
        anyhow!(
            "Failed to convert transaction amounts to Fuel in calculate_earnings_in_days: {:?}",
            e
        )
    })?;

    vec_of_fuels
        .into_iter()
        .try_fold(Fuel::new(0), |acc, tx_fuel| acc + tx_fuel)
        .map_err(|e| anyhow!("Failed to sum Fuel in calculate_earnings_in_days: {:?}", e))
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct HostEarningsResponse {
    earnings: Earnings,
    holofuel: HolofuelBalances,
    recent_payments: Vec<TransactionAndInvoiceDetails>,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct Earnings {
    last30days: Fuel,
    last7days: Fuel,
    lastday: Fuel,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct HolofuelBalances {
    redeemable: Fuel,
    balance: Fuel,
    available: Fuel,
}
