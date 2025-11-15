use std::env;

use light_client_minimal::net::rpc::RpcClient;
use zcash_crypto::verify_pow;

/// Fixed set of interesting block heights to exercise PoW verification.
fn test_block_heights() -> Vec<u32> {
    vec![1_000_000, 1_000_001, 2_000_000, 415000]
}

/// Integration-style test that fetches headers over RPC and runs `verify_pow` on them.
///
/// This test is ignored by default because it requires a running `zcashd`-compatible node.
/// To use it:
/// - start a node with RPC enabled;
/// - set:
///   - `ZCASH_RPC_URL` (e.g. `http://127.0.0.1:8232`);
/// - run: `cargo test -p light_client_minimal rpc_verify_pow_blocks -- --ignored`.
#[tokio::test]
async fn rpc_verify_pow_blocks() -> Result<(), Box<dyn std::error::Error>> {
    let url = match env::var("ZCASH_RPC_URL") {
        Ok(u) => u,
        Err(_) => {
            eprintln!("ZCASH_RPC_URL not set; skipping RPC pow test");
            return Ok(());
        }
    };

    let client = RpcClient::new(&url)?;
    let heights = test_block_heights();

    for h in heights {
        eprintln!("rpc_verify_pow_blocks: checking height {h}");
        let header = client.get_block_header_by_height(h).await?;
        verify_pow(&header).unwrap();
    }

    Ok(())
}

/// Integration-style test that fetches headers over RPC and runs `verify_header` on them,
/// exercising Equihash, the difficulty filter, and contextual difficulty.
#[tokio::test]
async fn rpc_verify_header_blocks() -> Result<(), Box<dyn std::error::Error>> {
    use light_client_minimal::sync::verify_header;
    let url = match env::var("ZCASH_RPC_URL") {
        Ok(u) => u,
        Err(_) => {
            eprintln!("ZCASH_RPC_URL not set; skipping RPC verify_header test");
            return Ok(());
        }
    };

    let client = RpcClient::new(&url)?;
    let heights = vec![3_000_000];

    for h in heights {
        eprintln!("rpc_verify_header_blocks: checking height {h}");
        verify_header(&client, h).await.unwrap();
    }

    Ok(())
}
