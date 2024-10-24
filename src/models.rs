use anyhow::Result;
use near_sdk::{
    serde_json::{json, Value},
    NearToken,
};
use near_workspaces::{network::Sandbox, Account, Contract, Worker};
use std::path::Path;

const DEFAULT_WASM_PATH: &str = env!("CARGO_MANIFEST_DIR");
const TEN_NEAR: NearToken = NearToken::from_near(10);

#[derive(Debug)]
pub struct OmniInfo {
    pub worker: Worker<Sandbox>,
    pub contract: Contract,
    pub account: Account,
}

impl OmniInfo {
    pub async fn new() -> Result<Self> {
        let worker = near_workspaces::sandbox().await?;

        let wasm_path = Path::new(DEFAULT_WASM_PATH)
            .join("src")
            .join("contract.wasm");

        if !wasm_path.exists() {
            return Err(anyhow::anyhow!("WASM file not found at: {:?}", wasm_path));
        }

        let wasm_bytes = std::fs::read(&wasm_path)?;

        let contract_account = worker
            .root_account()?
            .create_subaccount("contractmpc")
            .initial_balance(TEN_NEAR)
            .transact()
            .await?
            .into_result()?;

        // Deploy the contract to the specific account
        let contract = contract_account.deploy(&wasm_bytes).await?.into_result()?;

        //Create an account to call the contract
        let account = worker
            .root_account()?
            .create_subaccount("ownermpc")
            .initial_balance(TEN_NEAR)
            .transact()
            .await?
            .into_result()?;

        println!("Account: {:?}", account.id());
        println!("Contract ID:  {:?}", contract_account.id());
        
        Ok(Self {
            worker,
            contract,
            account,
        })
    }

    pub async fn call_contract(&self, method: &str, args: Option<Value>) -> Result<Option<Value>> {
        let result = self
            .account
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
            .account
            .view(&self.contract.id(), method)
            .args_json(args.unwrap_or(json!({})))
            .await?;

        Ok(result.json()?)
    }
}

// async fn create_subaccount(
//     root: &near_workspaces::Account,
//     name: &str,
// ) -> Result<near_workspaces::Account, anyhow::Error> {
//     let subaccount = root
//         .create_subaccount(name)
//         .initial_balance(TEN_NEAR)
//         .transact()
//         .await?
//         .unwrap();

//     Ok(subaccount)
// }
