use rocket::{
    http::Status,
    serde::json::Json,
    {get, State},
};

use crate::hpos::WsMutex;

/// Returns default hosting preferences in units defined in servicelogger prefs
/// https://github.com/Holo-Host/servicelogger-rsm/blob/develop/zomes/service_integrity/src/entries/logger_settings.rs#L8
/// Those default prefs are applied to each hosted happ by `holo-auto-installer` at the time of installation
#[get("/billing_preferences")]
pub async fn billing_preferences(
    wsm: &State<WsMutex>,
) -> Result<Json<()>, (Status, String)> {
    let mut ws = wsm.lock().await;

    // res.status(200).send(yaml.load(fs.readFileSync(SL_PREFS_PATH, 'utf8')))

    Ok(Json(()))
}
