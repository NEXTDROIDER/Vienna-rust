use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::Context;
use axum::{extract::{Path, State}, routing::get, Json, Router};
use clap::Parser;
use tracing::info;
use vienna_modloader::{LoadedModInfo, ModLoader};

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

    #[arg(long, default_value = "./mods")]
    mods_dir: PathBuf,
}

#[derive(Clone)]
struct AppState {
    db: PathBuf,
    static_data: PathBuf,
    eventbus: String,
    objectstore: String,
    mods_dir: PathBuf,
    mods: Arc<Vec<LoadedModInfo>>,
    mod_loader: Arc<Mutex<ModLoader>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let args = Args::parse();
    let mods_dir = resolve_workspace_path(&args.mods_dir);
    let mod_loader = ModLoader::load_from_directory(&mods_dir)
        .with_context(|| format!("failed to load mods from {}", mods_dir.display()))?;
    let loaded_mods = Arc::new(mod_loader.infos());
    mod_loader.dispatch_server_start();
    let mod_loader = Arc::new(Mutex::new(mod_loader));

    let state = AppState {
        db: args.db,
        static_data: args.static_data,
        eventbus: args.eventbus,
        objectstore: args.objectstore,
        mods_dir: mods_dir,
        mods: Arc::clone(&loaded_mods),
        mod_loader: Arc::clone(&mod_loader),
    };

    info!(db = %state.db.display(), "configured database");
    info!(static_data = %state.static_data.display(), "configured static data");
    info!(eventbus = %state.eventbus, "configured event bus");
    info!(objectstore = %state.objectstore, "configured object store");
    info!(mods_dir = %state.mods_dir.display(), loaded_mods = loaded_mods.len(), "configured VMA mod loader");

    let app = Router::new()
        .route("/health", get(health))
        .route("/mods", get(list_mods))
        .route("/debug/hooks/player-join/:player_name", get(debug_player_join))
        .route("/debug/hooks/player-leave/:player_name", get(debug_player_leave))
        .route("/debug/hooks/command/:player_name/:command", get(debug_command))
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

    if let Ok(mod_loader) = mod_loader.lock() {
        mod_loader.dispatch_server_stop();
    }

    Ok(())
}

async fn root(State(state): State<AppState>) -> String {
    format!("vienna apiserver rust scaffold with {} VMA mod(s)", state.mods.len())
}

async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "modsDir": state.mods_dir,
        "loadedMods": state.mods.len(),
    }))
}

async fn list_mods(State(state): State<AppState>) -> Json<serde_json::Value> {
    let mods = state
        .mods
        .iter()
        .map(|loaded_mod| {
            serde_json::json!({
                "name": loaded_mod.name,
                "version": loaded_mod.version,
                "path": loaded_mod.path,
            })
        })
        .collect::<Vec<_>>();

    Json(serde_json::json!(mods))
}

async fn debug_player_join(
    State(state): State<AppState>,
    Path(player_name): Path<String>,
) -> Json<serde_json::Value> {
    if let Ok(mod_loader) = state.mod_loader.lock() {
        mod_loader.dispatch_player_join(&player_name);
    }

    Json(serde_json::json!({
        "status": "ok",
        "hook": "player_join",
        "playerName": player_name,
    }))
}

async fn debug_player_leave(
    State(state): State<AppState>,
    Path(player_name): Path<String>,
) -> Json<serde_json::Value> {
    if let Ok(mod_loader) = state.mod_loader.lock() {
        mod_loader.dispatch_player_leave(&player_name);
    }

    Json(serde_json::json!({
        "status": "ok",
        "hook": "player_leave",
        "playerName": player_name,
    }))
}

async fn debug_command(
    State(state): State<AppState>,
    Path((player_name, command)): Path<(String, String)>,
) -> Json<serde_json::Value> {
    if let Ok(mod_loader) = state.mod_loader.lock() {
        mod_loader.dispatch_command(&player_name, &command);
    }

    Json(serde_json::json!({
        "status": "ok",
        "hook": "command",
        "playerName": player_name,
        "command": command,
    }))
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

fn resolve_workspace_path(path: &FsPath) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }

    workspace_root().join(path)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(FsPath::parent)
        .expect("workspace root should exist")
        .to_path_buf()
}
