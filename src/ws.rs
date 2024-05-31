use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub struct ClientMessage {
    pub service: String,
    pub context: String,
    pub request: ClientRequest,
}

#[derive(Clone, Debug, Deserialize)]
pub enum ClientRequest {
    ChronoboxCsv { run_number: u32 },
    InitialOdb { run_number: u32 },
    SequencerEvents { run_number: u32 },
    TrgScalersCsv { run_number: u32 },
    VerticesCsv { run_number: u32 },
}

#[derive(Clone, Debug, Serialize)]
pub struct ServerMessage {
    pub service: String,
    pub context: String,
    pub response: ServerResponse,
}

#[derive(Clone, Debug, Serialize)]
pub enum ServerResponse {
    Text(String),
}
