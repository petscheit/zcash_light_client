use core::fmt;

use crate::difficulty::target::{Target, cmp_target, target_from_nbits};

/// Errors that can occur during difficulty verification.
#[derive(Debug)]
pub enum DiffError {
    /// `ToTarget(nBits)` returned zero (invalid compact encoding).
    InvalidTarget,
    /// Target derived from `nBits` is above the PoW limit.
    TargetAbovePowLimit,
    /// SHA256d(header) is greater than the target.
    HashAboveTarget,
    /// Not enough prior headers are available for contextual difficulty.
    InsufficientContext,
    /// Header height does not immediately follow the context tip height.
    HeightMismatch { expected: u32, found: u32 },
    /// `nBits` does not match the contextual difficulty adjustment.
    BitsMismatch { expected: u32, found: u32 },
}

impl fmt::Display for DiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffError::InvalidTarget => f.write_str("nBits encodes an invalid target"),
            DiffError::TargetAbovePowLimit => f.write_str("target exceeds PoW limit"),
            DiffError::HashAboveTarget => f.write_str("block hash is above target"),
            DiffError::InsufficientContext => {
                f.write_str("insufficient context for contextual difficulty")
            }
            DiffError::HeightMismatch { expected, found } => write!(
                f,
                "header height {found} does not follow context tip height {expected}"
            ),
            DiffError::BitsMismatch { expected, found } => write!(
                f,
                "nBits {found:#x} does not match contextual difficulty {expected:#x}"
            ),
        }
    }
}

impl std::error::Error for DiffError {}

/// PoWLimit(mainnet) = 2^243 − 1, encoded as a 256-bit little-endian integer.
pub(crate) const POW_LIMIT_LE: Target = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x07, 0x00,
];

/// Verifies the difficulty filter `Hash(header) <= ToTarget(nBits)`.
///
/// `header_hash` is the 32-byte SHA256d hash of the full serialized header, in the
/// same byte order as returned by `BlockHeader::hash().0` / RPC (little-endian for
/// consensus purposes). `n_bits` is the compact difficulty encoding taken from the
/// header.
pub fn verify_difficulty_filter(header_hash: &[u8; 32], n_bits: u32) -> Result<(), DiffError> {
    let hash_le: Target = *header_hash;
    println!("nBits: {:?}", hex::encode(n_bits.to_be_bytes()));
    let target_le = target_from_nbits(n_bits);
    println!("target_le: {:?}", hex::encode(target_le));

    if target_le == [0u8; 32] {
        return Err(DiffError::InvalidTarget);
    }

    if cmp_target(&target_le, &POW_LIMIT_LE) == core::cmp::Ordering::Greater {
        return Err(DiffError::TargetAbovePowLimit);
    }

    if cmp_target(&hash_le, &target_le) == core::cmp::Ordering::Greater {
        return Err(DiffError::HashAboveTarget);
    }

    Ok(())
}

/// Backwards-compatible alias.
pub fn verify_difficulty(header_hash: &[u8; 32], n_bits: u32) -> Result<(), DiffError> {
    verify_difficulty_filter(header_hash, n_bits)
}
