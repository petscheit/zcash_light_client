//! Minimal Equihash and difficulty verification for Zcash-style block headers.
//!
//! This crate exposes:
//! - Equihash (n=200,k=9) verification: `verify_equihash_solution`, `verify_equihash_solution_with_params`
//! - Difficulty filter: `verify_difficulty` (alias for `verify_difficulty_filter`)
//! - Contextual difficulty: `difficulty::context::{DifficultyContext, expected_nbits, verify_difficulty}`
//! - Combined helpers: `verify_pow`, `verify_pow_with_context`
pub mod difficulty;
pub mod equihash;

use core::fmt;
use cairo_runner::run_stwo;
use cairo_runner::types::InputData;
use zcash_primitives::block::BlockHeader;

pub use difficulty::context::DifficultyContext;
pub use difficulty::filter::{DiffError, verify_difficulty, verify_difficulty_filter};
pub use equihash::{Error, Kind, verify_equihash_solution, verify_equihash_solution_with_params};

/// Combined Equihash + difficulty verification error.
#[derive(Debug)]
pub enum PowError {
    Equihash(Error),
    Difficulty(DiffError),
    ContextDifficulty(DiffError),
}

impl fmt::Display for PowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PowError::Equihash(e) => write!(f, "Equihash error: {e}"),
            PowError::Difficulty(e) => write!(f, "Difficulty filter error: {e}"),
            PowError::ContextDifficulty(e) => write!(f, "Contextual difficulty error: {e}"),
        }
    }
}

impl std::error::Error for PowError {}

/// Verifies both the Equihash solution and difficulty filter for a parsed `BlockHeader`.
pub fn verify_pow(header: &BlockHeader) -> Result<(), PowError> {
    // Reconstruct the Equihash "powheader": header bytes up to and including the nonce.
    let mut powheader = Vec::with_capacity(140);
    powheader.extend_from_slice(&header.version.to_le_bytes());
    powheader.extend_from_slice(&header.prev_block.0);
    powheader.extend_from_slice(&header.merkle_root);
    powheader.extend_from_slice(&header.final_sapling_root);
    powheader.extend_from_slice(&header.time.to_le_bytes());
    powheader.extend_from_slice(&header.bits.to_le_bytes());
    powheader.extend_from_slice(&header.nonce);

    // 1. Equihash solution validity.
    equihash::verify_equihash_solution(&powheader, &header.solution).map_err(PowError::Equihash)?;

    // 2. Difficulty filter using the full header hash and nBits.
    let hash = header.hash();
    difficulty::filter::verify_difficulty(&hash.0, header.bits).map_err(PowError::Difficulty)
}

pub fn verify_pow_in_cairo(header: &BlockHeader) -> Result<(), PowError> {
    let mut powheader = Vec::with_capacity(140);
    powheader.extend_from_slice(&header.version.to_le_bytes());
    powheader.extend_from_slice(&header.prev_block.0);
    powheader.extend_from_slice(&header.merkle_root);
    powheader.extend_from_slice(&header.final_sapling_root);
    powheader.extend_from_slice(&header.time.to_le_bytes());
    powheader.extend_from_slice(&header.bits.to_le_bytes());
    powheader.extend_from_slice(&header.nonce);

    let header_bytes: Vec<u32> = powheader.chunks_exact(4).map(|chunk| u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])).collect();
    let solution_bytes = header
        .solution
        .chunks_exact(4)
        .map(|chunk| u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();

    let input = InputData {
        header_bytes,
        solution_bytes,
    };

    run_stwo("cairo/build/main.json", input, "info", "output", true, false).unwrap();

    Ok(())
}

/// Verifies Equihash, the difficulty filter, and contextual difficulty for a header.
///
/// The caller is responsible for maintaining `ctx` in chain order. On success,
/// this function appends the header to the context.
pub fn verify_pow_with_context(
    header: &BlockHeader,
    height: u32,
    ctx: &mut DifficultyContext,
) -> Result<(), PowError> {
    // Reconstruct the Equihash "powheader": header bytes up to and including the nonce.
    let mut powheader = Vec::with_capacity(140);
    powheader.extend_from_slice(&header.version.to_le_bytes());
    powheader.extend_from_slice(&header.prev_block.0);
    powheader.extend_from_slice(&header.merkle_root);
    powheader.extend_from_slice(&header.final_sapling_root);
    powheader.extend_from_slice(&header.time.to_le_bytes());
    powheader.extend_from_slice(&header.bits.to_le_bytes());
    powheader.extend_from_slice(&header.nonce);

    equihash::verify_equihash_solution(&powheader, &header.solution).map_err(PowError::Equihash)?;

    let hash = header.hash();
    difficulty::filter::verify_difficulty(&hash.0, header.bits).map_err(PowError::Difficulty)?;

    difficulty::context::verify_difficulty(ctx, height, header.bits)
        .map_err(PowError::ContextDifficulty)?;

    ctx.push_header(height, header.time, header.bits);
    Ok(())
}
