use std::collections::BTreeMap;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post, put},
    Json, Router,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use vienna_db::EarthDb;
use vienna_modloader::{LoadedModInfo, ModLoader};
use vienna_staticdata::StaticData as ViennaStaticData;
use vienna_tappables::{
    active_tile_notification_from_location, ActiveTiles, Spawner, StaticData as TappablesStaticData,
    TappablesManager,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_LOG_TAIL_BYTES: u64 = 64 * 1024;

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

    #[arg(long = "logs-dir", default_value = "./logs")]
    logs_dir: PathBuf,

    #[arg(long = "log-secret")]
    log_secret: Option<String>,

    #[arg(long = "custom-login-only", default_value_t = false)]
    custom_login_only: bool,

    #[arg(long = "max-tile-cache-size", default_value_t = 2048)]
    max_tile_cache_size: usize,
}

#[derive(Clone)]
struct AppState {
    db_path: PathBuf,
    db: EarthDb,
    static_data_path: PathBuf,
    static_data: Arc<Option<ViennaStaticData>>,
    tappables_static_data: Arc<Option<TappablesStaticData>>,
    eventbus: String,
    objectstore: String,
    mods_dir: PathBuf,
    logs_dir: PathBuf,
    log_secret: Option<String>,
    custom_login_only: bool,
    max_tile_cache_size: usize,
    mods: Arc<Vec<LoadedModInfo>>,
    mod_loader: Arc<Mutex<ModLoader>>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct PlayerInventory {
    items: BTreeMap<String, u32>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct PlayerRoles {
    roles: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct InventoryDelta {
    item_id: String,
    count: i32,
}

#[derive(Debug, Deserialize)]
struct GenerateTappablesRequest {
    player_id: String,
    lat: f32,
    lon: f32,
    #[serde(default = "now_ms")]
    now: u64,
    #[serde(default = "default_radius")]
    radius: f32,
    #[serde(default)]
    seed: u64,
}

#[derive(Debug, Deserialize)]
struct ImportDataRequest {
    #[serde(default)]
    merge: bool,
    records: Vec<ImportRecord>,
}

#[derive(Debug, Deserialize)]
struct ImportRecord {
    #[serde(rename = "type")]
    object_type: String,
    id: String,
    value: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct LogFileInfo {
    name: String,
    bytes: u64,
    modified_ms: Option<u64>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let args = Args::parse();
    let db_path = resolve_workspace_path(&args.db);
    let static_data_path = resolve_workspace_path(&args.static_data);
    let mods_dir = resolve_workspace_path(&args.mods_dir);
    let logs_dir = resolve_workspace_path(&args.logs_dir);

    let db = EarthDb::open(&db_path).with_context(|| format!("failed to open {}", db_path.display()))?;
    fs::create_dir_all(&logs_dir).with_context(|| format!("failed to create {}", logs_dir.display()))?;

    let static_data = match ViennaStaticData::load(&static_data_path) {
        Ok(data) => Some(data),
        Err(error) => {
            warn!(static_data = %static_data_path.display(), %error, "static data is not available");
            None
        }
    };
    let tappables_static_data = match TappablesStaticData::load(&static_data_path) {
        Ok(data) => Some(data),
        Err(error) => {
            warn!(static_data = %static_data_path.display(), %error, "tappables static data is not available");
            None
        }
    };

    let mod_loader = ModLoader::load_from_directory(&mods_dir)
        .with_context(|| format!("failed to load mods from {}", mods_dir.display()))?;
    let loaded_mods = Arc::new(mod_loader.infos());
    mod_loader.dispatch_server_start();
    let mod_loader = Arc::new(Mutex::new(mod_loader));

    let state = AppState {
        db_path,
        db,
        static_data_path,
        static_data: Arc::new(static_data),
        tappables_static_data: Arc::new(tappables_static_data),
        eventbus: args.eventbus,
        objectstore: args.objectstore,
        mods_dir,
        logs_dir,
        log_secret: args.log_secret,
        custom_login_only: args.custom_login_only,
        max_tile_cache_size: args.max_tile_cache_size,
        mods: Arc::clone(&loaded_mods),
        mod_loader: Arc::clone(&mod_loader),
    };

    info!(version = VERSION, "configured version");
    info!(db = %state.db_path.display(), "configured database");
    info!(static_data = %state.static_data_path.display(), loaded = state.static_data.is_some(), "configured static data");
    info!(eventbus = %state.eventbus, "configured event bus");
    info!(objectstore = %state.objectstore, "configured object store");
    info!(mods_dir = %state.mods_dir.display(), loaded_mods = loaded_mods.len(), "configured VMA mod loader");
    info!(logs_dir = %state.logs_dir.display(), "configured logs");

    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/version", get(version))
        .route("/auth/config", get(auth_config))
        .route("/mods", get(list_mods))
        .route("/static/summary", get(static_summary))
        .route("/shop/catalog", get(shop_catalog))
        .route("/levels/:level/rewards", get(level_rewards))
        .route("/players/:player_id/items", get(get_player_items))
        .route("/players/:player_id/items", post(add_player_item))
        .route("/players/:player_id/roles", get(get_player_roles))
        .route("/players/:player_id/roles", put(set_player_roles))
        .route("/data/import", post(import_data))
        .route("/tappables/generate", post(generate_tappables))
        .route("/logs", get(list_logs))
        .route("/logs/:name", get(read_log))
        .route("/logs", delete(clear_logs))
        .route("/debug/hooks/player-join/:player_name", get(debug_player_join))
        .route("/debug/hooks/player-leave/:player_name", get(debug_player_leave))
        .route("/debug/hooks/command/:player_name/:command", get(debug_command))
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
    format!("vienna apiserver {VERSION} with {} VMA mod(s)", state.mods.len())
}

async fn version() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "version": VERSION }))
}

async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": VERSION,
        "db": state.db_path,
        "staticData": state.static_data_path,
        "staticDataLoaded": state.static_data.is_some(),
        "tappablesLoaded": state.tappables_static_data.is_some(),
        "modsDir": state.mods_dir,
        "loadedMods": state.mods.len(),
        "customLoginOnly": state.custom_login_only,
        "maxTileCacheSize": state.max_tile_cache_size,
    }))
}

async fn auth_config(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "customLoginOnly": state.custom_login_only,
        "microsoftAccountVerification": !state.custom_login_only,
    }))
}

async fn static_summary(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let static_data = state.static_data.as_ref().as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    Json(serde_json::json!({
        "items": static_data.catalog.items().count(),
        "levels": static_data.levels.levels.len(),
        "tappables": static_data.tappables_config.tappables.len(),
        "encounters": static_data.encounters_config.encounters.len(),
    }))
    .pipe(Ok)
}

async fn shop_catalog(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let static_data = state.static_data.as_ref().as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let mut items = static_data
        .catalog
        .items()
        .map(|item| serde_json::json!({
            "id": item.id,
            "rarity": item.rarity,
            "experience": item.experience,
        }))
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    Ok(Json(serde_json::json!({ "items": items })))
}

async fn level_rewards(
    State(state): State<AppState>,
    Path(level): Path<u32>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let static_data = state.static_data.as_ref().as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let reward = static_data
        .levels
        .rewards_for_level(level)
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(serde_json::json!({ "level": level, "reward": reward })))
}

async fn get_player_items(
    State(state): State<AppState>,
    Path(player_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let record = state
        .db
        .get::<PlayerInventory>("player_items", &player_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "playerId": player_id, "version": record.version, "items": record.value.items })))
}

async fn add_player_item(
    State(state): State<AppState>,
    Path(player_id): Path<String>,
    Json(delta): Json<InventoryDelta>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut record = state
        .db
        .get::<PlayerInventory>("player_items", &player_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let current = record.value.items.get(&delta.item_id).copied().unwrap_or_default() as i64;
    let updated = (current + i64::from(delta.count)).max(0) as u32;
    if updated == 0 {
        record.value.items.remove(&delta.item_id);
    } else {
        record.value.items.insert(delta.item_id, updated);
    }
    let version = state
        .db
        .update("player_items", &player_id, &record.value)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "playerId": player_id, "version": version, "items": record.value.items })))
}

async fn get_player_roles(
    State(state): State<AppState>,
    Path(player_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let record = state
        .db
        .get::<PlayerRoles>("player_roles", &player_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "playerId": player_id, "version": record.version, "roles": record.value.roles })))
}

async fn set_player_roles(
    State(state): State<AppState>,
    Path(player_id): Path<String>,
    Json(mut roles): Json<PlayerRoles>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    roles.roles.sort();
    roles.roles.dedup();
    let version = state
        .db
        .update("player_roles", &player_id, &roles)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "playerId": player_id, "version": version, "roles": roles.roles })))
}

async fn import_data(
    State(state): State<AppState>,
    Json(request): Json<ImportDataRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut imported = 0usize;
    let mut skipped = Vec::new();
    let mut warnings = Vec::new();

    for record in request.records {
        if record.object_type.trim().is_empty() || record.id.trim().is_empty() {
            warnings.push(serde_json::json!({
                "type": record.object_type,
                "id": record.id,
                "warning": "empty type or id",
            }));
            continue;
        }

        let existing = state
            .db
            .get::<serde_json::Value>(&record.object_type, &record.id)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if existing.version > 1 && !request.merge {
            skipped.push(serde_json::json!({
                "type": record.object_type,
                "id": record.id,
                "version": existing.version,
                "reason": "already exists",
            }));
            continue;
        }

        if existing.version > 1 {
            warnings.push(serde_json::json!({
                "type": record.object_type,
                "id": record.id,
                "warning": "merged over existing value",
                "previousVersion": existing.version,
            }));
        }

        state
            .db
            .update(&record.object_type, &record.id, &record.value)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        imported += 1;
    }

    Ok(Json(serde_json::json!({
        "imported": imported,
        "skipped": skipped,
        "warnings": warnings,
    })))
}

async fn generate_tappables(
    State(state): State<AppState>,
    Json(request): Json<GenerateTappablesRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let static_data = state
        .tappables_static_data
        .as_ref()
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let notification =
        active_tile_notification_from_location(request.player_id, request.lat, request.lon);
    let mut active_tiles = ActiveTiles::new();
    let update = active_tiles.record_active_tile(notification, request.now);
    let mut spawner = Spawner::new(static_data, request.seed, request.now);
    let batch = spawner
        .spawn_tiles(&update.active, request.now)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut manager = TappablesManager::new();
    manager.add_spawn_batch(batch);
    Ok(Json(serde_json::json!({
        "tappables": manager.get_tappables_around(request.lat, request.lon, request.radius),
        "encounters": manager.get_encounters_around(request.lat, request.lon, request.radius),
        "activeTiles": update.active,
        "inactiveTiles": update.inactive,
    })))
}

async fn list_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_log_secret(&state, &headers)?;
    let mut files = Vec::new();
    for entry in fs::read_dir(&state.logs_dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? {
        let entry = entry.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let metadata = entry.metadata().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if metadata.is_file() {
            files.push(LogFileInfo {
                name: entry.file_name().to_string_lossy().into_owned(),
                bytes: metadata.len(),
                modified_ms: metadata.modified().ok().and_then(system_time_to_ms),
            });
        }
    }
    files.sort_by(|left, right| right.modified_ms.cmp(&left.modified_ms));
    Ok(Json(serde_json::json!({ "logs": files })))
}

async fn read_log(
    State(state): State<AppState>,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Result<String, StatusCode> {
    require_log_secret(&state, &headers)?;
    let path = safe_child_path(&state.logs_dir, &name).ok_or(StatusCode::BAD_REQUEST)?;
    let metadata = fs::metadata(&path).map_err(|_| StatusCode::NOT_FOUND)?;
    let start = metadata.len().saturating_sub(DEFAULT_LOG_TAIL_BYTES);
    let bytes = fs::read(&path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(String::from_utf8_lossy(&bytes[start as usize..]).into_owned())
}

async fn clear_logs(State(state): State<AppState>, headers: HeaderMap) -> Result<StatusCode, StatusCode> {
    require_log_secret(&state, &headers)?;
    for entry in fs::read_dir(&state.logs_dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? {
        let entry = entry.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if entry.metadata().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.is_file() {
            fs::remove_file(entry.path()).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }
    Ok(StatusCode::NO_CONTENT)
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

fn require_log_secret(state: &AppState, headers: &HeaderMap) -> Result<(), StatusCode> {
    let Some(secret) = &state.log_secret else {
        return Ok(());
    };
    let supplied = headers
        .get("x-vienna-log-secret")
        .and_then(|value| value.to_str().ok());
    if supplied == Some(secret.as_str()) {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn safe_child_path(root: &FsPath, name: &str) -> Option<PathBuf> {
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return None;
    }
    Some(root.join(name))
}

fn now_ms() -> u64 {
    system_time_to_ms(SystemTime::now()).unwrap_or_default()
}

fn default_radius() -> f32 {
    5.0
}

fn system_time_to_ms(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}

impl<T> Pipe for T {}

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
