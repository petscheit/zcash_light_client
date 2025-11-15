light_client_minimal
====================

A tiny light client binary that:
- fetches Zcash headers over JSON-RPC,
- verifies Equihash, difficulty filter, and contextual difficulty using `zcash_crypto`,
- persists verified headers to a JSONL file as hex-encoded bytes,
- resumes from the last verified height on restart.

Usage
- Build: `cargo build -p light_client_minimal`
- Run:
  - `ZCASH_RPC_URL=http://127.0.0.1:8232 cargo run -p light_client_minimal`
  - Optional: `START_HEIGHT=3000000` (ignored if persistence already has a tip)

Persistence
- Stored at `./data/headers.jsonl` by default.
- Each line is a JSON object: `{ "height": u32, "header_hex": String }`.
- On startup:
  - reads the last N headers to build the difficulty context,
  - continues syncing from the last stored height + 1.

Integration
- Library entry points (re-exported): `light_client_minimal::{net, store, sync}`.
- RPC client is minimal and supports `http://` and `https://` via reqwest (rustls).


