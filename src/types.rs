use rocket::{
    response::{Debug, Responder},
    serde::{Deserialize, Serialize},
};
use anyhow::Error;

// Return value of hosted_happs
#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct HappDetails {

}

// [rocket::response::Debug](https://api.rocket.rs/v0.5-rc/rocket/response/struct.Debug.html) implements Responder to Error
pub type Result<T, E = Debug<Error>> = std::result::Result<T, E>;

// Debug errors default to 500
pub type Error500 = Debug<Error>;

#[derive(Responder, Debug)]
#[response(status = 400)]
pub enum Error400 {
    Info(String),
    Message(&'static str),
}

#[derive(Responder, Debug)]
#[response(status = 404)]
pub enum Error404 {
    Info(String),
    Message(&'static str),
}

#[derive(Responder, Debug)]
pub enum ApiError {
    BadRequest(Error400),
    NotFound(Error404),
}
