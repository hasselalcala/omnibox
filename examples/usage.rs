use anyhow::Result;
use omnibox::OmniInfo;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let omni = OmniInfo::new().await?;

    //tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    println!("\n🚀 Creating new sign request...");
    match omni.sign("Message to sign".to_string()).await {
        Ok(_) => println!("✅ Sign request completed successfully"),
        Err(e) => println!("❌ Sign request failed: {:?}", e),
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;

    Ok(())
}
