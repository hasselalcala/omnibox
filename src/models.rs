use crate::{block_streamer::start_polling, NonceManager, Signer, TransactionProcessor};
use anyhow::Result;
use near_crypto::InMemorySigner;
use near_jsonrpc_client::JsonRpcClient;
use near_sdk::{
    serde_json::{json, Value},
    NearToken,
};

use near_workspaces::{network::Sandbox, Account, Contract, Worker};
use std::path::Path;
use std::sync::Arc;
use tokio::task::JoinHandle;

const DEFAULT_WASM_PATH: &str = env!("CARGO_MANIFEST_DIR");
const TEN_NEAR: NearToken = NearToken::from_near(10);
const HUNDRED_NEAR: NearToken = NearToken::from_near(100);

#[derive(Debug)]
pub struct OmniInfo {
    pub worker: Worker<Sandbox>,
    pub contract: Contract,
    pub account: Account,
    pub last_block_processed: u64,
    polling_handle: JoinHandle<()>,
}

impl OmniInfo {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let worker = near_workspaces::sandbox().await?;

        let wasm_path = Path::new(DEFAULT_WASM_PATH)
            .join("src")
            .join("mpc_test_contract.wasm");

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

        //Create an account to call the contract
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

        let nonce_manager = Arc::new(NonceManager::new(rpc_client.clone(), Arc::new(signer)));
        let processor: Arc<dyn TransactionProcessor> = Arc::new(Signer::new(account.id().clone(), nonce_manager));

        //start_polling(&rpc_client, last_block_processed, processor).await?;


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

        Ok(result.json()?)
    }
}
