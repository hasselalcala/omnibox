use near_jsonrpc_client::{methods, JsonRpcClient};
use near_jsonrpc_primitives::types::transactions::{RpcTransactionError, TransactionInfo};
use near_primitives::hash::CryptoHash;
use near_primitives::views::TxExecutionStatus;
use std::sync::Arc;
use std::time::{Duration, Instant};
#[derive(Debug)]
pub struct TxSender {
    pub client: Arc<JsonRpcClient>,
    timeout: Duration,
}

impl TxSender {
    pub fn new(client: Arc<JsonRpcClient>, timeout: Duration) -> Self {
        Self { client, timeout }
    }

    pub async fn send_transaction(
        &self,
        request: methods::send_tx::RpcSendTransactionRequest,
    ) -> Result<
        near_jsonrpc_primitives::types::transactions::RpcTransactionResponse,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let sent_at = Instant::now();

        match self.client.call(request.clone()).await {
            Ok(response) => {
                self.log_response_time(sent_at);
                Ok(response)
            }
            Err(err) => {
                if let Some(RpcTransactionError::TimeoutError) = err.handler_error() {
                    let tx_hash = request.signed_transaction.get_hash();
                    let sender_account_id =
                        request.signed_transaction.transaction.signer_id().clone();
                    self.wait_for_transaction(tx_hash, sender_account_id, sent_at)
                        .await
                } else {
                    Err(err.into())
                }
            }
        }
    }

    async fn wait_for_transaction(
        &self,
        tx_hash: CryptoHash,
        sender_account_id: near_primitives::types::AccountId,
        sent_at: Instant,
    ) -> Result<
        near_jsonrpc_primitives::types::transactions::RpcTransactionResponse,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        loop {
            let response = self
                .client
                .call(methods::tx::RpcTransactionStatusRequest {
                    transaction_info: TransactionInfo::TransactionId {
                        tx_hash,
                        sender_account_id: sender_account_id.clone(),
                    },
                    wait_until: TxExecutionStatus::Final,
                })
                .await;

            if sent_at.elapsed() > self.timeout {
                return Err("Time limit exceeded for the transaction to be recognized".into());
            }

            match response {
                Ok(response) => {
                    self.log_response_time(sent_at);
                    return Ok(response);
                }
                Err(err) => {
                    if let Some(RpcTransactionError::TimeoutError) = err.handler_error() {
                        continue;
                    }
                    return Err(err.into());
                }
            }
        }
    }

    fn log_response_time(&self, sent_at: Instant) {
        let delta = sent_at.elapsed().as_secs();
        println!("Response received after: {}s", delta);
    }
}
