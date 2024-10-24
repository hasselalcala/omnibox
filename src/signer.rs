use crate::EventData;
use crate::{NonceManager, TransactionProcessor};
use async_trait::async_trait;
use near_sdk::AccountId;
use std::sync::Arc;

pub struct Signer {
    pub account: AccountId,
    nonce_manager: Arc<NonceManager>,
}

impl Signer {
    pub fn new(account: AccountId, nonce_manager: Arc<NonceManager>) -> Self {
        Self {
            account,
            nonce_manager,
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
}
