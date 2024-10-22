use anyhow::Result;
use near_sdk::serde_json::json;
use omnibox::OmniInfo;


#[tokio::main]
async fn main() -> Result<()> {
    //Initialize the OmniInfo
    let omni = OmniInfo::new().await?;

    println!("Calling contract...");
    // Set the greeting
    let set_result = omni.call_contract(
            "set_greeting",
            Some(json!({"greeting": "Hello from Hassel"})),
        )
        .await?;

    match set_result {
        Some(value) => println!("Set greeting result: {:?}", value),
        None => println!("Greeting set successfully (no return value)"),
    }

    println!("Getting greeting...");
    // Get the greeting
    let get_result = omni.view_contract("get_greeting", None).await?;
    let greeting = get_result.as_str().unwrap_or("Failed to get greeting");
    println!("Greeting: {}", greeting);

    Ok(())
}