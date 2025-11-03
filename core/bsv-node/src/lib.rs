use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::error::Error;
use base64::{Engine as _, engine::general_purpose};

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcRequest {
    jsonrpc: String,
    method: String,
    params: Vec<serde_json::Value>,
    id: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcResponse<T> {
    pub result: Option<T>,
    pub error: Option<RpcError>,
    pub id: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub txid: String,
    pub confirmations: u32,
    pub blockhash: Option<String>,
    pub blockheight: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockHeader {
    pub hash: String,
    pub confirmations: u32,
    pub height: u64,
    pub merkleroot: String,
}

pub struct BsvNodeClient {
    url: String,
    username: String,
    password: String,
    client: reqwest::Client,
}

impl BsvNodeClient {
    pub fn new(url: String, username: String, password: String) -> Self {
        Self {
            url,
            username,
            password,
            client: reqwest::Client::new(),
        }
    }

    async fn call_rpc<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<T, Box<dyn Error>> {
        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            method: method.to_string(),
            params,
            id: 1,
        };

        let auth = general_purpose::STANDARD.encode(format!("{}:{}", self.username, self.password));

        let response = self.client
            .post(&self.url)
            .header("Authorization", format!("Basic {}", auth))
            .json(&request)
            .send()
            .await?;

        let rpc_response: RpcResponse<T> = response.json().await?;

        if let Some(error) = rpc_response.error {
            return Err(format!("RPC Error: {} ({})", error.message, error.code).into());
        }

        rpc_response.result.ok_or("No result in response".into())
    }

    pub async fn get_transaction(&self, txid: &str) -> Result<Transaction, Box<dyn Error>> {
        self.call_rpc("getrawtransaction", vec![
            serde_json::json!(txid),
            serde_json::json!(true),
        ]).await
    }

    pub async fn get_block_header(&self, hash: &str) -> Result<BlockHeader, Box<dyn Error>> {
        self.call_rpc("getblockheader", vec![serde_json::json!(hash)]).await
    }

    pub async fn verify_spv(&self, txid: &str, min_confirmations: u32) -> Result<bool, Box<dyn Error>> {
        let tx = self.get_transaction(txid).await?;
        Ok(tx.confirmations >= min_confirmations)
    }

    pub async fn send_raw_transaction(&self, hex: &str) -> Result<String, Box<dyn Error>> {
        self.call_rpc("sendrawtransaction", vec![serde_json::json!(hex)]).await
    }

    pub async fn get_block_count(&self) -> Result<u64, Box<dyn Error>> {
        self.call_rpc("getblockcount", vec![]).await
    }

    pub fn create_op_return_data(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hasher.finalize();
        format!("6a{}", hex::encode(hash))
    }
}
