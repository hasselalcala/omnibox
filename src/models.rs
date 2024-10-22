use anyhow::Result;
use near_sdk::serde_json::{json, Value};
use near_workspaces::{network::Sandbox, Account, Contract, Worker};
use std::path::Path;

const DEFAULT_WASM_PATH : &str = "src/contract.wasm";
#[derive(Debug)]
pub struct OmniInfo {
    pub worker: Worker<Sandbox>,
    pub contract: Contract,
    pub owner: Account,
}

impl OmniInfo {
    pub async fn new() -> Result<Self> {
        let worker = near_workspaces::sandbox().await?;
        
        let wasm_path = Path::new(DEFAULT_WASM_PATH);
        if !wasm_path.exists() {
            return Err(anyhow::anyhow!("WASM file not found at: {:?}", wasm_path.canonicalize()?));
        }

        let wasm_bytes = std::fs::read(wasm_path)?;
        let contract = worker.dev_deploy(&wasm_bytes).await?;
        let owner = worker.dev_create_account().await?;

        Ok(Self {
            worker,
            contract,
            owner,
        })
    }

    pub async fn call_contract(&self, method: &str, args: Option<Value>) -> Result<Option<Value>> {
        let result = self
            .owner
            .call(&self.contract.id(), method)
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
            .owner
            .view(&self.contract.id(), method)
            .args_json(args.unwrap_or(json!({})))
            .await?;

        Ok(result.json()?)
    }
}