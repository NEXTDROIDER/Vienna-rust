use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Context;
use axum::{routing::get, Json, Router};
use clap::Parser;
use tracing::info;

#[derive(Debug, Parser)]
#[command(name = "vienna-apiserver")]
#[command(about = "Rust rewrite scaffold for the Vienna API server")]
struct Args {
    #[arg(long, default_value_t = 8080)]
    port: u16,

    #[arg(long, default_value = "./earth.db")]
    db: PathBuf,

    #[arg(long = "static-data", default_value = "./data")]
    static_data: PathBuf,

    #[arg(long, default_value = "localhost:5532")]
    eventbus: String,

    #[arg(long, default_value = "localhost:5396")]
    objectstore: String,
}

#[derive(Clone)]
struct AppState {
    db: PathBuf,
    static_data: PathBuf,
    eventbus: String,
    objectstore: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let args = Args::parse();

    let state = AppState {
        db: args.db,
        static_data: args.static_data,
        eventbus: args.eventbus,
        objectstore: args.objectstore,
    };

    info!(db = %state.db.display(), "configured database");
    info!(static_data = %state.static_data.display(), "configured static data");
    info!(eventbus = %state.eventbus, "configured event bus");
    info!(objectstore = %state.objectstore, "configured object store");

    let app = Router::new()
        .route("/health", get(health))
        .route("/", get(root))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind API server to {addr}"))?;

    info!(address = %addr, "api server listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("api server exited with an error")?;

    Ok(())
}

async fn root() -> &'static str {
    "vienna apiserver rust scaffold"
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    ctrl_c.await;
    info!("shutdown signal received");
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vienna_apiserver=debug,tower_http=debug".into()),
        )
        .init();
}

