use std::time::{SystemTime, UNIX_EPOCH};

// Define the struct
pub struct auth_payload {
    pub email: String,
    pub timestamp: u64,
    pub pub_key: String,
}

impl auth_payload {
    pub fn new(email: String, pub_key: String) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();
        
        auth_payload {
            email,
            timestamp,
            pub_key,
        }
    }

    // Method to convert the struct into bytes
    pub fn into_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        bytes.extend(self.email.as_bytes());
        bytes.extend(&self.timestamp.to_be_bytes());
        bytes.extend(self.pub_key.as_bytes());

        bytes
    }
}