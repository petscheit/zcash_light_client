//! Simple persistence layer storing headers as hex-encoded bytes in a JSONL file.
//!
//! Each line is a JSON object: `{ "height": u32, "header_hex": String }`.
//! `tip()` returns the last seen height; `get(height)` scans the file for the last record.
use std::io;

pub trait Store {
    fn put(&self, height: u32, header_hex: &str) -> io::Result<()>;
    fn get(&self, height: u32) -> io::Result<Option<String>>;
    fn tip(&self) -> io::Result<Option<u32>>;
    fn last_n(&self, n: usize) -> io::Result<Vec<(u32, String)>>;
}

pub mod file;
