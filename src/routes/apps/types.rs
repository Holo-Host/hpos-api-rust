use rocket::serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct HostedRegisterRequestBody {
  pub name: String,
  pub hosted_urls: Vec<String>,
  pub bundle_url: String,
  pub dnas: Vec<String>,
  pub special_installed_app_id: Option<String>,
  pub network_seed: String,
}