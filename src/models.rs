use anyhow::Result;
use near_sdk::{serde_json::{json, Value}, NearToken};
use near_workspaces::{network::Sandbox, Account, Contract, Worker};
use std::path::Path;

const DEFAULT_WASM_PATH: &str = env!("CARGO_MANIFEST_DIR");
const TEN_NEAR: NearToken = NearToken::from_near(10);

#[derive(Debug)]
pub struct OmniInfo {
    pub worker: Worker<Sandbox>,
    pub contract: Contract,
    pub owner: Account,
}

impl OmniInfo {
    pub async fn new() -> Result<Self> {
        let worker = near_workspaces::sandbox().await?;
        
        let wasm_path = Path::new(DEFAULT_WASM_PATH).join("src").join("contract.wasm");
        if !wasm_path.exists() {
            return Err(anyhow::anyhow!("WASM file not found at: {:?}", wasm_path));
        }

        let wasm_bytes = std::fs::read(&wasm_path)?;

        let contract_account = worker.dev_create_account().await?;
        let contract = contract_account.deploy(&wasm_bytes).await?.into_result()?;

        //Create a define account for the contract and owner
        //let root = worker.root_account()?;
        //let owner = create_subaccount(&root, "contractmpc").await?;
        let owner = worker.dev_create_account().await?;
        
        println!("Owner: {:?}", owner.id());
        println!("Contract ID:  {:?}", contract_account.id());

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

async fn create_subaccount(
    root: &near_workspaces::Account,
    name: &str,
) -> Result<near_workspaces::Account, anyhow::Error> {
    let subaccount = root
        .create_subaccount(name)
        .initial_balance(TEN_NEAR)
        .transact()
        .await?
        .unwrap();

    Ok(subaccount)
}