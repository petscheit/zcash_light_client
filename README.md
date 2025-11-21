# ZCash Light Client Minimal

A ZCash light client implementation that verifies block headers using both Rust and Cairo Zero. This project provides a reference implementation for verifying ZCash proof-of-work (PoW) in both native Rust and as a provable Cairo program.

## Overview

This repository contains a minimal ZCash light client that validates block headers by verifying:
- Header hashing (SHA256d)
- Difficulty target conversion from nBits
- Difficulty filter validation
- Equihash solution verification
- Contextual difficulty adjustment (Rust only)

The implementation consists of two parallel verification paths:
1. **Rust implementation**: A complete, sound verification following ZCash specifications
2. **Cairo Zero implementation**: A provable verification that omits contextual difficulty validation (currently unsound)

## Rust Implementation

The Rust implementation (`crates/zcash_crypto`) provides a complete, specification-compliant verification that performs all required checks:

### Verification Steps

1. **Hash Header**: Computes SHA256d (double SHA256) of the full serialized block header
2. **Convert nBits to Difficulty**: Extracts and converts the compact `nBits` encoding to a 256-bit target value
3. **Assert Difficulty Filter**: Verifies that `Hash(header) <= ToTarget(nBits)` and that the target is within the PoW limit
4. **Assert Equihash Solution**: Validates the Equihash (n=200, k=9) solution by:
   - Decoding the minimal solution into indices
   - Building a binary merge tree with collision requirements
   - Verifying lexicographic ordering and disjoint index sets
   - Ensuring the root node is zero
5. **Assert Valid Difficulty Based on Previous Headers**: Validates that the `nBits` value matches the expected contextual difficulty adjustment based on the previous 28 blocks

### Components

- **`zcash_crypto`**: Core verification primitives (Equihash, difficulty filter, contextual difficulty)
- **`light_client_minimal`**: Light client binary that syncs headers via RPC and persists verified headers
- **`cairo_runner`**: Cairo VM runner that executes Cairo programs and generates proofs
- **`stwo_prover`**: STWO prover integration for generating zero-knowledge proofs

## Cairo Implementation

The Cairo Zero implementation (`cairo/`) provides a provable verification that can be used to generate zero-knowledge proofs of header validation. It performs the same verification steps as the Rust implementation, with one critical exception:

### Verification Steps

1. **Hash Header**: Computes SHA256d of the full serialized block header (using Cairo's SHA256 builtin)
2. **Convert nBits to Difficulty**: Extracts and converts `nBits` to target (same as Rust)
3. **Assert Difficulty Filter**: Verifies `Hash(header) <= ToTarget(nBits)` (same as Rust)
4. **Assert Equihash Solution**: Validates Equihash solution using a binary merge tree (same as Rust)

### Limitations

⚠️ **Currently Unsound**: The Cairo implementation **does not** verify contextual difficulty adjustment based on previous headers. This means it cannot detect difficulty manipulation attacks that would be caught by the Rust implementation. The implementation is sound for individual header verification but not for chain validation.

### Blake2b Constraint

The Equihash verification requires BLAKE2b hashing, which is not available as a Cairo builtin. The implementation uses **unconstrained hints** to compute BLAKE2b hashes. This means:

- The BLAKE2b computation is not proven in the Cairo program
- The prover must trust that the hint implementation correctly computes BLAKE2b
- This is a known limitation and the implementation should be considered experimental

## Usage

### Building

#### Rust

```bash
# Build all crates
cargo build --release
```

#### Cairo

First-time setup (only needed once):

```bash
# Set up the virtual environment and install dependencies
make setup
```

Building Cairo programs:

```bash
# Activate the virtual environment
source scripts/activate.sh

# Build Cairo programs
make build-cairo
```

The Cairo build process:
1. Activates a Python virtual environment with Cairo tooling
2. Compiles `cairo/src/main.cairo` to `cairo/build/main.json` in proof mode

### Running the Light Client

```bash
# Set ZCash RPC URL
export ZCASH_RPC_URL=http://127.0.0.1:8232

# Optional: Set starting height (default: 3,000,000)
export START_HEIGHT=3000000

# Run the light client (without proof generation)
cargo run --release -p light_client_minimal

# Run the light client with STWO proof generation
cargo run --release -p light_client_minimal -- --prove
```

**Command-line options:**
- `--prove` / `-p`: Generate STWO zero-knowledge proofs for each verified block. When enabled, proofs are saved to `output/block_{height}/proof_block_{height}.json`. Note: Proof generation significantly increases processing time per block.

The light client will:
- Fetch headers from the ZCash RPC endpoint
- Verify each header using both Rust and Cairo implementations
- Persist verified headers to `./data/headers.jsonl`
- Resume from the last verified height on restart
- Optionally generate proofs for each block (when `--prove` flag is used)

### Verifying a Single Header

```bash
# Using Rust implementation
cargo run --release -p zcash_crypto

# The Cairo implementation is invoked automatically during light client sync
```