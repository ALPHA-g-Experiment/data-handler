use askama_axum::Template;
use axum::{extract, routing::get, Router};
use clap::Parser;
use tokio::sync::OnceCell;

static MIDAS_DATA_PATH: OnceCell<std::path::PathBuf> = OnceCell::const_new();

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
async fn main() {
    let args = Args::parse();
    MIDAS_DATA_PATH.set(args.data_dir).unwrap();

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/:run_number", get(run_info));

    let listener = tokio::net::TcpListener::bind(args.addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
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
