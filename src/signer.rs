use async_trait::async_trait;
use near_sdk::AccountId;
use std::sync::Arc;
//use tokio::sync::Mutex;

use crate::{NonceManager, TransactionProcessor};
//use crate::tx_builder::TxBuilder;
//use crate::tx_sender::TxSender;
use crate::EventData;
pub struct Signer {
    pub account: AccountId,
    nonce_manager: Arc<NonceManager>,
    //tx_builder: Arc<Mutex<TxBuilder>>,
    //tx_sender: Arc<TxSender>,
}

impl Signer {
    pub fn new(account: AccountId, nonce_manager: Arc<NonceManager>,// tx_builder: Arc<Mutex<TxBuilder>>,
    //    tx_sender: Arc<TxSender>,
    ) -> Self {
        Self {
            account,
            nonce_manager,
            //tx_builder,
            //tx_sender,
        }
    }
}

#[async_trait]
impl TransactionProcessor for Signer {
    async fn process_transaction(
        &self,
        event_data: EventData,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        println!("Processing transaction: {:?}", event_data);

        Ok(true)
    }

    async fn respond(
        &self,
        event_data: EventData,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>{
        println!("Responding to event: {:?}", event_data);
        Ok(())
    }
}
