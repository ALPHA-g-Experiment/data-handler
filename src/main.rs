use anyhow::{ensure, Context};
use askama_axum::Template;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{self, State};
use axum::response::{IntoResponse, Response};
use axum::{http::StatusCode, routing::get, Router};
use clap::Parser;
use cmd::{spawn_core_command, wait_core_command, AppState, CoreBin, CoreCmd};
use futures::{sink::SinkExt, stream::StreamExt};
use serde_json::Value;
use std::sync::{Arc, OnceLock};
use tokio::fs;
use tokio::sync::mpsc;
use ws::{ClientMessage, ClientRequest, ServerMessage, ServerResponse};

mod cmd;
mod ws;

static MIDAS_DATA_PATH: OnceLock<std::path::PathBuf> = OnceLock::new();
static CACHE_PATH: OnceLock<std::path::PathBuf> = OnceLock::new();

#[derive(Parser)]
struct Args {
    /// The address to listen on
    #[arg(short, long, default_value = "127.0.0.1:8080")]
    addr: String,
    /// Path to the MIDAS data directory
    #[arg(short, long, default_value = ".")]
    data_dir: std::path::PathBuf,
}

struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error: {:?}", self.0),
        )
            .into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let args = Args::parse();
    MIDAS_DATA_PATH
        .set(args.data_dir)
        .expect("failed to set MIDAS_DATA_PATH");

    let project_dirs = directories::ProjectDirs::from("com", "ALPHA", "ALPHA-g-Data-Handler")
        .context("failed to get project directories")?;
    CACHE_PATH
        .set(project_dirs.cache_dir().to_path_buf())
        .expect("failed to set CACHE_PATH");

    let app_state = Arc::new(AppState::default());
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/:run_number", get(run_info))
        .route("/ws", get(websocket_handler))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(args.addr)
        .await
        .context("failed to create tcp listener")?;
    axum::serve(listener, app)
        .await
        .context("failed to start server")?;

    Ok(())
}

#[derive(Template)]
#[template(path = "run_info.html")]
struct RunInfoTemplate {
    run_number: u32,
    start_time: String,
    stop_time: String,
    operator_comment: String,
}

impl RunInfoTemplate {
    fn try_from_odb(odb: &Value) -> Result<Self, anyhow::Error> {
        let run_number = odb
            .pointer("/Runinfo/Run number")
            .and_then(Value::as_u64)
            .context("failed to get run number")?
            .try_into()
            .context("failed to convert run number to u32")?;
        let start_time = odb
            .pointer("/Runinfo/Start time binary")
            .and_then(Value::as_str)
            .and_then(|s| s.strip_prefix("0x"))
            .context("failed to get binary start time")
            .and_then(|s| i64::from_str_radix(s, 16).map_err(|e| anyhow::anyhow!(e)))
            .context("failed to parse start time as u64")?;
        let stop_time = odb
            .pointer("/Runinfo/Stop time binary")
            .and_then(Value::as_str)
            .and_then(|s| s.strip_prefix("0x"))
            .context("failed to get binary stop time")
            .and_then(|s| i64::from_str_radix(s, 16).map_err(|e| anyhow::anyhow!(e)))
            .context("failed to parse stop time as u64")?;
        ensure!(start_time < stop_time, "start time after stop time");
        let operator_comment = odb
            .pointer("/Experiment/Edit on start/Comment")
            .and_then(Value::as_str)
            .context("failed to get comment")?
            .to_string();

        let start_time = time::OffsetDateTime::from_unix_timestamp(start_time)
            .context("failed to convert start time to `OffsetDateTime`")?
            .format(&time::format_description::well_known::Rfc2822)
            .context("failed to format start time")?;
        let stop_time = time::OffsetDateTime::from_unix_timestamp(stop_time)
            .context("failed to convert stop time to `OffsetDateTime`")?
            .format(&time::format_description::well_known::Rfc2822)
            .context("failed to format stop time")?;

        Ok(Self {
            run_number,
            start_time,
            stop_time,
            operator_comment,
        })
    }
}

async fn run_info(
    State(app_state): State<Arc<AppState>>,
    extract::Path(run_number): extract::Path<u32>,
) -> Result<RunInfoTemplate, AppError> {
    let cmd = CoreCmd {
        bin: CoreBin::FinalOdb,
        run_number,
        data_dir: MIDAS_DATA_PATH.get().unwrap().clone(),
        output_dir: CACHE_PATH.get().unwrap().join(run_number.to_string()),
    };

    spawn_core_command(cmd.clone(), app_state.clone())
        .await
        .with_context(|| {
            format!(
                "failed to spawn `{:?}` for run number `{run_number}`",
                cmd.bin
            )
        })?;

    let output = wait_core_command(&cmd, app_state).await.with_context(|| {
        format!(
            "failed to wait `{:?}` for run number `{run_number}`",
            cmd.bin
        )
    })?;
    let contents = fs::read(&output)
        .await
        .with_context(|| format!("failed to read `{}`", output.display()))?;

    let start_index = contents.iter().position(|&c| c == b'{').with_context(|| {
        format!("failed to find JSON data in final ODB for run number `{run_number}`")
    })?;
    let odb = serde_json::from_slice::<Value>(&contents[start_index..])
        .with_context(|| format!("failed to parse final ODB for run number `{run_number}`"))?;
    let template = RunInfoTemplate::try_from_odb(&odb).with_context(|| {
        format!("failed to create `RunInfo` from ODB for run number `{run_number}`")
    })?;

    Ok(template)
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(app_state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, app_state))
}

async fn websocket(mut ws: WebSocket, app_state: Arc<AppState>) {
    let (mut ws_tx, mut ws_rx) = ws.split();
    let (mpsc_tx, mut mpsc_rx) = mpsc::unbounded_channel();

    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = mpsc_rx.recv().await {
            let msg = serde_json::to_string(&msg).unwrap();
            if ws_tx.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            if let Message::Text(msg) = msg {
                let Ok(msg) = serde_json::from_str::<ClientMessage>(&msg) else {
                    continue;
                };

                let tx = mpsc_tx.clone();
                tokio::spawn(async move {
                    let response = ServerMessage {
                        service: msg.service.clone(),
                        context: msg.context.clone(),
                        response: ServerResponse::Text("Hello from server!".to_string()),
                    };
                    tx.send(response).unwrap();

                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

                    let response = ServerMessage {
                        service: msg.service,
                        context: msg.context,
                        response: ServerResponse::Text("Bye from server!".to_string()),
                    };
                    tx.send(response).unwrap();
                });
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => (),
        _ = (&mut recv_task) => (),
    }
}
