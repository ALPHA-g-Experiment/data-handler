use crate::core_command::{spawn_core_command, wait_core_command, AppState, CoreBin, CoreCmd};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

// The `service` and `context` fields are 100% ignored by the server. They are
// only used to help the client keep track of what each response corresponds to.
// These should just be passed directly and unmodified to the response.
#[derive(Clone, Debug, Deserialize)]
pub struct ClientMessage {
    pub service: String,
    pub context: String,
    pub request: ClientRequest,
}
// These are all the possible things a client can request from the server.
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

pub async fn handle_client_message(
    msg: ClientMessage,
    tx: mpsc::UnboundedSender<ServerMessage>,
    app_state: Arc<AppState>,
) {
    match msg.request {
        ClientRequest::ChronoboxCsv { .. } => {
            handle_chronobox_csv(msg, tx, app_state).await;
        }
        ClientRequest::InitialOdb { .. } => {
            handle_initial_odb(msg, tx, app_state).await;
        }
        ClientRequest::SequencerEvents { .. } => {
            handle_sequencer_events(msg, tx, app_state).await;
        }
        ClientRequest::TrgScalersCsv { .. } => {
            handle_trg_scalers_csv(msg, tx, app_state).await;
        }
        ClientRequest::VerticesCsv { .. } => {
            handle_vertices_csv(msg, tx, app_state).await;
        }
    }
}

async fn run_core_command(
    service: &str,
    context: &str,
    cmd: CoreCmd,
    tx: &mpsc::UnboundedSender<ServerMessage>,
    app_state: Arc<AppState>,
    // This returns a Result because it makes it easier to `tokio::try_join!`.
    // The error type doesn't matter at all, because any error is just reported
    // to the client as a text response.
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
                response: ServerResponse::Text(format!("Finished running `{:?}`", cmd.bin)),
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

async fn handle_chronobox_csv(
    msg: ClientMessage,
    tx: mpsc::UnboundedSender<ServerMessage>,
    app_state: Arc<AppState>,
) {
    let ClientRequest::ChronoboxCsv { run_number } = msg.request else {
        unreachable!();
    };
    let cmd = CoreCmd {
        bin: CoreBin::ChronoboxTimestamps,
        run_number,
    };
    let Ok(output) = run_core_command(&msg.service, &msg.context, cmd, &tx, app_state).await else {
        return;
    };
}

async fn handle_initial_odb(
    msg: ClientMessage,
    tx: mpsc::UnboundedSender<ServerMessage>,
    app_state: Arc<AppState>,
) {
    let ClientRequest::InitialOdb { run_number } = msg.request else {
        unreachable!();
    };
    let cmd = CoreCmd {
        bin: CoreBin::InitialOdb,
        run_number,
    };
    let Ok(output) = run_core_command(&msg.service, &msg.context, cmd, &tx, app_state).await else {
        return;
    };
}

async fn handle_sequencer_events(
    msg: ClientMessage,
    tx: mpsc::UnboundedSender<ServerMessage>,
    app_state: Arc<AppState>,
) {
    let ClientRequest::SequencerEvents { run_number } = msg.request else {
        unreachable!();
    };
    let sequencer_cmd = CoreCmd {
        bin: CoreBin::Sequencer,
        run_number,
    };
    let initial_odb_cmd = CoreCmd {
        bin: CoreBin::InitialOdb,
        run_number,
    };
    let chronobox_cmd = CoreCmd {
        bin: CoreBin::ChronoboxTimestamps,
        run_number,
    };
    let Ok((sequencer_csv, initial_odb_json, chronobox_csv)) = tokio::try_join!(
        run_core_command(
            &msg.service,
            &msg.context,
            sequencer_cmd,
            &tx,
            app_state.clone()
        ),
        run_core_command(
            &msg.service,
            &msg.context,
            initial_odb_cmd,
            &tx,
            app_state.clone()
        ),
        run_core_command(
            &msg.service,
            &msg.context,
            chronobox_cmd,
            &tx,
            app_state.clone()
        )
    ) else {
        return;
    };
}

async fn handle_trg_scalers_csv(
    msg: ClientMessage,
    tx: mpsc::UnboundedSender<ServerMessage>,
    app_state: Arc<AppState>,
) {
    let ClientRequest::TrgScalersCsv { run_number } = msg.request else {
        unreachable!();
    };
    let cmd = CoreCmd {
        bin: CoreBin::TrgScalers,
        run_number,
    };
    let Ok(output) = run_core_command(&msg.service, &msg.context, cmd, &tx, app_state).await else {
        return;
    };
}

async fn handle_vertices_csv(
    msg: ClientMessage,
    tx: mpsc::UnboundedSender<ServerMessage>,
    app_state: Arc<AppState>,
) {
    let ClientRequest::VerticesCsv { run_number } = msg.request else {
        unreachable!();
    };
    let cmd = CoreCmd {
        bin: CoreBin::Vertices,
        run_number,
    };
    let Ok(output) = run_core_command(&msg.service, &msg.context, cmd, &tx, app_state).await else {
        return;
    };
}
