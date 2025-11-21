use std::env;

use light_client_minimal::{net::rpc::RpcClient, store::file::FileStore, sync::sync_chain};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"))
        .add_directive("stwo=warn".parse().unwrap())
        .add_directive("stwo_prover=warn".parse().unwrap())
        .add_directive("stwo_cairo_prover=warn".parse().unwrap())
        .add_directive("run=warn".parse().unwrap());

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

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
