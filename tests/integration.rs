mod utils;

use log::{debug, info};
use rocket::tokio;
use utils::Test;

#[tokio::test]
async fn install_components() {
    env_logger::init();

    let mut test = Test::init().await;

    // Start API

    // Make some calls, starting with `/`
}