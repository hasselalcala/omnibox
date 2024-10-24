use near_crypto::InMemorySigner;
use near_jsonrpc_client::methods;
use near_jsonrpc_client::JsonRpcClient;
use near_jsonrpc_primitives::types::query::QueryResponseKind;
use near_primitives::hash::CryptoHash;
use near_primitives::types::BlockReference;
use near_primitives::views::QueryRequest;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct NonceManager {
    client: Arc<JsonRpcClient>,
    signer: Arc<InMemorySigner>,
    current_nonce: Mutex<u64>,
}

impl NonceManager {
    pub fn new(client: Arc<JsonRpcClient>, signer: Arc<InMemorySigner>) -> Self {
        Self {
            client,
            signer,
            current_nonce: Mutex::new(0),
        }
    }

    pub async fn get_nonce_and_tx_hash(
        &self,
    ) -> Result<(u64, CryptoHash), Box<dyn std::error::Error + Send + Sync>> {
        let access_key_query_response = self
            .client
            .call(methods::query::RpcQueryRequest {
                block_reference: BlockReference::latest(),
                request: QueryRequest::ViewAccessKey {
                    account_id: self.signer.account_id.clone(),
                    public_key: self.signer.public_key.clone(),
                },
            })
            .await?;

        match access_key_query_response.kind {
            QueryResponseKind::AccessKey(access_key) => {
                let mut current_nonce = self.current_nonce.lock().await;
                let new_nonce = std::cmp::max(access_key.nonce, *current_nonce) + 1;
                *current_nonce = new_nonce;
                println!("Using nonce: {}", new_nonce);
                Ok((new_nonce, access_key_query_response.block_hash))
            }
            _ => Err("Failed to extract current nonce".into()),
        }
    }
}
