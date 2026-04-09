use std::net::SocketAddr;

use anyhow::Context;
use clap::Parser;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{error, info};

#[derive(Debug, Parser)]
#[command(name = "vienna-eventbus-server")]
#[command(about = "Rust rewrite scaffold for the Vienna event bus server")]
struct Args {
    #[arg(long, default_value_t = 5532)]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let args = Args::parse();

    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind event bus server to {addr}"))?;

    info!(address = %addr, "event bus server listening");

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (mut stream, peer) = result.context("failed to accept event bus connection")?;
                info!(peer = %peer, "accepted event bus connection");

                tokio::spawn(async move {
                    let mut buffer = [0_u8; 1024];
                    loop {
                        match stream.read(&mut buffer).await {
                            Ok(0) => {
                                info!(peer = %peer, "event bus connection closed");
                                break;
                            }
                            Ok(read) => {
                                if let Err(error) = stream.write_all(&buffer[..read]).await {
                                    error!(peer = %peer, %error, "failed to echo event bus payload");
                                    break;
                                }
                            }
                            Err(error) => {
                                error!(peer = %peer, %error, "failed to read event bus payload");
                                break;
                            }
                        }
                    }
                });
            }
            _ = shutdown_signal() => {
                info!("shutdown signal received");
                break;
            }
        }
    }

    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vienna_eventbus_server=debug".into()),
        )
        .init();
}

