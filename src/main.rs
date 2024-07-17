use crate::communication::{handle_client_message, Claims};
use crate::core_command::{
    install_core_binaries, spawn_core_command, wait_core_command, AppState, CoreBin, CoreCmd,
};
use crate::secondary_script::setup_analysis_scripts;
use crate::templates::RunInfoTemplate;
use anyhow::Context;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{self, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::{routing::get, Router};
use clap::{Parser, Subcommand};
use futures::{sink::SinkExt, stream::StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use jsonwebtoken::{decode, DecodingKey, Validation};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tokio::{fs, sync::mpsc};
use tower_http::services::ServeDir;

mod communication;
mod core_command;
mod secondary_script;
mod templates;

static PROJECT_HOME: OnceLock<PathBuf> = OnceLock::new();

#[derive(Parser)]
/// Web application for the ALPHA-g experiment
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Update all internal analysis programs to the latest supported version
    Update,
    /// Start the web server
    Serve {
        /// The address to listen on
        #[arg(short, long, default_value = "127.0.0.1:8080")]
        address: String,
        /// Path to the MIDAS data directory
        #[arg(short, long, default_value = ".")]
        data_dir: std::path::PathBuf,
    },
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
    PROJECT_HOME
        .set(
            directories::BaseDirs::new()
                .context("failed to get base directories")?
                .home_dir()
                .join(".alpha-g-data-handler"),
        )
        .expect("failed to set PROJECT_HOME");

    let cli = Cli::parse();

    match cli.command {
        Commands::Update => {
            let spinner = ProgressBar::new_spinner()
                .with_style(ProgressStyle::default_spinner().tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ "));
            spinner.enable_steady_tick(std::time::Duration::from_millis(100));

            spinner.set_message("Installing core binaries...");
            install_core_binaries().context("failed to install core binaries")?;

            spinner.set_message("Setting up analysis scripts...");
            setup_analysis_scripts().context("failed to set up analysis scripts")?;

            spinner.finish_and_clear();
        }
        Commands::Serve { address, data_dir } => {
            core_command::MIDAS_DATA_PATH
                .set(data_dir)
                .expect("failed to set MIDAS_DATA_PATH");

            let app_state = Arc::new(AppState::default());
            let app = Router::new()
                .route("/", get(index))
                .route("/:run_number", get(run_info))
                .route("/ws", get(websocket_handler))
                .route("/download/:token", get(download_handler))
                .nest_service(
                    "/assets",
                    ServeDir::new(env!("CARGO_MANIFEST_DIR").to_owned() + "/assets"),
                )
                .with_state(app_state);

            let listener = tokio::net::TcpListener::bind(address)
                .await
                .context("failed to create tcp listener")?;
            axum::serve(listener, app)
                .await
                .context("failed to start server")?;
        }
    }

    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(std::include_str!("../assets/index.html"))
}

async fn run_info(
    State(app_state): State<Arc<AppState>>,
    extract::Path(run_number): extract::Path<u32>,
) -> Result<RunInfoTemplate, AppError> {
    let cmd = CoreCmd {
        bin: CoreBin::FinalOdb,
        run_number,
    };

    spawn_core_command(cmd, app_state.clone())
        .await
        .with_context(|| {
            format!(
                "failed to spawn `{:?}` for run number `{run_number}`",
                cmd.bin
            )
        })?;

    let output = wait_core_command(cmd, app_state).await.with_context(|| {
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
    let odb = serde_json::from_slice::<serde_json::Value>(&contents[start_index..])
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

async fn websocket(ws: WebSocket, app_state: Arc<AppState>) {
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
                let Ok(msg) = serde_json::from_str(&msg) else {
                    continue;
                };

                let tx = mpsc_tx.clone();
                let app_state = app_state.clone();
                tokio::spawn(async move {
                    handle_client_message(msg, tx, app_state).await;
                });
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => (),
        _ = (&mut recv_task) => (),
    }
}

async fn download_handler(
    extract::Path(token): extract::Path<String>,
) -> Result<(HeaderMap, Vec<u8>), AppError> {
    let secret = std::env::var("AG_JWT_SECRET").context("failed to get JWT shared secret")?;
    let token = decode::<Claims>(
        &token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    )
    .context("failed to decode JWT string")?;

    let path = token.claims.path;
    let contents = fs::read(&path)
        .await
        .with_context(|| format!("failed to read `{}`", path.display()))?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        // I don't want to specify any type because e.g. the JSON odb file
        // starts with a comment, which would make it not a valid JSON.
        // Simply just say everything is binary data.
        "application/octet-stream".parse().unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!(
            "attachment; filename=\"{}\"",
            path.file_name()
                .context("failed to get file name")?
                .to_string_lossy()
        )
        .parse()
        .unwrap(),
    );

    Ok((headers, contents))
}
