use hpos_api_rust::rocket;

#[rocket::main]
async fn main() {
    // entire rocket() is moved to module so that I can call it in integration test
    let _ = rocket().await.launch().await;
}
