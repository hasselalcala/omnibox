use crate::block_streamer::extract_logs;
use crate::constants::ACCOUNT_TO_LISTEN;
use crate::models::EventData;
use crate::nonce_manager::NonceManager;
use crate::qx_builder::QueryBuilder;
use crate::qx_sender::QuerySender;
use crate::tx_builder::TxBuilder;
use crate::tx_sender::TxSender;

use async_trait::async_trait;
use near_jsonrpc_client::methods;
use near_primitives::views::TxExecutionStatus;
use near_sdk::AccountId;
use tokio::time::{sleep, Duration};

use std::sync::Arc;
use tokio::sync::Mutex;

use super::TransactionProcessor;

pub struct Miner {
    nonce_manager: Arc<NonceManager>,
    tx_builder: Arc<Mutex<TxBuilder>>,
    tx_sender: Arc<TxSender>,
    db: Arc<Mutex<rocksdb::DB>>,
    account_id: AccountId,
}

impl Miner {
    pub fn new(
        nonce_manager: Arc<NonceManager>,
        tx_builder: Arc<Mutex<TxBuilder>>,
        tx_sender: Arc<TxSender>,
        db: Arc<Mutex<rocksdb::DB>>,
        account_id: AccountId,
    ) -> Self {
        Self {
            nonce_manager,
            tx_builder,
            tx_sender,
            db,
            account_id,
        }
    }
}

#[async_trait]
impl TransactionProcessor for Miner {
    async fn process_transaction(
        &self,
        event_data: EventData,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        println!("Miner Processor");
        println!("Miner Event Data: {:?}", event_data);

        let commit_attempts = 30;
        let reveal_attempts = 30;
        let mut committed = false;

        // Wait for CommitMiners stage
        for _attempt in 0..commit_attempts {
            let stage_result = self
                .get_stage(self.tx_sender.client.clone(), event_data.clone())
                .await?;
            let stage = stage_result.trim_matches('"').to_string();
            println!("Current Stage: {:?}", stage);

            if stage == "CommitMiners" {
                match self.commit(event_data.clone()).await {
                    Ok(_) => {
                        committed = true;
                        break;
                    }
                    Err(e) => {
                        println!("Failed to commit by miner: {}", e);
                        return Err(e);
                    }
                }
            } else if stage == "RevealMiners" || stage == "CommitValidators" || stage == "RevealValidators" || stage == "Ended" {
                println!("Commit stage passed without committing, skipping transaction.");
                return Ok(false);
            } else {
                println!("Waiting for CommitMiners stage...");
                sleep(Duration::from_secs(10)).await;
            }
        }

        if !committed {
            println!("Failed to reach CommitMiners stage, skipping transaction.");
            return Ok(false);
        }

        // Wait for RevealMiners stage
        for _attempt in 0..reveal_attempts {
            let stage_result = self
                .get_stage(self.tx_sender.client.clone(), event_data.clone())
                .await?;
            let stage = stage_result.trim_matches('"').to_string();
            println!("Current Stage: {:?}", stage);

            if stage == "RevealMiners" {
                match self.reveal(event_data.clone()).await {
                    Ok(_) => {
                        return Ok(true);
                    }
                    Err(e) => {
                        println!("Failed to reveal by miner: {}", e);
                        return Err(e);
                    }
                }
            } else if stage == "CommitValidators" || stage == "RevealValidators" || stage == "Ended" {
                println!("RevealMiner stage has ended");
                return Ok(false);
            } else {
                println!("Waiting for RevealMiners stage...");
                sleep(Duration::from_secs(10)).await;
            }
        }

        println!("Failed to reach appropriate stages after multiple attempts.");
        Ok(false)
    }

    async fn commit(
        &self,
        event_data: EventData,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("Miner Commit");

        // Query to obtain hash answer to commit
        let query = QueryBuilder::new(ACCOUNT_TO_LISTEN.to_string())
            .with_method_name("hash_miner_answer")
            .with_args(serde_json::json!({
                "miner": self.account_id.to_string(),
                "request_id": event_data.request_id,
                "answer": true,
                "message": "It's the best option",
            }))
            .build();

        let query_sender = QuerySender::new(self.tx_sender.client.clone());
        let query_result = query_sender.send_query(query).await?;
        let answer_hash = query_result.trim_matches('"');

        // Transaction to send the commit
        let (nonce, block_hash) = self.nonce_manager.get_nonce_and_tx_hash().await?;

        let mut tx_builder = self.tx_builder.lock().await;

        let (tx, _) = tx_builder
            .with_method_name("commit_by_miner")
            .with_args(serde_json::json!({
                "request_id": event_data.request_id,
                "answer": answer_hash,
            }))
            .build(nonce, block_hash);

        let signer = &tx_builder.signer;

        let request = methods::send_tx::RpcSendTransactionRequest {
            signed_transaction: tx.sign(signer),
            wait_until: TxExecutionStatus::Final,
        };

        let tx_response = self.tx_sender.send_transaction(request).await?;
        let log_tx = extract_logs(&tx_response);

        println!("COMMIT_MINER_LOG: {:?}", log_tx);

        Ok(())
    }

    async fn reveal(
        &self,
        event_data: EventData,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("Reveal by miner");

        // Transaction to send the values to reveal
        let (nonce, block_hash) = self.nonce_manager.get_nonce_and_tx_hash().await?;

        let mut tx_builder = self.tx_builder.lock().await;

        let (tx, _) = tx_builder
            .with_method_name("reveal_by_miner")
            .with_args(serde_json::json!({
                "request_id": event_data.request_id,
                "answer": true,
                "message" : "It's the best option",
            }))
            .build(nonce, block_hash);

        let signer = &tx_builder.signer;

        let request = methods::send_tx::RpcSendTransactionRequest {
            signed_transaction: tx.sign(signer),
            wait_until: TxExecutionStatus::Final,
        };

        let tx_response = self.tx_sender.send_transaction(request).await?;
        let log_tx = extract_logs(&tx_response);
        println!("REVEAL_MINER_LOG: {:?}", log_tx);

        Ok(())
    }

}