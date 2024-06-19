#[derive(Debug, Serialize)]
pub struct holo_client_auth {
    id: String,
    email: String,
    accessToken: String,
    permissions: String,
    profileImage: String,
    displayName: String,
    kyc: String,
    jurisdiction: String,
    publicKey: String,
}