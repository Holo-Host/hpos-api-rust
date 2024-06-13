use holofuel_types::fuel::Fuel;
use rocket::{
    http::Status,
    serde::{json::Json, Deserialize, Serialize},
    {get, State},
};

use crate::hpos::WsMutex;

/// Returns overview of host earnings as needed for the host-console-ui dashboard page
/// -- includes optional cutoff quantity param to control the volume of recent hosting payments to return to client
#[get("/earnings?<quantity>")]
pub async fn earnings(wsm: &State<WsMutex>, quantity: u16) -> Result<Json<()>, (Status, String)> {
    let mut ws = wsm.lock().await;

    Ok(Json(()))
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct HostEarningsResponse {
    earnings: LastEarnings,
    holofuel: HolofuelBalances,
    recent_payments: RecentPaymentsDetails,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct LastEarnings {
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

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct RecentPaymentsDetails {
    // id: 1,
    // amount: Fuel,
    // status: 'received',
    // updatedAt: Date.now(),
    // happ: {
    // name: 'HoloFuel',
    // id: 123
    // },
    // invoiceDetails: {
    // start: '',
    // end: '',
    // bandwidth: {
    //     price: 1234, // hosting bandwidth prices hf/mb
    //     quantity: 1 // traffic serviced (should be in mb) - to calculate bandwidth,
    // },
    // compute: {
    //     price: 12,
    //     quantity: 234
    // },
    // storage: {
    //     price: 432
    // }
    // }
}
