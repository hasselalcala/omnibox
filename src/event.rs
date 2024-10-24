use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct EventData {
    pub account_id: String,
    pub greeting: String,
}

#[derive(Deserialize, Debug)]
pub struct EventLog {
    pub standard: String,
    pub version: String,
    pub event: String,
    pub data: Vec<EventData>,
}
