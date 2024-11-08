use anyhow::Result;
use near_crypto::InMemorySigner;
use near_jsonrpc_client::JsonRpcClient;
use near_sdk::{
    serde_json::{json, Value},
    NearToken,
};

use near_workspaces::{network::Sandbox, Account, Contract, Worker};
use std::{path::Path, sync::Arc};
use tokio::{task::JoinHandle, sync::Mutex};
use near_jsonrpc_client::methods;
use near_primitives::views::TxExecutionStatus;
use near_primitives::transaction::Transaction;
use crate::{EventData, block_streamer::extract_logs};

use crate::{TxBuilder, TxSender};
use crate::{block_streamer::start_polling, NonceManager, Signer, TransactionProcessor, constants::*};


const STANDARD: &str = "mpc-1.0.0";
const EVENT_RESPOND: &str = "respond";
const STATUS_COMPLETED: &str = "in progress";

const DEFAULT_WASM_PATH: &str = env!("CARGO_MANIFEST_DIR");
const TEN_NEAR: NearToken = NearToken::from_near(10);
const HUNDRED_NEAR: NearToken = NearToken::from_near(100);

#[derive(Debug)]
pub struct OmniInfo {
    pub worker: Worker<Sandbox>,
    pub contract: Contract,
    pub account: Account,
    pub last_block_processed: u64,
    pub polling_handle: JoinHandle<()>,
    pub nonce_manager: Arc<NonceManager>,
    pub tx_builder: Arc<Mutex<TxBuilder>>,
    pub tx_sender: Arc<TxSender>,
}

impl OmniInfo {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let worker = near_workspaces::sandbox().await?;

        let wasm_path = Path::new(DEFAULT_WASM_PATH)
            .join("src")
            .join("mpc_contract.wasm");

        if !wasm_path.exists() {
            return Err(anyhow::anyhow!("WASM file not found at: {:?}", wasm_path).into());
        }

        let wasm_bytes = std::fs::read(&wasm_path)?;

        let contract_account = worker
            .root_account()?
            .create_subaccount("contractmpc")
            .initial_balance(HUNDRED_NEAR)
            .transact()
            .await?
            .into_result()?;

        // Deploy the contract to the specific account
        let contract = contract_account.deploy(&wasm_bytes).await?.into_result()?;

        //Create an account to call the contract (REMOVE THIS)
        let account = worker
            .root_account()?
            .create_subaccount("accountmpc")
            .initial_balance(HUNDRED_NEAR)
            .transact()
            .await?
            .into_result()?;

        println!("Account: {:?}", account.id());
        println!("Contract ID:  {:?}", contract_account.id());

        // Prepare the RPC client
        let rpc_address = worker.rpc_addr();
        let rpc_client = Arc::new(JsonRpcClient::connect(rpc_address));

        let last_block_processed = worker.view_block().await?.height();

        let signer = InMemorySigner {
            account_id: account.id().clone(),
            public_key: account.secret_key().public_key().to_string().parse()?,
            secret_key: account.secret_key().to_string().parse()?,
        };

        let nonce_manager = Arc::new(NonceManager::new(rpc_client.clone(), Arc::new(signer.clone())));
        let tx_builder = Arc::new(Mutex::new(TxBuilder::new(signer.clone(), worker.clone())));
        let tx_sender = Arc::new(TxSender::new(rpc_client.clone(), DEFAULT_TIMEOUT));

        //let processor: Arc<dyn TransactionProcessor> =
          //  Arc::new(Signer::new(account.id().clone(), nonce_manager.clone(), tx_builder.clone(), tx_sender.clone()));

        let processor = Arc::new(Signer::new(account.id().clone(), nonce_manager.clone()));

        let polling_handle = tokio::spawn({
            let rpc_client = rpc_client.clone();
            async move {
                if let Err(e) = start_polling(&rpc_client, last_block_processed, processor).await {
                    eprintln!("Polling error: {}", e);
                }
            }
        });

        Ok(Self {
            worker,
            contract,
            account,
            last_block_processed,
            polling_handle,
            nonce_manager,
            tx_builder,
            tx_sender,
        })
    }

    pub async fn call_contract(&self, method: &str, args: Option<Value>) -> Result<Option<Value>> {
        let result = self
            .account
            .call(&self.contract.id(), method)
            .deposit(TEN_NEAR)
            .max_gas()
            .args_json(args.unwrap_or(json!({})))
            .transact()
            .await?;

        println!("RESULT CALL CONTRACT: {:?}", result.outcome());

        if result.is_success() {
            Ok(result.json().ok())
        } else {
            Err(anyhow::anyhow!(
                "Contract call failed: {:?}",
                result.outcome()
            ))
        }
    }

    pub async fn view_contract(&self, method: &str, args: Option<Value>) -> Result<Value> {
        let result = self
            .account
            .view(&self.contract.id(), method)
            .args_json(args.unwrap_or(json!({})))
            .await?;

        println!(
            "Pending requests: standard: {}, event: {}, status: {}",
            STANDARD, EVENT_RESPOND, STATUS_COMPLETED
        );
        Ok(result.json()?)
    }

    // pub async fn sign(
    //     &self,
    //     prompt: String,
    //     //event_data: EventData,
    // ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>{

    //     println!("SIGNING: {:?}", prompt);

    //     // Transaction to send the sign
    //     let (nonce, block_hash) = self.nonce_manager.get_nonce_and_tx_hash().await?;

    //     let mut tx_builder = self.tx_builder.lock().await;

    //     let (tx, _) = tx_builder
    //         .with_method_name("sign")
    //         .with_args(serde_json::json!({
    //             "prompt": prompt  
    //         }))
    //         .build(nonce, block_hash);

    //     let signer = near_crypto::Signer::from(tx_builder.signer.clone());
    //     let signed_tx = Transaction::V0(tx).sign(&signer);

    //     let request = methods::send_tx::RpcSendTransactionRequest {
    //         signed_transaction: signed_tx,
    //         wait_until: TxExecutionStatus::Final,
    //     };

    //     let tx_response = self.tx_sender.send_transaction(request).await?;
    //     let log_tx = extract_logs(&tx_response);

    //     println!("SIGN_LOG: {:?}", log_tx);

    //     Ok(())
    // }



pub async fn sign(
    &self,
    prompt: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("SIGNING: {:?}", prompt);
    
    // Aumentar el número de intentos y añadir retry logic
    let max_attempts = 3;
    let mut attempt = 0;
    
    while attempt < max_attempts {
        let (nonce, block_hash) = self.nonce_manager.get_nonce_and_tx_hash().await?;
        let mut tx_builder = self.tx_builder.lock().await;

        let (tx, _) = tx_builder
            .with_method_name("sign")
            .with_args(serde_json::json!({
                "prompt": prompt.clone()
            }))
            .build(nonce, block_hash);

        let signer = near_crypto::Signer::from(tx_builder.signer.clone());
        let signed_tx = Transaction::V0(tx).sign(&signer);

        let request = methods::send_tx::RpcSendTransactionRequest {
            signed_transaction: signed_tx,
            wait_until: TxExecutionStatus::Final,
        };

        match self.tx_sender.send_transaction(request).await {
            Ok(tx_response) => {
                let log_tx = extract_logs(&tx_response);
                println!("SIGN_LOG: {:?}", log_tx);
                return Ok(());
            }
            Err(e) => {
                attempt += 1;
                if attempt == max_attempts {
                    return Err(e.into());
                }
                println!("Attempt {} failed, retrying...", attempt);
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        }
    }
    
    Err(anyhow::anyhow!("Failed after {} attempts", max_attempts).into())
}


}
