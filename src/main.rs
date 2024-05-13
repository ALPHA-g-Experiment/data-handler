use anyhow::Context;
use askama_axum::Template;
use axum::extract::{self, State};
use axum::response::{IntoResponse, Response};
use axum::{http::StatusCode, routing::get, Router};
use clap::Parser;
use core_commands::CoreCommand;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::{fs, process::Child};

mod core_commands;

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

#[derive(Default)]
struct AppState {
    childs: tokio::sync::Mutex<HashMap<(u32, CoreCommand), Child>>,
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

async fn run_info(
    State(app_state): State<Arc<AppState>>,
    extract::Path(run_number): extract::Path<u32>,
) -> Result<RunInfoTemplate, AppError> {
    let contents = get_core_output(CoreCommand::FinalOdb, run_number, app_state)
        .await
        .with_context(|| format!("failed to get final ODB for run number `{run_number}`"))?;

    let start_index = contents.iter().position(|&c| c == b'{').with_context(|| {
        format!("failed to find JSON data in final ODB for run number `{run_number}`")
    })?;
    let odb = serde_json::from_slice::<Value>(&contents[start_index..])
        .with_context(|| format!("failed to parse final ODB for run number `{run_number}`"))?;

    Ok(RunInfoTemplate {
        run_number,
        start_time: odb
            .pointer("/Runinfo/Start time")
            .and_then(Value::as_str)
            .with_context(|| format!("failed to get start time for run number `{run_number}`"))?
            .to_string(),
        stop_time: odb
            .pointer("/Runinfo/Stop time")
            .and_then(Value::as_str)
            .with_context(|| format!("failed to get stop time for run number `{run_number}`"))?
            .to_string(),
        operator_comment: odb
            .pointer("/Experiment/Edit on start/Comment")
            .and_then(Value::as_str)
            .with_context(|| format!("failed to get comment for run number `{run_number}`"))?
            .to_string(),
    })
}

async fn get_core_output(
    command: CoreCommand,
    run_number: u32,
    app_state: Arc<AppState>,
) -> Result<Vec<u8>, anyhow::Error> {
    let cache_path = CACHE_PATH.get().unwrap().join(run_number.to_string());
    let output_path = cache_path.join(command.output(run_number));

    fs::create_dir_all(&cache_path).await.with_context(|| {
        format!(
            "failed to create cache directory `{}`",
            cache_path.display()
        )
    })?;

    match fs::read(&output_path).await {
        Ok(contents) => Ok(contents),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(e).context("failed here"),
        Err(e) => Err(e).with_context(|| format!("failed to read `{}`", output_path.display())),
    }
}

async fn get_run_files(run_number: u32) -> Result<Vec<std::path::PathBuf>, anyhow::Error> {
    let prefix = format!("run{run_number:05}sub");

    let mut run_files = Vec::new();
    let mut entries = fs::read_dir(MIDAS_DATA_PATH.get().unwrap())
        .await
        .context("failed to read MIDAS data directory")?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .context("failed to iterate over MIDAS data directory")?
    {
        let file_name = entry.file_name();
        if file_name
            .to_str()
            .with_context(|| format!("non-UTF-8 filename `{:?}`", entry.file_name()))?
            .starts_with(&prefix)
        {
            run_files.push(entry.path());
        }
    }
    run_files.sort_unstable();

    Ok(run_files)
}
