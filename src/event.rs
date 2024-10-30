use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct EventLog {
    pub standard: String,
    pub event: String,
    pub data: Vec<EventData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventData {
    pub request_id: Option<u32>,
    pub yield_id: Option<String>,
    pub prompt: Option<String>,
    pub status: String,
    pub response: Option<String>,
}