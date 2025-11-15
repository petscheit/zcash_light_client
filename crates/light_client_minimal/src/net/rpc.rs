use hex;
use reqwest::{self, Client, StatusCode, Url, header};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value, json};
use std::fmt;

use zcash_primitives::block::{BlockHash, BlockHeader};

/// Errors that can occur when talking to a `zcashd` JSON-RPC endpoint.
#[derive(Debug)]
pub enum RpcError {
    NonHttpUrl,
    Client(String),
    Json(serde_json::Error),
    Status(StatusCode),
    Rpc { code: i64, message: String },
    Hex(hex::FromHexError),
    DecodeHeader(String),
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RpcError::NonHttpUrl => write!(f, "only http:// URLs are supported"),
            RpcError::Client(e) => write!(f, "client error: {e}"),
            RpcError::Json(e) => write!(f, "JSON error: {e}"),
            RpcError::Status(status) => write!(f, "unexpected HTTP status: {status}"),
            RpcError::Rpc { code, message } => {
                write!(f, "RPC error {code}: {message}")
            }
            RpcError::Hex(e) => write!(f, "hex decoding error: {e}"),
            RpcError::DecodeHeader(e) => write!(f, "failed to decode block header: {e}"),
        }
    }
}

impl std::error::Error for RpcError {}

impl From<serde_json::Error> for RpcError {
    fn from(e: serde_json::Error) -> Self {
        RpcError::Json(e)
    }
}

impl From<hex::FromHexError> for RpcError {
    fn from(e: hex::FromHexError) -> Self {
        RpcError::Hex(e)
    }
}

#[derive(Serialize)]
struct JsonRpcRequest<'a> {
    jsonrpc: &'static str,
    id: &'a str,
    method: &'a str,
    #[serde(borrow)]
    params: &'a [Value],
}

#[derive(Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

#[derive(Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
    id: Value,
}

/// Minimal JSON-RPC client for talking to a `zcashd`-compatible node over HTTP(S).
///
/// This is intentionally small and opinionated:
/// - only `http://` URLs are supported.
pub struct RpcClient {
    client: Client,
    url: Url,
}

impl RpcClient {
    /// Creates a new client for the given `zcashd` JSON-RPC endpoint.
    ///
    /// `url` should typically look like `http://127.0.0.1:8232` or an HTTPS endpoint such
    /// as `https://go.getblock.io/...`.
    pub fn new(url: &str) -> Result<Self, RpcError> {
        let url = Url::parse(url).map_err(|e| RpcError::Client(e.to_string()))?;
        match url.scheme() {
            "http" | "https" => {}
            _ => {
                return Err(RpcError::NonHttpUrl);
            }
        }

        let client = Client::new();

        Ok(RpcClient { client, url })
    }

    async fn call<T>(&self, method: &str, params: &[Value]) -> Result<T, RpcError>
    where
        T: DeserializeOwned,
    {
        let request_body = JsonRpcRequest {
            jsonrpc: "1.0",
            id: "light-client-minimal",
            method,
            params,
        };

        let req = self
            .client
            .post(self.url.clone())
            .header(header::CONTENT_TYPE, "application/json");

        let res = req
            .json(&request_body)
            .send()
            .await
            .map_err(|e| RpcError::Client(e.to_string()))?;

        if !res.status().is_success() {
            return Err(RpcError::Status(res.status()));
        }

        let bytes = res
            .bytes()
            .await
            .map_err(|e| RpcError::Client(e.to_string()))?;
        let rpc_response: JsonRpcResponse<T> = serde_json::from_slice(&bytes)?;

        if let Some(err) = rpc_response.error {
            return Err(RpcError::Rpc {
                code: err.code,
                message: err.message,
            });
        }

        rpc_response.result.ok_or_else(|| RpcError::Rpc {
            code: -1,
            message: "missing result field in RPC response".to_string(),
        })
    }

    /// Returns the current block height reported by the node (`getblockcount`).
    pub async fn get_block_count(&self) -> Result<u64, RpcError> {
        self.call("getblockcount", &[]).await
    }

    /// Returns the hash of the best chain tip (`getbestblockhash`).
    pub async fn get_best_block_hash(&self) -> Result<BlockHash, RpcError> {
        let hash_hex: String = self.call("getbestblockhash", &[]).await?;
        decode_block_hash_from_hex(&hash_hex)
    }

    /// Returns the block hash at the given height (`getblockhash`).
    pub async fn get_block_hash(&self, height: u32) -> Result<BlockHash, RpcError> {
        let hash_hex: String = self.call("getblockhash", &[json!(height)]).await?;
        decode_block_hash_from_hex(&hash_hex)
    }

    /// Returns the raw block bytes for the given hash (`getblock` with `verbosity = 0`).
    pub async fn get_block(&self, hash: &BlockHash) -> Result<Vec<u8>, RpcError> {
        let hash_hex = encode_block_hash_to_hex(hash);
        let block_hex: String = self.call("getblock", &[json!(hash_hex), json!(0)]).await?;
        Ok(hex::decode(block_hex)?)
    }

    /// Fetches a block and decodes its header using `zcash_primitives`.
    pub async fn get_block_header(&self, hash: &BlockHash) -> Result<BlockHeader, RpcError> {
        let raw_block = self.get_block(hash).await?;
        BlockHeader::read(&raw_block[..]).map_err(|e| RpcError::DecodeHeader(e.to_string()))
    }

    /// Convenience helper: fetches the header at a given height.
    pub async fn get_block_header_by_height(&self, height: u32) -> Result<BlockHeader, RpcError> {
        let hash = self.get_block_hash(height).await?;
        self.get_block_header(&hash).await
    }
}

fn decode_block_hash_from_hex(s: &str) -> Result<BlockHash, RpcError> {
    let mut bytes = hex::decode(s)?;
    bytes.reverse();
    BlockHash::try_from_slice(&bytes)
        .ok_or_else(|| RpcError::DecodeHeader("block hash must be 32 bytes".to_string()))
}

fn encode_block_hash_to_hex(hash: &BlockHash) -> String {
    let mut bytes = hash.0;
    bytes.reverse();
    hex::encode(bytes)
}
