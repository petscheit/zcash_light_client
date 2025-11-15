zcash_crypto
============

Minimal Zcash verification primitives.

What it provides
- Equihash verification (default n=200, k=9) bound to the header preimage (powheader).
- Difficulty filter check: SHA256d(header) <= ToTarget(nBits).
- Contextual difficulty calculation and verification (NU5/NU6 rules): expected nBits for the next height.
- Helpers to verify full PoW on a parsed `zcash_primitives::block::BlockHeader`.

Dependencies
- blake2b_simd (Equihash), sha2 (SHA256d), zcash_primitives (header types only).

Key APIs
- Equihash:
  - `zcash_crypto::verify_equihash_solution(powheader, solution)`
  - `zcash_crypto::verify_equihash_solution_with_params(n, k, powheader, solution)`
- Difficulty filter:
  - `zcash_crypto::verify_difficulty(header_hash_le, n_bits)`
- Contextual difficulty:
  - `zcash_crypto::DifficultyContext`
  - `zcash_crypto::difficulty::context::{expected_nbits, verify_difficulty}`
- Combined:
  - `zcash_crypto::verify_pow(&BlockHeader)`
  - `zcash_crypto::verify_pow_with_context(&BlockHeader, height, &mut DifficultyContext)`

Example
```rust
use zcash_crypto::{verify_pow, DifficultyContext, verify_pow_with_context};
use zcash_primitives::block::BlockHeader;

let header = BlockHeader::read(&raw[..]).unwrap();
verify_pow(&header).unwrap();

let mut ctx = DifficultyContext::new(height - 1);
// seed ctx with previous headers...
verify_pow_with_context(&header, height, &mut ctx).unwrap();
```


