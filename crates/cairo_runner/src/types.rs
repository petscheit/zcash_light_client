#[derive(Debug, Clone)]
pub struct InputData {
    pub header_bytes: Vec<u8>,
    pub solution_indexes: Vec<u32>,
}
