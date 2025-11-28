use reqwest::Client;
use serde::{Deserialize, Serialize};
use anyhow::Result;

pub struct BsvBankClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

impl BsvBankClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            api_key: None,
        }
    }
    
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }
}
