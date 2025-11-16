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
pub struct Params {
    n: u32,
    k: u32,
}

impl Params {
    /// Construct validated parameters.
    pub fn new(n: u32, k: u32) -> Option<Self> {
        if n.is_multiple_of(8) && (k >= 3) && (k < n) && n.is_multiple_of(k + 1) {
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
        self.collision_bit_length().div_ceil(8)
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

    let out_width = bit_len.div_ceil(8) + byte_pad;
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
pub fn indices_from_minimal(p: Params, minimal: &[u8]) -> Option<Vec<u32>> {
    let c_bit_len = p.collision_bit_length();
    if minimal.len() != ((1 << p.k) * (c_bit_len + 1)) / 8 {
        return None;
    }
    let digit_bytes = (c_bit_len + 1).div_ceil(8);
    let byte_pad = core::mem::size_of::<u32>() - digit_bytes;
    let expanded = expand_array(minimal, c_bit_len + 1, byte_pad);
    if !expanded.len().is_multiple_of(4) {
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
    println!("state: {:?}", state);

    let root = tree_validator(&p, &state, &indices)?;
    if root.is_zero(p.collision_byte_length()) {
        Ok(())
    } else {
        Err(Error(Kind::NonZeroRootHash))
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indices_from_minimal() {
        let p = Params::new(200, 9).unwrap();
        let solution: Vec<u8> = vec![0, 148, 157, 85, 222, 12, 198, 51, 224, 204, 228, 30, 70, 73, 239, 74, 163, 52, 159, 1, 0, 41, 15, 254, 40, 27, 148, 123, 59, 83, 251, 210, 243, 91, 28, 226, 146, 100, 155, 150, 172, 110, 8, 131, 175, 58, 104, 68, 185, 85, 146, 231, 69, 86, 218, 52, 75, 71, 1, 150, 28, 212, 19, 12, 104, 33, 156, 250, 19, 65, 213, 175, 181, 4, 158, 176, 232, 190, 74, 45, 146, 214, 120, 196, 7, 133, 227, 55, 5, 84, 139, 95, 58, 84, 240, 164, 195, 154, 47, 88, 238, 120, 74, 36, 22, 60, 216, 111, 84, 129, 35, 39, 223, 85, 225, 213, 92, 168, 75, 110, 123, 136, 122, 124, 191, 185, 9, 26, 88, 91, 219, 142, 164, 117, 147, 7, 197, 108, 27, 61, 175, 198, 105, 36, 90, 111, 101, 75, 111, 115, 0, 82, 38, 106, 1, 173, 79, 156, 11, 89, 237, 78, 23, 113, 43, 62, 114, 223, 4, 152, 170, 141, 228, 136, 143, 153, 53, 49, 198, 10, 205, 237, 29, 75, 102, 232, 157, 224, 182, 72, 44, 204, 212, 167, 18, 245, 207, 157, 76, 168, 59, 224, 249, 34, 222, 44, 29, 187, 58, 20, 7, 72, 13, 190, 135, 149, 153, 61, 139, 230, 64, 152, 138, 191, 231, 168, 161, 179, 58, 18, 19, 28, 69, 30, 26, 188, 13, 131, 251, 133, 24, 98, 198, 55, 206, 114, 77, 95, 233, 122, 169, 168, 6, 207, 52, 186, 181, 9, 244, 85, 75, 12, 209, 10, 125, 223, 213, 130, 27, 9, 26, 210, 201, 12, 26, 161, 216, 30, 179, 215, 45, 180, 25, 147, 182, 72, 244, 30, 33, 56, 255, 149, 49, 163, 15, 247, 59, 34, 20, 14, 78, 189, 123, 170, 51, 132, 142, 81, 45, 153, 48, 12, 92, 19, 28, 110, 117, 245, 113, 74, 92, 109, 203, 23, 139, 74, 73, 120, 218, 200, 58, 212, 18, 251, 214, 146, 1, 146, 80, 197, 83, 4, 154, 173, 69, 121, 132, 190, 223, 201, 106, 231, 1, 198, 89, 188, 112, 7, 169, 125, 10, 144, 2, 185, 69, 189, 236, 69, 169, 69, 239, 98, 133, 178, 205, 85, 59, 76, 9, 217, 7, 198, 39, 134, 63, 3, 153, 232, 114, 91, 79, 247, 252, 89, 121, 227, 207, 242, 40, 20, 80, 132, 72, 239, 139, 152, 49, 194, 133, 149, 147, 51, 57, 106, 163, 98, 165, 28, 242, 5, 9, 122, 250, 190, 193, 94, 65, 251, 110, 48, 182, 34, 55, 75, 245, 139, 55, 239, 157, 27, 36, 30, 173, 90, 104, 43, 152, 182, 87, 73, 165, 117, 104, 226, 56, 213, 10, 253, 65, 126, 30, 150, 14, 123, 90, 6, 79, 217, 246, 148, 215, 131, 162, 203, 205, 88, 85, 45, 237, 187, 158, 94, 17, 35, 103, 78, 247, 58, 82, 65, 150, 207, 5, 211, 229, 36, 102, 5, 73, 255, 231, 189, 101, 104, 5, 113, 53, 255, 213, 175, 217, 67, 246, 218, 17, 203, 181, 151, 232, 204, 236, 215, 126, 203, 233, 9, 222, 6, 49, 191, 162, 156, 211, 227, 213, 84, 70, 113, 186, 128, 37, 97, 83, 214, 233, 153, 11, 136, 173, 142, 12, 244, 152, 155, 239, 75, 228, 87, 249, 199, 176, 241, 170, 205, 110, 14, 243, 32, 96, 92, 41, 237, 12, 210, 235, 108, 252, 226, 22, 197, 42, 49, 117, 128, 32, 28, 173, 122, 9, 67, 210, 75, 123, 6, 213, 191, 117, 135, 97, 221, 150, 225, 25, 112, 181, 222, 214, 151, 34, 43, 44, 119, 231, 242, 86, 166, 5, 172, 117, 85, 73, 193, 101, 31, 37, 173, 252, 157, 83, 217, 17, 126, 58, 11, 180, 9, 238, 228, 166, 0, 18, 4, 114, 148, 156, 125, 218, 28, 46, 219, 60, 51, 12, 127, 150, 23, 153, 130, 145, 100, 87, 211, 49, 233, 99, 9, 221, 36, 223, 116, 238, 221, 0, 231, 219, 73, 126, 225, 48, 247, 125, 230, 102, 235, 85, 127, 179, 22, 232, 122, 218, 241, 129, 60, 228, 38, 164, 88, 166, 238, 227, 168, 91, 42, 184, 143, 101, 83, 170, 218, 232, 222, 101, 46, 33, 26, 29, 159, 51, 77, 89, 107, 94, 182, 23, 52, 7, 239, 204, 46, 129, 84, 187, 156, 161, 33, 42, 169, 161, 161, 18, 29, 47, 90, 119, 18, 207, 37, 204, 129, 72, 184, 5, 46, 13, 46, 9, 242, 14, 91, 162, 169, 130, 119, 233, 117, 176, 238, 217, 168, 146, 6, 150, 99, 55, 22, 63, 33, 92, 157, 4, 166, 89, 139, 9, 88, 211, 51, 216, 70, 119, 60, 105, 229, 171, 253, 10, 4, 39, 243, 102, 6, 20, 221, 130, 183, 154, 219, 133, 26, 13, 88, 182, 45, 245, 240, 179, 172, 131, 110, 110, 37, 243, 165, 31, 73, 169, 154, 222, 87, 121, 111, 233, 252, 194, 111, 10, 31, 148, 255, 8, 25, 254, 82, 183, 80, 135, 237, 190, 211, 168, 22, 38, 235, 84, 22, 198, 101, 87, 241, 28, 15, 206, 223, 242, 35, 214, 170, 140, 213, 195, 83, 134, 229, 180, 185, 90, 15, 3, 146, 202, 48, 26, 56, 179, 104, 125, 9, 68, 147, 185, 233, 210, 100, 208, 122, 25, 12, 229, 125, 17, 104, 4, 56, 42, 63, 171, 225, 90, 244, 223, 79, 160, 67, 240, 40, 122, 161, 237, 85, 104, 217, 239, 93, 18, 81, 13, 1, 12, 205, 171, 78, 182, 22, 246, 223, 19, 187, 49, 38, 239, 67, 217, 214, 87, 53, 228, 228, 192, 75, 87, 99, 72, 208, 64, 181, 53, 5, 90, 61, 90, 225, 145, 183, 95, 6, 18, 243, 178, 64, 102, 160, 82, 69, 242, 127, 229, 123, 218, 102, 189, 109, 236, 126, 79, 201, 203, 35, 104, 2, 6, 42, 221, 227, 205, 14, 49, 52, 130, 201, 42, 12, 114, 17, 2, 177, 243, 139, 1, 90, 184, 208, 21, 89, 203, 203, 64, 246, 116, 233, 239, 173, 94, 233, 194, 254, 19, 63, 170, 85, 202, 29, 208, 255, 38, 113, 15, 157, 168, 25, 204, 20, 89, 203, 126, 210, 96, 218, 211, 219, 5, 150, 37, 141, 71, 199, 76, 50, 168, 184, 82, 182, 113, 197, 160, 202, 162, 0, 22, 3, 217, 12, 145, 167, 223, 46, 45, 78, 233, 174, 155, 241, 166, 177, 236, 136, 21, 28, 98, 54, 13, 3, 2, 77, 46, 45, 1, 20, 8, 79, 107, 136, 197, 187, 162, 74, 167, 206, 207, 172, 22, 233, 30, 11, 175, 61, 134, 83, 226, 24, 9, 62, 129, 210, 166, 60, 50, 239, 241, 217, 3, 15, 158, 20, 20, 236, 228, 32, 218, 162, 78, 13, 213, 184, 69, 179, 39, 75, 184, 57, 202, 28, 83, 188, 192, 25, 66, 66, 215, 75, 38, 49, 185, 73, 90, 101, 79, 187, 220, 191, 173, 119, 159, 115, 34, 182, 7, 54, 36, 152, 128, 96, 72, 33, 217, 105, 36, 227, 250, 57, 127, 53, 74, 94, 204, 163, 79, 97, 77, 165, 69, 111, 155, 54, 51, 140, 55, 216, 246, 251, 246, 38, 190, 152, 52, 119, 118, 96, 34, 135, 39, 70, 218, 16, 161, 119, 28, 235, 2, 221, 138, 172, 1, 186, 24, 107, 241, 72, 134, 48, 71, 158, 18, 132, 218, 1, 144, 252, 232, 181, 154, 198, 176, 253, 65, 107, 238, 86, 183, 47, 10, 88, 69, 21, 53, 87, 255, 15, 73, 80, 160, 220, 91, 230, 92, 233, 66, 210, 46, 24, 83, 76, 78, 14, 250, 187, 45, 21, 37, 220, 72, 88, 185, 176, 247, 125, 71, 74, 18, 94, 188, 37, 14, 8, 254, 219, 250, 166, 111, 69, 61, 144, 147, 44, 171, 63, 244, 82, 33, 144, 153, 104, 229, 30, 107, 194, 84, 213, 9, 173, 235, 117, 203, 167, 109, 72, 254, 2, 78, 62, 102, 216, 223, 94];
        let indices = indices_from_minimal(p, solution.as_slice()).unwrap();

        let expected: Vec<u32> = vec![4755, 1398648, 418585, 1969358, 539788, 1211346, 1382820, 2031872, 336383, 1613934, 671133, 1392573, 386742, 473252, 1254620, 1485934, 69749, 1894817, 154794, 1650292, 699828, 856785, 1576112, 1889299, 101636, 422888, 631018, 1768272, 605537, 1716114, 1141910, 1472708, 61628, 842773, 673199, 1287503, 84359, 429014, 488386, 664598, 498445, 1921540, 1151983, 1400349, 702800, 1235870, 1852371, 1884089, 74571, 94062, 479802, 1650812, 710710, 1010673, 1263906, 1732453, 617966, 786760, 1258752, 1758457, 1578675, 1790853, 1804633, 1995487, 37653, 669586, 280524, 1266460, 791963, 1787730, 1783620, 1958070, 591257, 1266332, 621287, 1692874, 1079233, 1984695, 1138925, 1784340, 59649, 1505822, 707742, 1621604, 78101, 1047018, 331161, 1708563, 231587, 1600240, 442877, 1593734, 363631, 1285267, 720715, 1747368, 55782, 1239764, 326186, 1355981, 136443, 1570144, 1103944, 1757897, 99156, 483450, 1698710, 1786265, 486545, 1902472, 641020, 1388963, 130791, 821328, 468830, 1555107, 461084, 1330022, 622690, 1839900, 904894, 1426729, 931557, 1145012, 1348337, 1487374, 1482903, 1824402, 12874, 202060, 150870, 1333144, 622015, 1202873, 1576498, 1686640, 62767, 1321536, 89250, 1826500, 742027, 1824929, 890474, 1391436, 80672, 2037918, 204673, 1678983, 308895, 1965846, 773918, 1045032, 166416, 1123262, 379928, 1845337, 730726, 940712, 1774888, 1896965, 77663, 719621, 991485, 1499915, 803950, 1244514, 1687420, 1907492, 251307, 630958, 809771, 1350231, 709060, 931138, 1567243, 1973910, 118635, 530751, 850762, 882746, 366490, 1447243, 1011164, 1990161, 290025, 1826025, 598219, 847965, 510536, 1671506, 1048381, 1926504, 44582, 1572694, 1567905, 2059681, 235371, 391731, 485051, 2018281, 80832, 1623806, 1134185, 1981781, 560355, 958473, 723614, 1501593, 94485, 1456179, 1723469, 2028734, 569331, 1174588, 874091, 921331, 265227, 1091508, 420213, 1494990, 273802, 691293, 786688, 1879418, 75898, 601580, 223967, 1529974, 244525, 1590876, 372470, 1480482, 353678, 2072521, 742146, 1754965, 693122, 1656777, 880612, 1922009, 143303, 536272, 325490, 679937, 264421, 337695, 970977, 973628, 418191, 1988702, 835912, 1459581, 418770, 1622647, 599803, 1371869, 7419, 861691, 1087611, 1564262, 906922, 2092229, 1524694, 1765761, 498820, 1741154, 1275761, 1738162, 1405214, 1660138, 1496902, 1991982, 271171, 1473741, 437429, 1436513, 944143, 1831691, 1313445, 1809569, 271701, 427652, 593559, 1419121, 368203, 1253458, 376873, 920878, 81473, 1666698, 1360187, 2004827, 122291, 664705, 1356569, 1513023, 273299, 1315481, 836996, 1412403, 503948, 1953562, 994655, 1903108, 327276, 1579091, 967003, 1682872, 668698, 1453451, 1027973, 1289347, 904644, 1560212, 1025236, 1682917, 979679, 1736496, 1275984, 2069759, 66367, 1657565, 541686, 1830202, 1059917, 1758469, 1454890, 1569052, 129499, 2082959, 742726, 875573, 462283, 863830, 1079324, 1231408, 214806, 893428, 303689, 1810077, 313760, 2000451, 469992, 1140740, 460103, 2011013, 883311, 1374724, 516176, 2009211, 699206, 1699677, 150049, 1311795, 447911, 745839, 900647, 969801, 1538590, 1693271, 441500, 1245485, 766372, 853003, 682506, 1478486, 1510541, 1531654, 155254, 590234, 1059106, 2041854, 718772, 1683291, 1008626, 1034699, 290048, 530603, 979430, 1106707, 591250, 688924, 1083413, 1176459, 11095, 213077, 845285, 1314663, 644063, 743354, 923632, 1261482, 702787, 1524732, 1259655, 1694337, 1284136, 1471199, 1479430, 1758171, 45764, 1455391, 239129, 691077, 355555, 1140786, 1380352, 1442777, 102964, 2063544, 1484660, 1763775, 216419, 1778181, 582417, 1445123, 18853, 1618948, 656423, 1489036, 751428, 1223155, 1473888, 1501470, 95719, 1448271, 1117188, 1304605, 347256, 834556, 968728, 1023508, 171420, 1082218, 1124102, 1923972, 747086, 1240590, 938210, 1293504, 206920, 744748, 1251548, 1349030, 696183, 1519595, 769275, 1254070, 59076, 1204737, 1057808, 1939090, 641012, 942029, 676598, 828239, 797108, 1381822, 891673, 1622909, 1175031, 1935791, 1360291, 1537632, 282852, 1907560, 544955, 1167024, 375573, 721006, 1098591, 1132678, 395507, 1591827, 852168, 1035915, 734605, 802640, 745330, 1488687, 84744, 1332437, 786311, 1348874, 112823, 1677114, 661137, 923731, 625089, 1829612, 1477266, 1950853, 1143649, 1957713, 1724562, 2014245, 114975, 1798122, 1259426, 1300745, 416086, 1047828, 1117316, 1665253, 249208, 611156, 317173, 1531066, 973457, 2064531, 1176374, 1630046];
        assert_eq!(indices, expected);
    }
}