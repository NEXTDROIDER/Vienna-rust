use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{bail, Context};
use clap::Parser;
use tokio::net::TcpListener;
use tracing::info;
use vienna_objectstore::{serve, DataStore, ObjectStoreServer};

#[derive(Debug, Parser)]
#[command(name = "vienna-objectstore-server")]
#[command(about = "Rust rewrite of the Vienna object store server")]
struct Args {
    #[arg(long = "data-dir")]
    data_dir: PathBuf,

    #[arg(long, default_value_t = 5396)]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let args = Args::parse();

    ensure_data_dir(&args.data_dir)?;
    let data_store = DataStore::open(&args.data_dir).context("failed to open data store")?;
    let server = ObjectStoreServer::new(data_store);

    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind object store server to {addr}"))?;

    info!(data_dir = %args.data_dir.display(), address = %addr, "object store server listening");

    serve(listener, server, shutdown_signal())
        .await
        .context("object store server exited with an error")?;

    Ok(())
}

fn ensure_data_dir(path: &PathBuf) -> anyhow::Result<()> {
    if path.is_dir() {
        return Ok(());
    }

    bail!("data directory path is not a directory: {}", path.display())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vienna_objectstore_server=debug".into()),
        )
        .init();
}
