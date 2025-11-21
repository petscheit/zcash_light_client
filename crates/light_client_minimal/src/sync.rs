use core::fmt;

use crate::net::rpc::{RpcClient, RpcError};
use crate::store::Store;
use tracing::{debug, info};
use zcash_crypto::{DifficultyContext, verify_pow_in_cairo, verify_pow_with_context};
use zcash_primitives::block::BlockHeader;

/// Errors that can occur when verifying a header fetched via RPC.
#[derive(Debug)]
pub enum VerifyHeaderError {
    Rpc(RpcError),
    Pow(VerifyPowError),
    /// Not enough prior headers are available to build the difficulty context.
    InsufficientContext {
        height: u32,
    },
}

impl fmt::Display for VerifyHeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerifyHeaderError::Rpc(e) => write!(f, "RPC error: {e}"),
            VerifyHeaderError::Pow(e) => write!(f, "PoW verification error: {e:?}"),
            VerifyHeaderError::InsufficientContext { height } => write!(
                f,
                "insufficient context to verify difficulty at height {height}"
            ),
        }
    }
}

impl std::error::Error for VerifyHeaderError {}

/// Wrapper to avoid exposing zcash_crypto's error types directly.
#[derive(Debug)]
pub struct VerifyPowError(pub Box<dyn std::error::Error + Send + Sync>);

impl From<zcash_crypto::PowError> for VerifyPowError {
    fn from(e: zcash_crypto::PowError) -> Self {
        VerifyPowError(Box::new(e))
    }
}

/// Fetches the header at `height`, builds minimal difficulty context, and verifies.
pub async fn verify_header(rpc: &RpcClient, height: u32) -> Result<(), VerifyHeaderError> {
    const CONTEXT_BLOCKS: u32 = 28;
    if height < CONTEXT_BLOCKS {
        return Err(VerifyHeaderError::InsufficientContext { height });
    }

    let header = rpc
        .get_block_header_by_height(height)
        .await
        .map_err(VerifyHeaderError::Rpc)?;

    let start = height - CONTEXT_BLOCKS;
    let mut ctx = DifficultyContext::new(height - 1);

    for h in start..height {
        let prev_header = rpc
            .get_block_header_by_height(h)
            .await
            .map_err(VerifyHeaderError::Rpc)?;
        ctx.push_header(h, prev_header.time, prev_header.bits);
    }

    verify_pow_with_context(&header, height, &mut ctx)
        .map_err(|e| VerifyHeaderError::Pow(VerifyPowError::from(e)))
}

fn header_to_hex(header: &BlockHeader) -> Result<String, VerifyHeaderError> {
    let mut buf = Vec::new();
    // BlockHeader::write is expected to be available in zcash_primitives.
    header
        .write(&mut buf)
        .map_err(|e| VerifyHeaderError::Rpc(RpcError::Client(format!("serialize header: {e}"))))?;
    Ok(hex::encode(buf))
}

fn header_from_hex(s: &str) -> Result<BlockHeader, VerifyHeaderError> {
    let bytes = hex::decode(s)
        .map_err(|e| VerifyHeaderError::Rpc(RpcError::Client(format!("hex decode: {e}"))))?;
    BlockHeader::read(&bytes[..])
        .map_err(|e| VerifyHeaderError::Rpc(RpcError::Client(format!("decode header: {e}"))))
}

async fn build_ctx_from_store_or_rpc<S: Store>(
    rpc: &RpcClient,
    store: &S,
    effective_start: u32,
) -> Result<DifficultyContext, VerifyHeaderError> {
    const CONTEXT_BLOCKS: usize = 28;
    let mut ctx = DifficultyContext::new(effective_start - 1);

    // Try to load as much context as possible from the store.
    let stored = store
        .last_n(CONTEXT_BLOCKS)
        .map_err(|e| VerifyHeaderError::Rpc(RpcError::Client(format!("store read: {e}"))))?;
    if !stored.is_empty() {
        // Ensure ascending order by height.
        let mut stored_sorted = stored.clone();
        stored_sorted.sort_by_key(|(h, _)| *h);
        let m = stored_sorted.len();
        // If we have insufficient context, fetch missing older headers via RPC first.
        if m < CONTEXT_BLOCKS {
            let need = CONTEXT_BLOCKS - m;
            let earliest = stored_sorted.first().map(|(h, _)| *h).unwrap();
            let start = earliest.saturating_sub(need as u32);
            for h in start..earliest {
                let hdr = rpc
                    .get_block_header_by_height(h)
                    .await
                    .map_err(VerifyHeaderError::Rpc)?;
                ctx.push_header(h, hdr.time, hdr.bits);
            }
        }
        // Now append the stored headers in ascending order.
        for (h, hex) in &stored_sorted {
            let hdr = header_from_hex(hex)?;
            ctx.push_header(*h, hdr.time, hdr.bits);
        }
        return Ok(ctx);
    }

    // No stored context available; build entirely from RPC.
    let context_start = effective_start - CONTEXT_BLOCKS as u32;
    for h in context_start..effective_start {
        let header = rpc
            .get_block_header_by_height(h)
            .await
            .map_err(VerifyHeaderError::Rpc)?;
        ctx.push_header(h, header.time, header.bits);
    }
    Ok(ctx)
}

/// Continuously verifies headers starting at `start_height`, persisting each verified header.
pub async fn sync_chain<S: Store>(
    rpc: &RpcClient,
    store: &S,
    start_height: u32,
    prove: bool,
) -> Result<(), VerifyHeaderError> {
    const CONTEXT_BLOCKS: u32 = 28;
    if start_height < CONTEXT_BLOCKS {
        return Err(VerifyHeaderError::InsufficientContext {
            height: start_height,
        });
    }

    // Determine effective start height from persistence, if available.
    let effective_start = match store
        .tip()
        .map_err(|e| VerifyHeaderError::Rpc(RpcError::Client(format!("store tip: {e}"))))?
    {
        Some(tip) => match tip.checked_add(1) {
            Some(h) => h,
            None => return Ok(()),
        },
        None => start_height,
    };

    // Build initial context using persisted headers where possible, filling gaps via RPC.
    let mut ctx = build_ctx_from_store_or_rpc(rpc, store, effective_start).await?;

    let mut height = effective_start;

    loop {
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        info!("Block {height}");
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        let header = rpc
            .get_block_header_by_height(height)
            .await
            .map_err(VerifyHeaderError::Rpc)?;

        verify_pow_with_context(&header, height, &mut ctx)
            .map_err(|e| VerifyHeaderError::Pow(VerifyPowError::from(e)))?;
        debug!("Rust PoW verification passed");

        verify_pow_in_cairo(&header, height, prove)
            .map_err(|e| VerifyHeaderError::Pow(VerifyPowError::from(e)))?;
        debug!("Cairo PoW verification passed");

        let header_hex = header_to_hex(&header)?;
        store
            .put(height, &header_hex)
            .map_err(|e| VerifyHeaderError::Rpc(RpcError::Client(format!("store header: {e}"))))?;

        if prove {
            info!("✓ Block {height} verified, proven and stored");
        } else {
            info!("✓ Block {height} verified and stored");
        }

        height = match height.checked_add(1) {
            Some(next) => next,
            None => break,
        };
    }

    Ok(())
}
