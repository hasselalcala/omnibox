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
    polling_handle: JoinHandle<()>,
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
        let processor: Arc<dyn TransactionProcessor> =
            Arc::new(Signer::new(account.id().clone(), nonce_manager));

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
            .contract
            .call( method)
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
}
