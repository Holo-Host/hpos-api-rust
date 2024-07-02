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

mod helpers;
mod types;

use anyhow::{anyhow, Result};
use url::Url;

use crate::common::types::PresentedHappBundle;
use hpos_hc_connect::sl_utils::{sl_get_current_time_bucket, SL_BUCKET_SIZE_DAYS};
use crate::hpos::Ws;
pub use helpers::update_happ_bundle;
use holochain_types::dna::ActionHashB64;
use holochain_types::prelude::AppBundleSource;
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
            // NB: If the happ has not yet been installed, we must take 4 steps: 1. install app's sl, 2. enable app's sl, 3. install app, 4. enable app
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

            let sl_app_id = match helpers::install_assigned_sl_instance(
                &mut admin_connection,
                &data.happ_id,
                host_pub_key.to_owned(),
                &core_happ_cell_info,
                AppBundleSource::Path(sl_bundle_path),
                SL_BUCKET_SIZE_DAYS,
                sl_get_current_time_bucket(SL_BUCKET_SIZE_DAYS)
            )
            .await?
            {
                SuccessfulInstallResult::New(a) => a.installed_app_id,
                SuccessfulInstallResult::AlreadyInstalled => helpers::get_sl_id(&data.happ_id),
            };

            // 2. Enable the sl instance assigned to the hosted happ
            helpers::handle_holochain_enable(&mut admin_connection, &sl_app_id).await?;

            // Steps 3 & 4 are only for non-core hosted apps (ie: whenever the app does not have the `special_installed_app_id` property)
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

pub async fn handle_clone_service_logger(ws: &mut Ws, data: ServiceLoggerTimeBucket) -> Result<String> {
    Ok(format!(
        "Not implmented to clone service logger: {:?}",
        data.version
    ))
}