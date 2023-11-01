use rocket::{self, get, launch};

#[get("/")]
async fn index() -> &'static str {
    "I'm your holoport 🤖"
}

#[launch]
async fn rocket() -> _ {
    rocket::build()
        .mount("/", rocket::routes![index])
}
