//! Equihash verifier implementation.
//!
//! High-level procedure:
//! 1) Decode the minimal solution into `2^k` indices (big-endian bit-packed).
//! 2) Initialize BLAKE2b with Zcash personalization ("ZcashPoW" || LE32(n) || LE32(k))
//!    and absorb the `powheader` (header bytes up to and including the nonce).
//! 3) Build a binary merge tree over the indices:
//!    - Require equal leading `collision_byte_length` bytes for each sibling pair.
//!    - Enforce lexicographic ordering of subtrees (binding condition).
//!    - Ensure index sets are disjoint.
//!    - Combine by XORing the remaining bytes (after trimming the collision prefix).
//! 4) At the root, the remaining bytes must be all zeros; otherwise the solution is invalid.
use blake2b_simd::{Hash as Blake2bHash, Params as Blake2bParams, State as Blake2bState};
use core::fmt;

/// Equihash parameters `(n, k)`.
///
/// - `n`: number of bits per leaf hash fragment.
/// - `k`: number of reduction rounds; a solution has `2^k` indices.
#[derive(Clone, Copy)]
struct Params {
    n: u32,
    k: u32,
}

impl Params {
    /// Construct validated parameters.
    fn new(n: u32, k: u32) -> Option<Self> {
        if (n % 8 == 0) && (k >= 3) && (k < n) && (n % (k + 1) == 0) {
            Some(Self { n, k })
        } else {
            None
        }
    }
    /// Number of indices represented per BLAKE2b digest output.
    fn indices_per_hash_output(&self) -> u32 {
        512 / self.n
    }
    /// Digest length for BLAKE2b personalization for these parameters.
    fn hash_output(&self) -> u8 {
        (self.indices_per_hash_output() * self.n / 8) as u8
    }
    /// Collision length in bits (required equal prefix per merge level).
    fn collision_bit_length(&self) -> usize {
        (self.n / (self.k + 1)) as usize
    }
    /// Collision length rounded up to whole bytes.
    fn collision_byte_length(&self) -> usize {
        (self.collision_bit_length() + 7) / 8
    }
}

/// Error wrapper indicating why verification failed.
#[derive(Debug)]
pub struct Error(pub Kind);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid solution: {}", self.0)
    }
}

/// Specific failure reasons during verification.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Kind {
    /// Invalid `(n,k)` parameters or solution length/encoding.
    InvalidParams,
    /// Leading collision bytes did not match for a pair of siblings.
    Collision,
    /// Left subtree did not lexicographically precede the right subtree.
    OutOfOrder,
    /// Duplicate index encountered across siblings.
    DuplicateIdxs,
    /// Final root bytes (after reductions) are not all zero.
    NonZeroRootHash,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::InvalidParams => f.write_str("invalid parameters"),
            Kind::Collision => f.write_str("invalid collision length between StepRows"),
            Kind::OutOfOrder => f.write_str("Index tree incorrectly ordered"),
            Kind::DuplicateIdxs => f.write_str("duplicate indices"),
            Kind::NonZeroRootHash => f.write_str("root hash of tree is non-zero"),
        }
    }
}

/// Initialize BLAKE2b with Zcash personalization and the desired digest length.
///
/// Personalization: "ZcashPoW" || LE32(n) || LE32(k).
fn initialise_state(n: u32, k: u32, digest_len: u8) -> Blake2bState {
    // personalization = "ZcashPoW" || LE32(n) || LE32(k)
    let mut personalization: [u8; 16] = *b"ZcashPoW\x00\x00\x00\x00\x00\x00\x00\x00";
    personalization[8..12].copy_from_slice(&n.to_le_bytes());
    personalization[12..16].copy_from_slice(&k.to_le_bytes());
    Blake2bParams::new()
        .hash_length(digest_len as usize)
        .personal(&personalization)
        .to_state()
}

/// Compute the `i`-th group BLAKE2b digest by hashing the 32-bit little-endian counter.
///
/// A digest contains several adjacent `n`-bit slices; leaf construction selects one slice.
fn generate_hash(base_state: &Blake2bState, i: u32) -> Blake2bHash {
    let mut state = base_state.clone();
    state.update(&i.to_le_bytes());
    state.finalize()
}

/// Expand a compact big-endian bitstring into fixed-width, optionally byte-padded chunks.
///
/// Used for both digest-slice expansion and minimal solution expansion to big-endian `u32`s.
fn expand_array(vin: &[u8], bit_len: usize, byte_pad: usize) -> Vec<u8> {
    assert!(bit_len >= 8);
    assert!((u32::BITS as usize) >= 7 + bit_len);

    let out_width = (bit_len + 7) / 8 + byte_pad;
    let out_len = 8 * out_width * vin.len() / bit_len;

    if out_len == vin.len() {
        return vin.to_vec();
    }
    let mut vout: Vec<u8> = vec![0; out_len];
    let bit_len_mask: u32 = (1 << bit_len) - 1;

    let mut acc_bits = 0usize;
    let mut acc_value: u32 = 0;
    let mut j = 0usize;

    for b in vin {
        acc_value = (acc_value << 8) | u32::from(*b);
        acc_bits += 8;
        if acc_bits >= bit_len {
            acc_bits -= bit_len;
            for x in byte_pad..out_width {
                vout[j + x] = ((acc_value >> (acc_bits + (8 * (out_width - x - 1))))
                    & ((bit_len_mask >> (8 * (out_width - x - 1))) & 0xFF))
                    as u8;
            }
            j += out_width;
        }
    }
    vout
}

/// Decode the minimal solution into a vector of big-endian `u32` indices.
///
/// Length check: `minimal.len() == (2^k * (c_bit_len+1)) / 8` where `c_bit_len = n/(k+1)`.
fn indices_from_minimal(p: Params, minimal: &[u8]) -> Option<Vec<u32>> {
    let c_bit_len = p.collision_bit_length();
    if minimal.len() != ((1 << p.k) * (c_bit_len + 1)) / 8 {
        return None;
    }
    let digit_bytes = ((c_bit_len + 1) + 7) / 8;
    let byte_pad = core::mem::size_of::<u32>() - digit_bytes;
    let expanded = expand_array(minimal, c_bit_len + 1, byte_pad);
    if expanded.len() % 4 != 0 {
        return None;
    }
    let mut ret = Vec::with_capacity(expanded.len() / 4);
    for chunk in expanded.chunks_exact(4) {
        ret.push(u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Some(ret)
}

/// Tree node holding the current reduced hash bytes and the ordered index list.
#[derive(Clone)]
struct Node {
    hash: Vec<u8>,
    indices: Vec<u32>,
}

impl Node {
    /// Construct a leaf:
    /// - Take the appropriate `n`-bit slice from the group digest.
    /// - Expand to bytes (big-endian) to form the leaf hash.
    fn new(p: &Params, state: &Blake2bState, i: u32) -> Self {
        let hash = generate_hash(state, i / p.indices_per_hash_output());
        let start = ((i % p.indices_per_hash_output()) * p.n / 8) as usize;
        let end = start + (p.n as usize) / 8;
        Node {
            hash: expand_array(&hash.as_bytes()[start..end], p.collision_bit_length(), 0),
            indices: vec![i],
        }
    }
    /// Combine siblings by XORing the post-collision bytes and concatenating indices
    /// with the lexicographically earlier subtree first.
    fn from_children(a: Node, b: Node, trim: usize) -> Self {
        let hash = a
            .hash
            .iter()
            .zip(b.hash.iter())
            .skip(trim)
            .map(|(x, y)| x ^ y)
            .collect();
        let indices = if a.indices_before(&b) {
            let mut v = a.indices;
            v.extend(b.indices.iter());
            v
        } else {
            let mut v = b.indices;
            v.extend(a.indices.iter());
            v
        };
        Node { hash, indices }
    }
    /// Order subtrees by their first index (binding condition).
    fn indices_before(&self, other: &Node) -> bool {
        self.indices[0] < other.indices[0]
    }
    /// Check that the first `len` bytes equal zero.
    fn is_zero(&self, len: usize) -> bool {
        self.hash.iter().take(len).all(|v| *v == 0)
    }
}

/// Check collision prefix equality (`len` bytes).
fn has_collision(a: &Node, b: &Node, len: usize) -> bool {
    a.hash
        .iter()
        .zip(b.hash.iter())
        .take(len)
        .all(|(x, y)| x == y)
}

/// Ensure index sets are disjoint.
fn distinct_indices(a: &Node, b: &Node) -> bool {
    for i in &a.indices {
        for j in &b.indices {
            if i == j {
                return false;
            }
        }
    }
    true
}

/// Validate sibling constraints: collision equality, ordering, and distinctness.
fn validate_subtrees(p: &Params, a: &Node, b: &Node) -> Result<(), Kind> {
    if !has_collision(a, b, p.collision_byte_length()) {
        Err(Kind::Collision)
    } else if b.indices_before(a) {
        Err(Kind::OutOfOrder)
    } else if !distinct_indices(a, b) {
        Err(Kind::DuplicateIdxs)
    } else {
        Ok(())
    }
}

/// Recursively build and validate the merge tree; returns the root node.
fn tree_validator(p: &Params, state: &Blake2bState, indices: &[u32]) -> Result<Node, Error> {
    if indices.len() > 1 {
        let end = indices.len();
        let mid = end / 2;
        let a = tree_validator(p, state, &indices[0..mid])?;
        let b: Node = tree_validator(p, state, &indices[mid..end])?;
        validate_subtrees(p, &a, &b).map_err(Error)?;
        Ok(Node::from_children(a, b, p.collision_byte_length()))
    } else {
        Ok(Node::new(p, state, indices[0]))
    }
}

/// Verify that `solution` encodes a valid Equihash solution for the provided `powheader`,
/// using the default Zcash parameters `(n=200, k=9)`.
pub fn verify_equihash_solution(powheader: &[u8], solution: &[u8]) -> Result<(), Error> {
    verify_equihash_solution_with_params(200, 9, powheader, solution)
}

/// Verify a solution for arbitrary valid `(n, k)` parameters.
///
/// Steps:
/// 1) Validate `(n,k)` and decode the minimal solution to an index array.
/// 2) Initialize BLAKE2b with personalization and absorb `powheader`.
/// 3) Recursively build and validate the Equihash merge tree over the indices.
/// 4) Require that the root’s remaining bytes are all zero.
///
/// Inputs:
/// - `powheader`: bytes bound into Equihash (typically the header up to and including the nonce).
/// - `solution`: minimal encoding for `(n,k)`; length `(2^k * (c_bit_len + 1))/8`
///   where `c_bit_len = n / (k + 1)`.
pub fn verify_equihash_solution_with_params(
    n: u32,
    k: u32,
    powheader: &[u8],
    solution: &[u8],
) -> Result<(), Error> {
    let p = Params::new(n, k).ok_or(Error(Kind::InvalidParams))?;
    let indices = indices_from_minimal(p, solution).ok_or(Error(Kind::InvalidParams))?;

    let mut state = initialise_state(p.n, p.k, p.hash_output());
    state.update(powheader);

    let root = tree_validator(&p, &state, &indices)?;
    if root.is_zero(p.collision_byte_length()) {
        Ok(())
    } else {
        Err(Error(Kind::NonZeroRootHash))
    }
}


