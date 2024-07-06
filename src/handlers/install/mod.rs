/*
Hosted Happ Installation Overview

Steps to install a happ bundle:
- Download the bundle to a particular location.
    - Use that location and install
- Install a servicelogger instance for hosted happ
    - We need to know the path to the base servicelogger
- Use that base servicelogger and install a new sl instance with the properties set as:
    { properties: {
        "bound_happ_id": <HHA Action Hash of Hosted Happ>,
        "bound_hha_dna": <Host's HHA DNA Hash> // Note: the above happ id should live in this hha dna's dht network
        "bound_hf_dna":  <Host's HF DNA Hash> // Note: Need to build in way to confirm this is the same hf dht network as publisher's hf instance
    }}
*/

pub mod helpers;
mod types;

use anyhow::{anyhow, Result};
use helpers::{do_sl_cloning, FixedDataForSlCloneCall};
use holochain_conductor_api::CellInfo;
use holochain_types::app::{DeleteCloneCellPayload, DisableCloneCellPayload};
use hpos_hc_connect::AppConnection;
use url::Url;

use crate::common::types::PresentedHappBundle;
use crate::hpos::Ws;
pub use helpers::update_happ_bundle;
use holochain_types::dna::ActionHashB64;
use holochain_types::prelude::{AppBundleSource, CapSecret, CloneCellId};
use hpos_hc_connect::sl_utils::{
    sl_get_current_time_bucket, sl_within_min_of_next_time_bucket, time_bucket_from_date,
    SL_BUCKET_SIZE_DAYS, SL_MINUTES_BEFORE_BUCKET_TO_CLONE,
};
pub use types::*;

pub async fn handle_install_app(ws: &mut Ws, data: types::InstallHappBody) -> Result<String> {
    log::debug!("Calling zome hosted/install with payload: {:?}", &data);
    let maybe_pubkey = ws.host_pub_key.clone();
    let base_sl = ws.base_sl.clone();
    let mut admin_connection = ws.admin.clone();
    let core_app_connection = ws.get_connection(ws.core_app_id.clone()).await?;

    // Note: We will be installing the hosted happ and their associated sl cells with the host pubkey
    let host_pub_key = helpers::get_host_pub_key(maybe_pubkey, core_app_connection).await?;

    let happ_bundle_details: PresentedHappBundle = helpers::get_app_details(
        core_app_connection,
        ActionHashB64::from_b64_str(&data.happ_id)?.into(),
    )
    .await?;
    match helpers::is_already_installed(&mut admin_connection, happ_bundle_details.id.to_string())
        .await?
    {
        true => {
            // NB: If app is already installed, then we only need to (re-)enable the happ bundle.
            helpers::handle_holochain_enable(&mut admin_connection, &data.happ_id).await?;
        }
        false => {
            // NB: If the happ has not yet been installed, we must take 4 steps: 1. install app's sl, enable app's and clone sl, 2. install app, 3. enable app
            // 1. Install the sl instance assigned to the hosted happ
            // Download the servicelogger source code for sl happ instance install
            let bundle_url = match base_sl.bundle_url {
                Some(url) => url,
                None => match base_sl.bundle_path {
                    Some(path) => match Url::from_file_path(path) {
                        Ok(url) => url,
                        Err(e) => return Err(anyhow!(
                            "Failed to install happ with `happ_id`: {:?}. Unable to read source code url for servicelogger.  Error: {:?}", data.happ_id, e
                        ))
                },
                    None => return Err(anyhow!(
                        "Failed to install happ with `happ_id`: {:?}. Unable to locate source code url for servicelogger.", data.happ_id
                    ))
                }
            };

            let core_happ_cell_info = core_app_connection.app_info().await?.cell_info;

            log::debug!(
                "Downloading bundle URL...{:?}",
                happ_bundle_details.bundle_url
            );
            let sl_bundle_path = hpos_hc_connect::utils::download_file(&bundle_url).await?;

            helpers::install_assigned_sl_instance(
                ws,
                &data.happ_id,
                host_pub_key.to_owned(),
                &core_happ_cell_info,
                AppBundleSource::Path(sl_bundle_path),
                SL_BUCKET_SIZE_DAYS,
                sl_get_current_time_bucket(SL_BUCKET_SIZE_DAYS),
            )
            .await?;

            // Steps 2 & 3 are only for non-core hosted apps (ie: whenever the app does not have the `special_installed_app_id` property)
            if happ_bundle_details.special_installed_app_id.is_none() {
                // 3. Install the hosted happ
                // Download the app source code to install
                let bundle_url = Url::parse(&happ_bundle_details.bundle_url)?;
                log::debug!(
                    "Downloading bundle URL...{:?}",
                    happ_bundle_details.bundle_url
                );
                let happ_bundle_path = hpos_hc_connect::utils::download_file(&bundle_url).await?;

                // Install app
                let raw_payload = types::RawInstallAppPayload {
                    source: AppBundleSource::Path(happ_bundle_path),
                    agent_key: host_pub_key.to_owned(),
                    installed_app_id: happ_bundle_details.id.to_string(),
                    membrane_proofs: data.membrane_proofs,
                    uid: happ_bundle_details.uid,
                };

                helpers::handle_install_app_raw(&mut admin_connection, raw_payload).await?;

                // 4. Enable the hosted happ
                helpers::handle_holochain_enable(&mut admin_connection, &data.happ_id).await?;
            }
        }
    }

    Ok(format!(
        "Successfully installed happ_id: {:?}",
        data.happ_id
    ))
}

pub async fn handle_check_service_loggers(ws: &mut Ws) -> Result<CheckServiceLoggersResult> {
    let mut result = CheckServiceLoggersResult {
        service_loggers_cloned: 0,
        service_loggers_deleted: 0,
    };
    let apps = ws.admin.list_enabled_apps().await?;

    // It would be nice to only get the core_hap_cell_info if we are actually going to do an clone,
    // but I cant put it in the loop because it make a double mutable borrow of ws, and I don't know how
    // to get around that.
    let core_app_connection: &mut AppConnection = ws.get_connection(ws.core_app_id.clone()).await?;
    let core_happ_cell_info: std::collections::HashMap<String, Vec<CellInfo>> =
        core_app_connection.app_info().await?.cell_info;

    let mut maybe_sl_clone_data: Option<FixedDataForSlCloneCall> = None;

    let current_time_bucket = sl_get_current_time_bucket(SL_BUCKET_SIZE_DAYS);
    let current_time_bucket_name = format!("{}", current_time_bucket);

    let clone_for_next =
        sl_within_min_of_next_time_bucket(SL_BUCKET_SIZE_DAYS, SL_MINUTES_BEFORE_BUCKET_TO_CLONE);
    let next_time_bucket_name = format!("{}", current_time_bucket + 1);

    for happ_id in apps
        .into_iter()
        .filter(|id| id.ends_with("::servicelogger"))
    {
        let app_ws = ws.get_connection(happ_id.clone()).await?;
        let clone_cells = app_ws.clone_cells("servicelogger".into()).await?;
        log::debug!(
            "Checking {} for cells {:?} for bucket {}, clone_for_next {}",
            happ_id,
            clone_cells,
            current_time_bucket_name,
            clone_for_next
        );
        // if there is no clone cell for the current bucket, the we gotta make it!
        if clone_cells
            .clone()
            .into_iter()
            .find(|cell| cell.name == current_time_bucket_name)
            .is_none()
        {
            if maybe_sl_clone_data.is_none() {
                maybe_sl_clone_data = Some(FixedDataForSlCloneCall::init(
                    &core_happ_cell_info,
                    SL_BUCKET_SIZE_DAYS,
                    current_time_bucket,
                )?);
            }
            if let Some(ref sl_clone_data) = maybe_sl_clone_data {
                if do_sl_cloning(app_ws, &happ_id, sl_clone_data).await? {
                    result.service_loggers_cloned += 1;
                }
            }
        }
        if result.service_loggers_cloned > 0 {
            let clone_cells = app_ws.clone_cells("servicelogger".into()).await?;
            log::debug!("CLONE CELLS AFTER: {:?}", clone_cells);
        }

        // if we are just before the next time bucket, and that bucket doesn't exist, also clone!
        if clone_for_next {
            // reset clone data
            maybe_sl_clone_data = None;
            if clone_cells
                .clone()
                .into_iter()
                .find(|cell| cell.name == next_time_bucket_name)
                .is_none()
            {
                if maybe_sl_clone_data.is_none() {
                    maybe_sl_clone_data = Some(FixedDataForSlCloneCall::init(
                        &core_happ_cell_info,
                        SL_BUCKET_SIZE_DAYS,
                        current_time_bucket + 1,
                    )?);
                }
                if let Some(ref sl_clone_data) = maybe_sl_clone_data {
                    if do_sl_cloning(app_ws, &happ_id, sl_clone_data).await? {
                        result.service_loggers_cloned += 1;
                    }
                }
            }

            let mut deleteable: Vec<CloneCellId> = Vec::new();
            // also, for any old cells, check to see if we can delete it by confirming that all logs have
            // been invoiced, and all those invoices aren'd pending, by comparing the CapSecrets
            debug!("calling zome holofuel/transactor/get_pending_invoices");
            let core_app_connection = ws.get_connection(ws.core_app_id.clone()).await?;

            let pending = core_app_connection
                .zome_call_typed::<(), RedemptionState>(
                    CoreAppRoleName::Holofuel.into(),
                    "transactor".into(),
                    "get_pending_invoices".into(),
                    (),
                )
                .await?;
            let mut pending_secrets: HashSet<CapSecret> = HashSet::new();
            for invoice in pending.invoice_pending {
                if let Some(pos) = invoice.proof_of_service {
                    if let POS::Hosting(secret) = pos {
                        pending_secrets.insert(secret)
                    }
                }
            }

            for cell in clone_cells {
                let cell_time_bucket_result = cell.name.parse::<u32>();
                if let Ok(cell_time_bucket) = cell_time_bucket_result {
                    // only query cells that are more than 2 time_buckets in the past (1 month)
                    if cell_time_bucket < current_time_bucket - 2 {
                        log::debug!(
                            "Calling all_invoiced for happ: {}::servicelogger.{} ",
                            happ_id,
                            cell_time_bucket
                        );
                        let result: Result<Option<Vec<CapSecret>>> = app_ws
                            .clone_zome_call_typed(
                                "servicelogger".into(),
                                cell.name,
                                "service".into(),
                                "all_invoiced".into(),
                                (),
                            )
                            .await;
                        match result {
                            Ok(all_invoiced) => {
                                if let Some(secrets) = all_invoiced {
                                    let s = HashSet::from_iter(secrets.into_iter());
                                    if s.is_disjoint(s) {
                                        deleteable.push(CloneCellId::CloneId(cell.clone_id));
                                    }
                                }
                            }
                            Err(err) => {
                                log::warn!(
                                    "Error while checking service logger {}.{}: {:?}",
                                    happ_id,
                                    cell_time_bucket,
                                    err
                                );
                            }
                        }
                    }
                }
            }
            // cells muist be disabled before they can be deleted.
            for clone_cell_id in deleteable.clone() {
                let payload = DisableCloneCellPayload {
                    clone_cell_id: clone_cell_id.clone(),
                };
                app_ws.disable_clone(payload).await?;
            }
            for clone_cell_id in deleteable {
                let payload = DeleteCloneCellPayload {
                    app_id: happ_id.clone(),
                    clone_cell_id,
                };
                let x = ws
                    .admin
                    .delete_clone(payload)
                    .await
                    .map_err(|err| anyhow!("Failed to delete clone cell: {:?}", err));
                log::debug!("DELETE CLONE RESULT {:?}", x);
                x?;
                result.service_loggers_deleted += 1;
            }
        }
    }
    Ok(result)
}
