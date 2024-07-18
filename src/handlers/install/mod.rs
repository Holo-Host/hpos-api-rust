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

use std::collections::HashSet;

use anyhow::{anyhow, Result};
use helpers::{do_sl_cloning, FixedDataForSlCloneCall};
use holochain_conductor_api::CellInfo;
use holochain_types::app::{DeleteCloneCellPayload, DisableCloneCellPayload};
use hpos_hc_connect::app_connection::CoreAppRoleName;
use hpos_hc_connect::holofuel_types::{Pending, POS};
use hpos_hc_connect::AppConnection;
use url::Url;

use super::hosted_happs::handle_enable;
use crate::common::types::PresentedHappBundle;
use crate::hpos::Ws;
pub use helpers::update_happ_bundle;
use holochain_types::dna::ActionHashB64;
use holochain_types::prelude::{AppBundleSource, CapSecret, CloneCellId};
use hpos_hc_connect::sl_utils::{
    sl_clone_name, sl_clone_name_spec, sl_get_current_time_bucket, sl_within_deleting_check_window,
    sl_within_min_of_next_time_bucket, SlCloneSpec, SL_BUCKET_SIZE_DAYS,
    SL_DELETING_LOG_WINDOW_SIZE_MINUTES, SL_MINUTES_BEFORE_BUCKET_TO_CLONE,
};
use std::iter::FromIterator;
pub use types::*;

pub async fn handle_install_app(ws: &mut Ws, data: types::InstallHappBody) -> Result<String> {
    log::debug!("Calling zome hosted/install with payload: {:?}", &data);
    let maybe_pubkey = ws.host_pub_key.clone();
    let base_sl = ws.base_sl.clone();
    let mut admin_connection = ws.admin.clone();
    let core_app_connection = ws.get_connection(ws.core_app_id.clone()).await?;

    // Note: We will be installing the hosted happ and their associated sl cells with the host pubkey
    let host_pub_key = helpers::get_host_pub_key(maybe_pubkey, core_app_connection).await?;

    let happ_bundle_details: PresentedHappBundle =
        helpers::get_app_details(core_app_connection, data.happ_id.clone().into()).await?;
    match helpers::is_already_installed(&mut admin_connection, happ_bundle_details.id.to_string())
        .await?
    {
        true => {
            // NB: If app is already installed, then we only need to make the happ as enable in hha.
            handle_enable(ws, data.happ_id.clone()).await?;
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
                helpers::handle_holochain_enable(&mut admin_connection, &data.happ_id.to_string())
                    .await?;
            }
            handle_enable(ws, data.happ_id.clone()).await?;
        }
    }

    Ok(format!(
        "Successfully installed happ_id: {:?}",
        data.happ_id
    ))
}

/// this function implements the things that need to be checked periodically about service loggers
/// 1. cloning new instances
/// 2. deleting old instances where all logs have been invoiced and paid.
/// So we set up contexts and get the data we need, and then run a loop across all service-logger instances.
pub async fn handle_check_service_loggers(ws: &mut Ws) -> Result<CheckServiceLoggersResult> {
    let mut result = CheckServiceLoggersResult {
        service_loggers_cloned: Vec::new(),
        service_loggers_deleted: Vec::new(),
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
    let current_time_bucket_name = sl_clone_name(SlCloneSpec {
        days_in_bucket: SL_BUCKET_SIZE_DAYS,
        time_bucket: current_time_bucket,
    });

    // find out if we are being run in the time right before the next clone so that we have to
    // check for doing cloning for the next time bucket rather than just this one (which we still should do)
    let check_cloning_for_next_bucket =
        sl_within_min_of_next_time_bucket(SL_BUCKET_SIZE_DAYS, SL_MINUTES_BEFORE_BUCKET_TO_CLONE);
    let next_time_bucket_name = sl_clone_name(SlCloneSpec {
        days_in_bucket: SL_BUCKET_SIZE_DAYS,
        time_bucket: current_time_bucket + 1,
    });

    let check_for_deleting = sl_within_deleting_check_window(SL_DELETING_LOG_WINDOW_SIZE_MINUTES);

    let mut pending_secrets: HashSet<CapSecret> = HashSet::new();
    // we are likely going to need the pending transactions if we are going to be checking for deletability
    // so get them once here outside the happ_id loop.
    if check_for_deleting {
        let pending = core_app_connection
            .zome_call_typed::<(), Pending>(
                CoreAppRoleName::Holofuel.into(),
                "transactor".into(),
                "get_pending_transactions".into(),
                (),
            )
            .await?;
        for invoice in pending.invoice_pending {
            if let Some(POS::Hosting(secret)) = invoice.proof_of_service {
                pending_secrets.insert(secret);
            }
        }
    }

    for installed_happ_id in apps
        .into_iter()
        .filter(|id| id.ends_with("::servicelogger"))
    {
        let app_ws = ws.get_connection(installed_happ_id.clone()).await?;
        let cloned_cells = app_ws.cloned_cells("servicelogger".into()).await?;
        log::debug!(
            "Checking {} for cells {:?} for bucket {}, check_cloning_for_next_bucket {}",
            installed_happ_id,
            cloned_cells,
            current_time_bucket_name,
            check_cloning_for_next_bucket
        );
        let happ_id_str = installed_happ_id.split("::").next().unwrap(); // safe because id is checked in the loop.
        let happ_id = ActionHashB64::from_b64_str(happ_id_str)?;

        // if there is no clone cell for the current bucket, the we gotta make it!
        if cloned_cells
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
                let cell: Option<holochain_types::prelude::ClonedCell> =
                    do_sl_cloning(app_ws, &happ_id, sl_clone_data).await?;
                if let Some(c) = cell {
                    result
                        .service_loggers_cloned
                        .push((happ_id.to_string(), c.name));
                }
            }
        }

        // if we are just before the next time bucket, and that bucket doesn't exist, also clone!
        if check_cloning_for_next_bucket {
            // reset clone data
            maybe_sl_clone_data = None;
            if cloned_cells
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
                    let cell = do_sl_cloning(app_ws, &happ_id, sl_clone_data).await?;
                    if let Some(c) = cell {
                        result
                            .service_loggers_cloned
                            .push((happ_id.to_string(), c.name));
                    }
                }
            }
        }

        if check_for_deleting {
            let mut deleteable: Vec<(CloneCellId, String)> = Vec::new();
            // also, for any old cells, check to see if we can delete it by confirming that all logs have
            // been invoiced, and all those invoices aren't pending, by comparing the CapSecrets

            for cell in cloned_cells {
                let spec_result = sl_clone_name_spec(&cell.name);
                if let Ok(SlCloneSpec {
                    time_bucket: cell_time_bucket,
                    days_in_bucket: _,
                }) = spec_result
                {
                    // only query cells that are more than 2 time_buckets in the past (1 month)
                    if cell_time_bucket < current_time_bucket - 2 {
                        let result: Result<Option<Vec<CapSecret>>> = app_ws
                            .clone_zome_call_typed(
                                "servicelogger".into(),
                                cell.name.clone(),
                                "service".into(),
                                "all_invoiced".into(),
                                (),
                            )
                            .await;
                        match result {
                            Ok(all_invoiced) => {
                                if let Some(secrets) = all_invoiced {
                                    let s: HashSet<CapSecret> = HashSet::from_iter(secrets);
                                    // if there are no secrets in common in the two sets, we know
                                    // all the invoiced items aren't pending, so we can delete this cell.
                                    if s.is_disjoint(&s) {
                                        deleteable
                                            .push((CloneCellId::CloneId(cell.clone_id), cell.name));
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
            // cells must be disabled before they can be deleted.
            for cell_data in deleteable.clone() {
                let payload = DisableCloneCellPayload {
                    clone_cell_id: cell_data.0,
                };
                app_ws.disable_clone(payload).await?;
            }
            for cell_data in deleteable {
                let payload = DeleteCloneCellPayload {
                    app_id: installed_happ_id.clone(),
                    clone_cell_id: cell_data.0,
                };
                ws.admin
                    .delete_clone(payload)
                    .await
                    .map_err(|err| anyhow!("Failed to delete clone cell: {:?}", err))?;
                result
                    .service_loggers_deleted
                    .push((happ_id.to_string(), cell_data.1));
            }
        }
    }
    Ok(result)
}
