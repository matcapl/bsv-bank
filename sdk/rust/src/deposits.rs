use super::client::BsvBankClient;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct CreateDepositRequest {
    pub user_paymail: String,
    pub amount_satoshis: i64,
    pub txid: String,
    pub lock_duration_days: i32,
}

#[derive(Debug, Deserialize)]
pub struct Deposit {
    pub id: String,
    pub paymail: String,
    pub amount_satoshis: i64,
    pub status: String,
}

impl BsvBankClient {
    pub async fn create_deposit(&self, req: CreateDepositRequest) -> Result<Deposit> {
        let url = format!("{}/deposits", self.base_url);
        let response = self.client.post(&url)
            .json(&req)
            .send()
            .await?
            .json::<Deposit>()
            .await?;
        Ok(response)
    }
    
    pub async fn get_balance(&self, paymail: &str) -> Result<i64> {
        let url = format!("{}/balance/{}", self.base_url, paymail);
        // Implementation...
        Ok(0)
    }
}
