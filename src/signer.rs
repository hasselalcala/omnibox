use async_trait::async_trait;
use hex;
use near_jsonrpc_client::methods;
use near_primitives::transaction::Transaction;
use near_primitives::views::{FinalExecutionOutcomeViewEnum, FinalExecutionStatus, TxExecutionStatus};
use near_sdk::AccountId;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::tx_builder::TxBuilder;
use crate::tx_sender::TxSender;
use crate::EventData;
use crate::{NonceManager, TransactionProcessor};


pub struct Signer {
    pub account: AccountId,
    nonce_manager: Arc<NonceManager>,
    tx_builder: Arc<Mutex<TxBuilder>>,
    tx_sender: Arc<TxSender>,
}

impl Signer {
    pub fn new(
        account: AccountId,
        nonce_manager: Arc<NonceManager>,
        tx_builder: Arc<Mutex<TxBuilder>>,
        tx_sender: Arc<TxSender>,
    ) -> Self {
        Self {
            account,
            nonce_manager,
            tx_builder,
            tx_sender,
        }
    }
}

#[async_trait]
impl TransactionProcessor for Signer {
    async fn process_transaction(
        &self,
        event_data: EventData,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        println!("Processing transaction...");
        self.respond(event_data).await?;
        Ok(true)
    }

    async fn respond(
        &self,
        event_data: EventData,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("\n📤 Starting respond for event:");
        println!("   Yield ID: {}", event_data.yield_id.as_ref().unwrap());
    
        let (nonce, block_hash) = self.nonce_manager.get_nonce_and_tx_hash().await?;
        let mut tx_builder = self.tx_builder.lock().await;
    
        let yield_id = event_data.yield_id.as_ref().ok_or("yield_id is required")?;
        let bytes = hex::decode(yield_id.trim()).map_err(|e| {
            anyhow::anyhow!("Error to decode yield_id as hex: {}", e)
        })?;
    
        if bytes.len() != 32 {
            return Err(anyhow::anyhow!(
                "yield_id must be a 32 bytes, have {}",
                bytes.len()
            ).into());
        }
    
        let message = event_data.prompt.ok_or("prompt is required")?;
        let signer = near_crypto::Signer::from(tx_builder.signer.clone());
        let signature = signer.sign(message.as_bytes());
    
        let args = serde_json::json!({
            "yield_id": bytes.as_slice(),  
            "response": signature.to_string()
        });
    
        println!("   Args: {:?}", args);
    
        let (tx, _) = tx_builder
            .with_method_name("respond")
            .with_args(args)
            .build(nonce, block_hash);
    
        let signer = near_crypto::Signer::from(tx_builder.signer.clone());
        let signed_tx = Transaction::V0(tx).sign(&signer);
    
        let request = methods::send_tx::RpcSendTransactionRequest {
            signed_transaction: signed_tx,
            wait_until: TxExecutionStatus::Final,
        };
    
        println!("\n🚀 Sending transaction RESPOND...");
    
        match self.tx_sender.send_transaction(request).await {
            Ok(tx_response) => {
                println!("✅ Transaction RESPOND sent successfully");
                println!("   Tx response RESPOND: {:?}", tx_response);
                if let Some(outcome) = &tx_response.final_execution_outcome {
                    if let FinalExecutionOutcomeViewEnum::FinalExecutionOutcome(outcome) = outcome {
                        if let FinalExecutionStatus::Failure(failure) = &outcome.status {
                            println!("❌ Transaction fails: {:?}", failure);
                            return Err(anyhow::anyhow!("Transaction fails: {:?}", failure).into());
                        }
                    }
                }
                Ok(())
            }
            Err(e) => {
                println!("❌ Error sending transaction RESPOND: {:?}", e);
                Err(anyhow::anyhow!("Error sending transaction RESPOND: {}", e).into())
            }
        }
    }
}

