use std::collections::HashMap;
use std::net::SocketAddr;

use anyhow::Context;
use axum::{extract::State, routing::get, Json, Router};
use clap::Parser;
use serde::Serialize;
use tracing::info;

#[derive(Debug, Parser)]
#[command(name = "vienna-locator")]
#[command(about = "Rust rewrite of the Vienna locator service")]
struct Args {
    #[arg(long, default_value_t = 8080)]
    port: u16,

    #[arg(long)]
    api: String,

    #[arg(long)]
    cdn: String,

    #[arg(long = "playfab-title-id")]
    playfab_title_id: String,
}

#[derive(Clone)]
struct AppState {
    api: String,
    cdn: String,
    playfab_title_id: String,
}

#[derive(Serialize)]
struct LocatorResponse {
    result: LocatorResult,
    updates: HashMap<String, i32>,
}

#[derive(Serialize)]
struct LocatorResult {
    #[serde(rename = "serviceEnvironments")]
    service_environments: HashMap<String, ServiceEnvironment>,
    #[serde(rename = "supportedEnvironments")]
    supported_environments: HashMap<String, Vec<String>>,
}

#[derive(Serialize)]
struct ServiceEnvironment {
    #[serde(rename = "serviceUri")]
    service_uri: String,
    #[serde(rename = "cdnUri")]
    cdn_uri: String,
    #[serde(rename = "playfabTitleId")]
    playfab_title_id: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let args = Args::parse();
    let state = AppState {
        api: args.api,
        cdn: args.cdn,
        playfab_title_id: args.playfab_title_id,
    };

    let app = Router::new()
        .route("/player/environment", get(locator))
        .route("/api/v1.1/player/environment", get(locator))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind locator to {addr}"))?;

    info!(address = %addr, "locator listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("locator exited with an error")?;
    Ok(())
}

async fn locator(State(state): State<AppState>) -> Json<LocatorResponse> {
    let mut service_environments = HashMap::new();
    service_environments.insert(
        "production".to_owned(),
        ServiceEnvironment {
            service_uri: state.api,
            cdn_uri: state.cdn,
            playfab_title_id: state.playfab_title_id,
        },
    );

    let mut supported_environments = HashMap::new();
    supported_environments.insert("2020.1217.02".to_owned(), vec!["production".to_owned()]);

    Json(LocatorResponse {
        result: LocatorResult {
            service_environments,
            supported_environments,
        },
        updates: HashMap::new(),
    })
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vienna_locator=debug".into()),
        )
        .init();
}

