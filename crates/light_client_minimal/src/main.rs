use std::env;

use light_client_minimal::{net::rpc::RpcClient, store::file::FileStore, sync::sync_chain};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = env::var("ZCASH_RPC_URL").expect("ZCASH_RPC_URL must be set");
    let client = RpcClient::new(&url)?;

    let start_height: u32 = match env::var("START_HEIGHT") {
        Ok(s) => s.parse().expect("START_HEIGHT must be a valid u32"),
        Err(_) => 3_000_000,
    };

    let store = FileStore::new("./data/headers.jsonl")?;
    sync_chain(&client, &store, start_height).await?;

    Ok(())
}
