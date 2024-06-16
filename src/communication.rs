use crate::core_command::{spawn_core_command, wait_core_command, AppState, CoreBin, CoreCmd};
use crate::secondary_script::{self, SecondaryScript};
use jsonwebtoken::{encode, get_current_timestamp, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

// The `service` and `context` fields are 100% ignored by the server. They are
// only used to help the client keep track of what each response corresponds to.
// These should just be passed directly and unmodified to the ServerMessage.
#[derive(Clone, Debug, Deserialize)]
pub struct ClientMessage {
    pub service: String,
    pub context: String,
    pub request: ClientRequest,
}
// These are all the possible things a client can request from the server.
#[derive(Clone, Debug, Deserialize)]
pub enum ClientRequest {
    ChronoboxCsv {
        run_number: u32,
    },
    InitialOdb {
        run_number: u32,
    },
    SequencerEvents {
        run_number: u32,
    },
    TrgScalersCsv {
        run_number: u32,
    },
    TrgScalersPlot {
        run_number: u32,
        args: secondary_script::TrgScalersArgs,
    },
    VerticesCsv {
        run_number: u32,
    },
    VerticesPlot {
        run_number: u32,
        args: secondary_script::VerticesArgs,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct ServerMessage {
    pub service: String,
    pub context: String,
    pub response: ServerResponse,
}
// Whenever a file is available for download, the server will send a JWT to the
// client. This allows stateless authentication for the download.
//
// Shared secret is stored in server environment variable `AG_JWT_SECRET`.
#[derive(Deserialize, Serialize)]
pub struct Claims {
    exp: u64,
    // Absolute path in the server.
    pub path: std::path::PathBuf,
}

#[derive(Clone, Debug, Serialize)]
pub enum ServerResponse {
    Text(String),
    Error(String),
    DownloadJWT(String),
}

pub async fn handle_client_message(
    msg: ClientMessage,
    // Any message that needs to be sent to the client should be sent through
    // this channel. This is handled by a `send_task` in the main websocket
    // loop.
    tx: mpsc::UnboundedSender<ServerMessage>,
    app_state: Arc<AppState>,
) {
    // All these "handle_*" functions take the same arguments instead of just
    // the minimum required for the specific request. This is because, for more
    // complex requests, this will start getting out of hand and unreadable.
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
        ClientRequest::TrgScalersPlot { .. } => {
            handle_trg_scalers_plot(msg, tx, app_state).await;
        }
        ClientRequest::VerticesCsv { .. } => {
            handle_vertices_csv(msg, tx, app_state).await;
        }
        ClientRequest::VerticesPlot { .. } => {
            handle_vertices_plot(msg, tx, app_state).await;
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
    // to the client as a response.
) -> Result<PathBuf, ()> {
    if let Err(e) = spawn_core_command(cmd, app_state.clone()).await {
        let response = ServerMessage {
            service: service.to_string(),
            context: context.to_string(),
            response: ServerResponse::Error(format!("Error: {e:?}")),
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
                response: ServerResponse::Error(format!("Error: {e:?}")),
            };
            let _ = tx.send(response);
            Err(())
        }
    }
}

async fn run_secondary_script<S: SecondaryScript>(
    service: &str,
    context: &str,
    script: S,
    output: &str,
    tx: &mpsc::UnboundedSender<ServerMessage>,
    // Same as `run_core_command`.
) -> Result<PathBuf, ()> {
    let response = ServerMessage {
        service: service.to_string(),
        context: context.to_string(),
        response: ServerResponse::Text(format!("Spawned `{script}`")),
    };
    let _ = tx.send(response);

    match script.spawn_and_wait(output).await {
        Ok(filename) => {
            let response = ServerMessage {
                service: service.to_string(),
                context: context.to_string(),
                response: ServerResponse::Text(format!("Finished running `{script}`")),
            };
            let _ = tx.send(response);
            Ok(filename)
        }
        Err(e) => {
            let response = ServerMessage {
                service: service.to_string(),
                context: context.to_string(),
                response: ServerResponse::Error(format!("Error: {e:?}")),
            };
            let _ = tx.send(response);
            Err(())
        }
    }
}

fn send_download_jwt(
    service: &str,
    context: &str,
    tx: &mpsc::UnboundedSender<ServerMessage>,
    path: PathBuf,
) {
    let claims = Claims {
        // Arbitrary short expiration time.
        exp: get_current_timestamp() + 120,
        path,
    };

    let Ok(secret) = std::env::var("AG_JWT_SECRET") else {
        let response = ServerMessage {
            service: service.to_string(),
            context: context.to_string(),
            response: ServerResponse::Error("Error: JWT secret not set in server".to_string()),
        };
        let _ = tx.send(response);
        return;
    };
    match encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    ) {
        Ok(token) => {
            let response = ServerMessage {
                service: service.to_string(),
                context: context.to_string(),
                response: ServerResponse::DownloadJWT(token),
            };
            let _ = tx.send(response);
        }
        Err(e) => {
            let response = ServerMessage {
                service: service.to_string(),
                context: context.to_string(),
                response: ServerResponse::Error(format!("Error: {e:?}")),
            };
            let _ = tx.send(response);
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
    send_download_jwt(&msg.service, &msg.context, &tx, output);
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
    send_download_jwt(&msg.service, &msg.context, &tx, output);
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

    let script = secondary_script::Sequencer {
        sequencer_csv,
        initial_odb_json,
        chronobox_csv,
    };
    let Ok(output) = run_secondary_script(
        &msg.service,
        &msg.context,
        script,
        &format!("R{run_number}_sequencer_events.csv"),
        &tx,
    )
    .await
    else {
        return;
    };
    send_download_jwt(&msg.service, &msg.context, &tx, output);
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
    send_download_jwt(&msg.service, &msg.context, &tx, output);
}

async fn handle_trg_scalers_plot(
    msg: ClientMessage,
    tx: mpsc::UnboundedSender<ServerMessage>,
    app_state: Arc<AppState>,
) {
    let ClientRequest::TrgScalersPlot { run_number, args } = msg.request else {
        unreachable!();
    };
    let cmd = CoreCmd {
        bin: CoreBin::TrgScalers,
        run_number,
    };
    let Ok(csv) = run_core_command(&msg.service, &msg.context, cmd, &tx, app_state).await else {
        return;
    };

    let script = secondary_script::TrgScalers { csv, args };
    let Ok(output) = run_secondary_script(
        &msg.service,
        &msg.context,
        script,
        &format!("R{run_number}_trg_scalers_plot.pdf"),
        &tx,
    )
    .await
    else {
        return;
    };
    send_download_jwt(&msg.service, &msg.context, &tx, output);
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
    send_download_jwt(&msg.service, &msg.context, &tx, output);
}

async fn handle_vertices_plot(
    msg: ClientMessage,
    tx: mpsc::UnboundedSender<ServerMessage>,
    app_state: Arc<AppState>,
) {
    let ClientRequest::VerticesPlot { run_number, args } = msg.request else {
        unreachable!();
    };
    let cmd = CoreCmd {
        bin: CoreBin::Vertices,
        run_number,
    };
    let Ok(csv) = run_core_command(&msg.service, &msg.context, cmd, &tx, app_state).await else {
        return;
    };

    let script = secondary_script::Vertices { csv, args };
    let Ok(output) = run_secondary_script(
        &msg.service,
        &msg.context,
        script,
        &format!("R{run_number}_vertices_plot.pdf"),
        &tx,
    )
    .await
    else {
        return;
    };
    send_download_jwt(&msg.service, &msg.context, &tx, output);
}
