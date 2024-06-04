use crate::core_command::{spawn_core_command, wait_core_command, AppState, CoreBin, CoreCmd};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

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

async fn handle_core_command(
    service: &str,
    context: &str,
    cmd: CoreCmd,
    tx: &mpsc::UnboundedSender<ServerMessage>,
    app_state: Arc<AppState>,
) -> Result<PathBuf, ()> {
    if let Err(e) = spawn_core_command(cmd, app_state.clone()).await {
        let response = ServerMessage {
            service: service.to_string(),
            context: context.to_string(),
            response: ServerResponse::Text(format!("Error: {e:?}")),
        };
        let _ = tx.send(response);
        return Err(());
    }
    let response = ServerMessage {
        service: service.to_string(),
        context: context.to_string(),
        response: ServerResponse::Text(format!("Spawned `{:?}`", cmd.bin)),
    };
    let _ = tx.send(response);

    match wait_core_command(cmd, app_state.clone()).await {
        Ok(filename) => {
            let response = ServerMessage {
                service: service.to_string(),
                context: context.to_string(),
                response: ServerResponse::Text(format!("Finished `{:?}`", cmd.bin)),
            };
            let _ = tx.send(response);
            Ok(filename)
        }
        Err(e) => {
            let response = ServerMessage {
                service: service.to_string(),
                context: context.to_string(),
                response: ServerResponse::Text(format!("Error: {e:?}")),
            };
            let _ = tx.send(response);
            Err(())
        }
    }
}

pub async fn handle_client_message(
    msg: ClientMessage,
    tx: mpsc::UnboundedSender<ServerMessage>,
    app_state: Arc<AppState>,
) {
    match msg.request {
        ClientRequest::ChronoboxCsv { run_number } => {
            let cmd = CoreCmd {
                bin: CoreBin::ChronoboxTimestamps,
                run_number,
            };
            let Ok(f) = handle_core_command(&msg.service, &msg.context, cmd, &tx, app_state).await
            else {
                return;
            };
        }
        ClientRequest::InitialOdb { run_number } => {
            let cmd = CoreCmd {
                bin: CoreBin::InitialOdb,
                run_number,
            };
            let Ok(f) = handle_core_command(&msg.service, &msg.context, cmd, &tx, app_state).await
            else {
                return;
            };
        }
        ClientRequest::SequencerEvents { run_number } => {
            let cmd = CoreCmd {
                bin: CoreBin::Sequencer,
                run_number,
            };
            let Ok(f) = handle_core_command(&msg.service, &msg.context, cmd, &tx, app_state).await
            else {
                return;
            };
        }
        ClientRequest::TrgScalersCsv { run_number } => {
            let cmd = CoreCmd {
                bin: CoreBin::TrgScalers,
                run_number,
            };
            let Ok(f) = handle_core_command(&msg.service, &msg.context, cmd, &tx, app_state).await
            else {
                return;
            };
        }
        ClientRequest::VerticesCsv { run_number } => {
            let cmd = CoreCmd {
                bin: CoreBin::Vertices,
                run_number,
            };
            let Ok(f) = handle_core_command(&msg.service, &msg.context, cmd, &tx, app_state).await
            else {
                return;
            };
        }
    }
}
