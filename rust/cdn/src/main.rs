use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Context;
use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderValue, Method, StatusCode},
    response::Response,
    routing::get,
    Router,
};
use clap::Parser;
use tracing::info;

const DEFAULT_RESOURCE_PACK_PATH: &str =
    "/availableresourcepack/resourcepacks/dba38e59-091a-4826-b76a-a08d7de5a9e2-1301b0c257a311678123b9e7325d0d6c61db3c35";

#[derive(Debug, Parser)]
#[command(name = "vienna-cdn")]
#[command(about = "Rust rewrite of the Vienna CDN resource pack server")]
struct Args {
    #[arg(long, default_value_t = 8080)]
    port: u16,

    #[arg(long = "resource-pack-path", default_value = DEFAULT_RESOURCE_PACK_PATH)]
    resource_pack_path: String,

    #[arg(long = "resource-pack-file")]
    resource_pack_file: PathBuf,
}

#[derive(Clone)]
struct AppState {
    resource_pack_file: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let args = Args::parse();

    let app = Router::new()
        .route(args.resource_pack_path.as_str(), get(resource_pack).head(resource_pack))
        .with_state(AppState {
            resource_pack_file: args.resource_pack_file,
        });

    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind CDN server to {addr}"))?;

    info!(address = %addr, "cdn listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("cdn exited with an error")?;

    Ok(())
}

async fn resource_pack(
    State(state): State<AppState>,
    method: Method,
) -> Result<Response<Body>, StatusCode> {
    let metadata = tokio::fs::metadata(&state.resource_pack_file)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut response = Response::new(if method == Method::HEAD {
        Body::empty()
    } else {
        let data = tokio::fs::read(&state.resource_pack_file)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Body::from(data)
    });

    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/zip"),
    );
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&metadata.len().to_string())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    );
    *response.status_mut() = StatusCode::OK;
    Ok(response)
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vienna_cdn=debug".into()),
        )
        .init();
}

