use anyhow::Result;
use near_sdk::serde_json::{json, Value};
use near_workspaces::{network::Sandbox, Contract, Worker};

const WASM_PATH: &str = "contract/contract.wasm";

#[derive(Debug)]
pub struct OmniInfo {
    pub worker: Worker<Sandbox>,
    pub contract: Contract,
}

impl OmniInfo {
    pub async fn new() -> Result<Self> {
        let worker = near_workspaces::sandbox().await?;
        let wasm = std::fs::read(WASM_PATH)?;
        let contract = worker.dev_deploy(&wasm).await?;


        Ok(Self {
            worker,
            contract,
        })
    }

    pub async fn call_contract(&self, method: &str, args: Option<Value>) -> Result<Option<Value>> {
        let owner = self.worker.dev_create_account().await?;

        let result = owner
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
        let owner = self.worker.dev_create_account().await?;

        let result = owner
            .view(&self.contract.id(), method)
            .args_json(args.unwrap_or(json!({})))
            .await?;

        Ok(result.json()?)
    }
}
