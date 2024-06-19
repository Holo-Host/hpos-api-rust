
use rocket::{
    http::Status,
    serde::{json::Json, Deserialize, Serialize},
    {get, State},
};
use anyhow::Result;

use crate::hpos::{Ws, WsMutex};

/// Returns list of all host invoices as needed for the host-console-ui invoice page
/// -- includes optional invoice_set {all, unpaid, paid} param to allow querying the invoices by their status
#[get("/usage?<usage_interval>")]
pub async fn usage(wsm: &State<WsMutex>, usage_interval: i64) -> Result<Json<UsageResponse>, (Status, String)> {
    let mut ws = wsm.lock().await;    

    Ok(Json(handle_usage(&mut ws, usage_interval).await.map_err(
        |e| (Status::InternalServerError, e.to_string()),
    )?))
}

async fn handle_usage(
    ws: &mut Ws,
    usage_interval: i64,
) -> Result<UsageResponse> {
    let all_hosted_happs = crate::handlers::hosted_happs::handle_get_all(
        usage_interval,
        None,
        ws
    ).await?;

    Ok(all_hosted_happs
        .into_iter()
        .fold(UsageResponse::default(), |acc, happ| {
            if !happ.enabled { // is this logic right? Isn't it possible for a happ to have some usage but now be disabled?
                return acc
            }

            let UsageResponse {
                total_hosted_agents,
                current_total_storage,
                total_hosted_happs,
                total_usage,
            } = acc;

            let TotalUsage {
                cpu,
                bandwidth,
            } = total_usage;

            let happ_usage = happ.usage.unwrap_or_default();

            UsageResponse {
                total_hosted_agents: total_hosted_agents + happ.source_chains.unwrap_or_default(),
                current_total_storage: current_total_storage + happ_usage.disk_usage,
                total_hosted_happs: total_hosted_happs + 1,
                total_usage: TotalUsage {
                    cpu: cpu + happ_usage.cpu,
                    bandwidth: bandwidth + happ_usage.bandwidth,
                },
            }
        }))    
}

#[derive(Serialize, Deserialize, Default)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct UsageResponse {
    total_hosted_agents: u16,
    current_total_storage: u64,
    total_hosted_happs: u16,
    total_usage: TotalUsage,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct TotalUsage {
    cpu: u64,
    bandwidth: u64,
}