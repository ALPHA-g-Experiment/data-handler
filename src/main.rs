use anyhow::{Context, Result};
use askama_axum::Template;
use axum::{extract, routing::get, Router};
use clap::Parser;
use std::sync::OnceLock;

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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    MIDAS_DATA_PATH
        .set(args.data_dir)
        .expect("failed to set MIDAS_DATA_PATH");

    let project_dirs = directories::ProjectDirs::from("com", "ALPHA", "ALPHA-g-Data-Handler")
        .context("failed to get project directories")?;
    CACHE_PATH
        .set(project_dirs.cache_dir().to_path_buf())
        .expect("failed to set CACHE_PATH");

    println!("Cache path: {}", CACHE_PATH.get().unwrap().display());

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/:run_number", get(run_info));

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
    end_time: String,
    operator_comment: String,
}

async fn run_info(extract::Path(run_number): extract::Path<u32>) -> RunInfoTemplate {
    todo!()
}
