use std::collections::HashMap;
use std::f64::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

const ACTIVE_TILE_RADIUS: i32 = 3;
const ACTIVE_TILE_EXPIRY_TIME_MS: u64 = 2 * 60 * 1000;
const SPAWN_INTERVAL_MS: u64 = 15 * 1000;
const MIN_TAPPABLE_COUNT: u32 = 1;
const MAX_TAPPABLE_COUNT: u32 = 3;
const MIN_TAPPABLE_DURATION_MS: u64 = 2 * 60 * 1000;
const MAX_TAPPABLE_DURATION_MS: u64 = 5 * 60 * 1000;
const MIN_TAPPABLE_DELAY_MS: u64 = 1 * 60 * 1000;
const MAX_TAPPABLE_DELAY_MS: u64 = 2 * 60 * 1000;
const ENCOUNTER_CHANCE_PER_TILE: u32 = 4;
const MIN_ENCOUNTER_DELAY_MS: u64 = 1 * 60 * 1000;
const MAX_ENCOUNTER_DELAY_MS: u64 = 2 * 60 * 1000;
const GRACE_PERIOD_MS: u64 = 30 * 1000;
const TILE_SCALE: f64 = 65536.0;

#[derive(Debug, Error)]
pub enum TappablesError {
    #[error("failed to read static data from {0}")]
    Read(PathBuf, #[source] std::io::Error),
    #[error("failed to parse JSON from {0}")]
    Parse(PathBuf, #[source] serde_json::Error),
    #[error("missing catalog item rarity for {0}")]
    MissingItem(String),
    #[error("missing item count for {item_id} in config {icon}")]
    MissingItemCount { icon: String, item_id: String },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Tappable {
    pub id: String,
    pub lat: f32,
    pub lon: f32,
    pub spawn_time: u64,
    pub valid_for: u64,
    pub icon: String,
    pub rarity: Rarity,
    pub items: Vec<TappableItem>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct TappableItem {
    pub id: String,
    pub count: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Encounter {
    pub id: String,
    pub lat: f32,
    pub lon: f32,
    pub spawn_time: u64,
    pub valid_for: u64,
    pub icon: String,
    pub rarity: Rarity,
    pub encounter_buildplate_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ActiveTile {
    pub tile_x: i32,
    pub tile_y: i32,
    pub first_active_time: u64,
    pub latest_active_time: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ActiveTileNotification {
    pub x: i32,
    pub y: i32,
    pub player_id: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct SpawnBatch {
    pub tappables: Vec<Tappable>,
    pub encounters: Vec<Encounter>,
}

#[derive(Clone, Debug)]
pub struct StaticData {
    pub catalog: Catalog,
    pub tappables_config: TappablesConfig,
    pub encounters_config: EncountersConfig,
}

impl StaticData {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, TappablesError> {
        let root = root.as_ref();
        let catalog = Catalog::load(root.join("catalog"))?;
        let tappables_config = TappablesConfig::load(root.join("tappables"))?;
        let encounters_config = EncountersConfig::load(root.join("encounters"))?;

        for tappable in &tappables_config.tappables {
            for drop_set in &tappable.drop_sets {
                for item_id in &drop_set.items {
                    if !tappable.item_counts.contains_key(item_id) {
                        return Err(TappablesError::MissingItemCount {
                            icon: tappable.icon.clone(),
                            item_id: item_id.clone(),
                        });
                    }
                }
            }
        }

        Ok(Self {
            catalog,
            tappables_config,
            encounters_config,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Catalog {
    items_by_id: HashMap<String, CatalogItem>,
}

impl Catalog {
    fn load(root: PathBuf) -> Result<Self, TappablesError> {
        let path = root.join("items.json");
        let items: Vec<CatalogItem> = read_json_file(&path)?;
        let items_by_id = items
            .into_iter()
            .map(|item| (item.id.clone(), item))
            .collect::<HashMap<_, _>>();
        Ok(Self { items_by_id })
    }

    pub fn item_rarity(&self, item_id: &str) -> Option<Rarity> {
        self.items_by_id.get(item_id).map(|item| item.rarity)
    }
}

#[derive(Clone, Debug, Deserialize)]
struct CatalogItem {
    id: String,
    rarity: Rarity,
}

#[derive(Clone, Debug)]
pub struct TappablesConfig {
    pub tappables: Vec<TappableConfig>,
}

impl TappablesConfig {
    fn load(root: PathBuf) -> Result<Self, TappablesError> {
        let mut tappables = Vec::new();
        for file in json_files(&root)? {
            tappables.push(read_json_file::<TappableConfig>(&file)?);
        }
        Ok(Self { tappables })
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct TappableConfig {
    pub icon: String,
    #[serde(rename = "dropSets")]
    pub drop_sets: Vec<DropSet>,
    #[serde(rename = "itemCounts")]
    pub item_counts: HashMap<String, ItemCount>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DropSet {
    pub items: Vec<String>,
    pub chance: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ItemCount {
    pub min: u32,
    pub max: u32,
}

#[derive(Clone, Debug)]
pub struct EncountersConfig {
    pub encounters: Vec<EncounterConfig>,
}

impl EncountersConfig {
    fn load(root: PathBuf) -> Result<Self, TappablesError> {
        let mut encounters = Vec::new();
        for file in json_files(&root)? {
            encounters.push(read_json_file::<EncounterConfig>(&file)?);
        }
        Ok(Self { encounters })
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct EncounterConfig {
    pub icon: String,
    pub rarity: Rarity,
    #[serde(rename = "encounterBuildplateId")]
    pub encounter_buildplate_id: String,
    pub duration: u64,
}

pub struct TappableGenerator {
    catalog: Catalog,
    tappables_config: TappablesConfig,
    rng: StdRng,
}

impl TappableGenerator {
    pub fn new(static_data: &StaticData, seed: u64) -> Self {
        Self {
            catalog: static_data.catalog.clone(),
            tappables_config: static_data.tappables_config.clone(),
            rng: StdRng::seed_from_u64(seed),
        }
    }

    pub fn max_tappable_lifetime(&self) -> u64 {
        MAX_TAPPABLE_DELAY_MS + MAX_TAPPABLE_DURATION_MS + GRACE_PERIOD_MS
    }

    pub fn generate_tappables(
        &mut self,
        tile_x: i32,
        tile_y: i32,
        current_time: u64,
    ) -> Result<Vec<Tappable>, TappablesError> {
        if self.tappables_config.tappables.is_empty() {
            return Ok(Vec::new());
        }

        let mut tappables = Vec::new();
        let count = self.rng.gen_range(MIN_TAPPABLE_COUNT..=MAX_TAPPABLE_COUNT);
        for _ in 0..count {
            let spawn_delay = self.rng.gen_range(MIN_TAPPABLE_DELAY_MS..=MAX_TAPPABLE_DELAY_MS);
            let duration = self
                .rng
                .gen_range(MIN_TAPPABLE_DURATION_MS..=MAX_TAPPABLE_DURATION_MS);
            let config_index = self.rng.gen_range(0..self.tappables_config.tappables.len());
            let tappable_config = &self.tappables_config.tappables[config_index];

            let (lat, lon) = random_tile_location(&mut self.rng, tile_x, tile_y);
            let drop_set = choose_drop_set(&mut self.rng, &tappable_config.drop_sets);

            let mut items = Vec::new();
            for item_id in &drop_set.items {
                let item_count = tappable_config.item_counts.get(item_id).ok_or_else(|| {
                    TappablesError::MissingItemCount {
                        icon: tappable_config.icon.clone(),
                        item_id: item_id.clone(),
                    }
                })?;

                items.push(TappableItem {
                    id: item_id.clone(),
                    count: self.rng.gen_range(item_count.min..=item_count.max),
                });
            }

            let rarity = items
                .iter()
                .map(|item| {
                    self.catalog
                        .item_rarity(&item.id)
                        .ok_or_else(|| TappablesError::MissingItem(item.id.clone()))
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .max()
                .unwrap_or(Rarity::Common);

            tappables.push(Tappable {
                id: Uuid::new_v4().to_string(),
                lat,
                lon,
                spawn_time: current_time + spawn_delay,
                valid_for: duration,
                icon: tappable_config.icon.clone(),
                rarity,
                items,
            });
        }

        Ok(tappables)
    }
}

pub struct EncounterGenerator {
    encounters_config: EncountersConfig,
    max_duration_ms: u64,
    rng: StdRng,
}

impl EncounterGenerator {
    pub fn new(static_data: &StaticData, seed: u64) -> Self {
        let max_duration_ms = static_data
            .encounters_config
            .encounters
            .iter()
            .map(|encounter| encounter.duration * 1000)
            .max()
            .unwrap_or(0);

        Self {
            encounters_config: static_data.encounters_config.clone(),
            max_duration_ms,
            rng: StdRng::seed_from_u64(seed),
        }
    }

    pub fn max_encounter_lifetime(&self) -> u64 {
        MAX_ENCOUNTER_DELAY_MS + self.max_duration_ms + GRACE_PERIOD_MS
    }

    pub fn generate_encounters(
        &mut self,
        tile_x: i32,
        tile_y: i32,
        current_time: u64,
    ) -> Vec<Encounter> {
        if self.encounters_config.encounters.is_empty() {
            return Vec::new();
        }

        if self.rng.gen_range(0..ENCOUNTER_CHANCE_PER_TILE) != 0 {
            return Vec::new();
        }

        let spawn_delay = self
            .rng
            .gen_range(MIN_ENCOUNTER_DELAY_MS..=MAX_ENCOUNTER_DELAY_MS);
        let config_index = self.rng.gen_range(0..self.encounters_config.encounters.len());
        let config = &self.encounters_config.encounters[config_index];
        let (lat, lon) = random_tile_location(&mut self.rng, tile_x, tile_y);

        vec![Encounter {
            id: Uuid::new_v4().to_string(),
            lat,
            lon,
            spawn_time: current_time + spawn_delay,
            valid_for: config.duration * 1000,
            icon: config.icon.clone(),
            rarity: config.rarity,
            encounter_buildplate_id: config.encounter_buildplate_id.clone(),
        }]
    }
}

#[derive(Default)]
pub struct ActiveTiles {
    active_tiles: HashMap<i64, ActiveTile>,
}

impl ActiveTiles {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_active_tile(
        &mut self,
        notification: ActiveTileNotification,
        current_time: u64,
    ) -> ActiveTileUpdate {
        let inactive = self.prune_active_tiles(current_time);
        let mut active = Vec::new();

        for tile_x in (notification.x - ACTIVE_TILE_RADIUS)..=(notification.x + ACTIVE_TILE_RADIUS) {
            for tile_y in (notification.y - ACTIVE_TILE_RADIUS)..=(notification.y + ACTIVE_TILE_RADIUS) {
                let tile = self.mark_tile_active(tile_x, tile_y, current_time);
                if tile.first_active_time == tile.latest_active_time {
                    active.push(tile);
                }
            }
        }

        ActiveTileUpdate { active, inactive }
    }

    pub fn get_active_tiles(&self, current_time: u64) -> Vec<ActiveTile> {
        self.active_tiles
            .values()
            .filter(|tile| current_time < tile.latest_active_time + ACTIVE_TILE_EXPIRY_TIME_MS)
            .cloned()
            .collect()
    }

    fn mark_tile_active(&mut self, tile_x: i32, tile_y: i32, current_time: u64) -> ActiveTile {
        let key = tile_key(tile_x, tile_y);
        let tile = match self.active_tiles.get(&key) {
            Some(existing) => ActiveTile {
                tile_x,
                tile_y,
                first_active_time: existing.first_active_time,
                latest_active_time: current_time,
            },
            None => ActiveTile {
                tile_x,
                tile_y,
                first_active_time: current_time,
                latest_active_time: current_time,
            },
        };
        self.active_tiles.insert(key, tile.clone());
        tile
    }

    fn prune_active_tiles(&mut self, current_time: u64) -> Vec<ActiveTile> {
        let mut inactive = Vec::new();
        self.active_tiles.retain(|_, tile| {
            let keep = tile.latest_active_time + ACTIVE_TILE_EXPIRY_TIME_MS > current_time;
            if !keep {
                inactive.push(tile.clone());
            }
            keep
        });
        inactive
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActiveTileUpdate {
    pub active: Vec<ActiveTile>,
    pub inactive: Vec<ActiveTile>,
}

pub struct Spawner {
    tappable_generator: TappableGenerator,
    encounter_generator: EncounterGenerator,
    max_tappable_lifetime_intervals: u32,
    spawn_cycle_time: u64,
    spawn_cycle_index: u32,
    last_spawn_cycle_for_tile: HashMap<i64, u32>,
}

impl Spawner {
    pub fn new(static_data: &StaticData, seed: u64, now: u64) -> Self {
        let tappable_generator = TappableGenerator::new(static_data, seed);
        let encounter_generator = EncounterGenerator::new(static_data, seed ^ 0x5f5f5f5f_u64);
        let max_lifetime = tappable_generator
            .max_tappable_lifetime()
            .max(encounter_generator.max_encounter_lifetime());
        let max_tappable_lifetime_intervals = (max_lifetime / SPAWN_INTERVAL_MS + 1) as u32;

        Self {
            tappable_generator,
            encounter_generator,
            max_tappable_lifetime_intervals,
            spawn_cycle_time: now,
            spawn_cycle_index: max_tappable_lifetime_intervals,
            last_spawn_cycle_for_tile: HashMap::new(),
        }
    }

    pub fn spawn_tiles(
        &mut self,
        active_tiles: &[ActiveTile],
        now: u64,
    ) -> Result<SpawnBatch, TappablesError> {
        let (spawn_cycle_time, spawn_cycle_index) =
            advance_spawn_clock(self.spawn_cycle_time, self.spawn_cycle_index, now);

        let mut batch = SpawnBatch::default();
        for active_tile in active_tiles {
            self.do_spawn_cycles_for_tile(
                active_tile.tile_x,
                active_tile.tile_y,
                spawn_cycle_time,
                spawn_cycle_index,
                &mut batch,
            )?;
        }

        let cutoff_time = spawn_cycle_time.saturating_sub(SPAWN_INTERVAL_MS);
        retain_valid_spawns(&mut batch, cutoff_time);
        Ok(batch)
    }

    pub fn spawn_cycle(
        &mut self,
        active_tiles: &[ActiveTile],
        now: u64,
    ) -> Result<SpawnBatch, TappablesError> {
        let current_active_tiles = active_tiles.to_vec();
        let (spawn_cycle_time, spawn_cycle_index) =
            advance_spawn_clock(self.spawn_cycle_time, self.spawn_cycle_index, now);
        self.spawn_cycle_time = spawn_cycle_time;
        self.spawn_cycle_index = spawn_cycle_index;

        let mut batch = SpawnBatch::default();
        for active_tile in current_active_tiles {
            self.do_spawn_cycles_for_tile(
                active_tile.tile_x,
                active_tile.tile_y,
                self.spawn_cycle_time,
                self.spawn_cycle_index,
                &mut batch,
            )?;
        }

        let cutoff_time = self.spawn_cycle_time.saturating_sub(SPAWN_INTERVAL_MS);
        retain_valid_spawns(&mut batch, cutoff_time);
        Ok(batch)
    }

    fn do_spawn_cycles_for_tile(
        &mut self,
        tile_x: i32,
        tile_y: i32,
        spawn_cycle_time: u64,
        spawn_cycle_index: u32,
        batch: &mut SpawnBatch,
    ) -> Result<(), TappablesError> {
        let key = tile_key(tile_x, tile_y);
        let last_spawn_cycle = *self.last_spawn_cycle_for_tile.get(&key).unwrap_or(&0);
        let cycles_to_spawn =
            (spawn_cycle_index - last_spawn_cycle).min(self.max_tappable_lifetime_intervals);

        for index in 0..cycles_to_spawn {
            let cycle_time =
                spawn_cycle_time - SPAWN_INTERVAL_MS * u64::from(cycles_to_spawn - index - 1);
            batch
                .tappables
                .extend(self.tappable_generator.generate_tappables(tile_x, tile_y, cycle_time)?);
            batch
                .encounters
                .extend(self.encounter_generator.generate_encounters(tile_x, tile_y, cycle_time));
        }

        self.last_spawn_cycle_for_tile.insert(key, spawn_cycle_index);
        Ok(())
    }
}

#[derive(Default)]
pub struct TappablesManager {
    tappables: HashMap<String, HashMap<String, Tappable>>,
    encounters: HashMap<String, HashMap<String, Encounter>>,
}

impl TappablesManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_spawn_batch(&mut self, batch: SpawnBatch) {
        for tappable in batch.tappables {
            self.add_tappable(tappable);
        }
        for encounter in batch.encounters {
            self.add_encounter(encounter);
        }
    }

    pub fn get_tappables_around(&self, lat: f32, lon: f32, radius: f32) -> Vec<Tappable> {
        get_tile_ids_around(lat, lon, radius)
            .into_iter()
            .filter_map(|tile_id| self.tappables.get(&tile_id))
            .flat_map(|tappables| tappables.values())
            .filter(|tappable| within_radius(tappable.lat, tappable.lon, lat, lon, radius))
            .cloned()
            .collect()
    }

    pub fn get_encounters_around(&self, lat: f32, lon: f32, radius: f32) -> Vec<Encounter> {
        get_tile_ids_around(lat, lon, radius)
            .into_iter()
            .filter_map(|tile_id| self.encounters.get(&tile_id))
            .flat_map(|encounters| encounters.values())
            .filter(|encounter| within_radius(encounter.lat, encounter.lon, lat, lon, radius))
            .cloned()
            .collect()
    }

    pub fn get_tappable(&self, id: &str, tile_id: &str) -> Option<&Tappable> {
        self.tappables.get(tile_id).and_then(|tile| tile.get(id))
    }

    pub fn get_encounter(&self, id: &str, tile_id: &str) -> Option<&Encounter> {
        self.encounters.get(tile_id).and_then(|tile| tile.get(id))
    }

    pub fn is_tappable_valid_for(
        &self,
        tappable: &Tappable,
        request_time: u64,
        _lat: f32,
        _lon: f32,
    ) -> bool {
        !(tappable.spawn_time.saturating_sub(GRACE_PERIOD_MS) > request_time
            || tappable.spawn_time + tappable.valid_for + GRACE_PERIOD_MS <= request_time)
    }

    pub fn is_encounter_valid_for(
        &self,
        encounter: &Encounter,
        request_time: u64,
        _lat: f32,
        _lon: f32,
    ) -> bool {
        !(encounter.spawn_time.saturating_sub(GRACE_PERIOD_MS) > request_time
            || encounter.spawn_time + encounter.valid_for <= request_time)
    }

    pub fn prune(&mut self, current_time: u64) {
        self.tappables.values_mut().for_each(|tile_tappables| {
            tile_tappables.retain(|_, tappable| {
                tappable.spawn_time + tappable.valid_for + GRACE_PERIOD_MS > current_time
            })
        });
        self.tappables.retain(|_, tile| !tile.is_empty());

        self.encounters.values_mut().for_each(|tile_encounters| {
            tile_encounters.retain(|_, encounter| {
                encounter.spawn_time + encounter.valid_for + GRACE_PERIOD_MS > current_time
            })
        });
        self.encounters.retain(|_, tile| !tile.is_empty());
    }

    fn add_tappable(&mut self, tappable: Tappable) {
        let tile_id = location_to_tile_id(tappable.lat, tappable.lon);
        self.tappables
            .entry(tile_id)
            .or_default()
            .insert(tappable.id.clone(), tappable);
    }

    fn add_encounter(&mut self, encounter: Encounter) {
        let tile_id = location_to_tile_id(encounter.lat, encounter.lon);
        self.encounters
            .entry(tile_id)
            .or_default()
            .insert(encounter.id.clone(), encounter);
    }
}

pub fn active_tile_notification_from_location(
    player_id: impl Into<String>,
    lat: f32,
    lon: f32,
) -> ActiveTileNotification {
    ActiveTileNotification {
        x: x_to_tile(lon_to_x(lon)),
        y: y_to_tile(lat_to_y(lat)),
        player_id: player_id.into(),
    }
}

pub fn location_to_tile_id(lat: f32, lon: f32) -> String {
    format!("{}_{}", x_to_tile(lon_to_x(lon)), y_to_tile(lat_to_y(lat)))
}

fn get_tile_ids_around(lat: f32, lon: f32, radius: f32) -> Vec<String> {
    let tile_x = x_to_tile(lon_to_x(lon));
    let tile_y = y_to_tile(lat_to_y(lat));
    let tile_radius = radius.ceil() as i32;

    let mut tile_ids = Vec::new();
    for x in (tile_x - tile_radius)..=(tile_x + tile_radius) {
        for y in (tile_y - tile_radius)..=(tile_y + tile_radius) {
            tile_ids.push(format!("{x}_{y}"));
        }
    }
    tile_ids
}

fn choose_drop_set<'a>(rng: &mut StdRng, drop_sets: &'a [DropSet]) -> &'a DropSet {
    let total: u32 = drop_sets.iter().map(|drop_set| drop_set.chance).sum();
    let mut roll = rng.gen_range(0..total.max(1));
    for drop_set in drop_sets {
        if roll < drop_set.chance {
            return drop_set;
        }
        roll = roll.saturating_sub(drop_set.chance);
    }
    &drop_sets[0]
}

fn retain_valid_spawns(batch: &mut SpawnBatch, cutoff_time: u64) {
    batch
        .tappables
        .retain(|tappable| tappable.spawn_time + tappable.valid_for >= cutoff_time);
    batch
        .encounters
        .retain(|encounter| encounter.spawn_time + encounter.valid_for >= cutoff_time);
}

fn advance_spawn_clock(spawn_cycle_time: u64, spawn_cycle_index: u32, now: u64) -> (u64, u32) {
    let mut spawn_cycle_time = spawn_cycle_time;
    let mut spawn_cycle_index = spawn_cycle_index;
    while spawn_cycle_time < now {
        spawn_cycle_time += SPAWN_INTERVAL_MS;
        spawn_cycle_index += 1;
    }
    (spawn_cycle_time, spawn_cycle_index)
}

fn random_tile_location(rng: &mut StdRng, tile_x: i32, tile_y: i32) -> (f32, f32) {
    let bounds = tile_bounds(tile_x, tile_y);
    let lat = rng.gen_range(bounds.1..=bounds.0);
    let lon = rng.gen_range(bounds.2..=bounds.3);
    (lat, lon)
}

fn tile_bounds(tile_x: i32, tile_y: i32) -> (f32, f32, f32, f32) {
    (
        y_to_lat(tile_y as f64 / TILE_SCALE),
        y_to_lat((tile_y + 1) as f64 / TILE_SCALE),
        x_to_lon(tile_x as f64 / TILE_SCALE),
        x_to_lon((tile_x + 1) as f64 / TILE_SCALE),
    )
}

fn x_to_lon(x: f64) -> f32 {
    (((x * 2.0 - 1.0) * PI).to_degrees()) as f32
}

fn y_to_lat(y: f64) -> f32 {
    (f64::atan(f64::sinh((1.0 - y * 2.0) * PI)).to_degrees()) as f32
}

fn lon_to_x(lon: f32) -> f32 {
    ((1.0 + f64::to_radians(lon as f64) / PI) / 2.0) as f32
}

fn lat_to_y(lat: f32) -> f32 {
    let lat_radians = f64::to_radians(lat as f64);
    ((1.0 - ((f64::ln(f64::tan(lat_radians) + 1.0 / f64::cos(lat_radians))) / PI)) / 2.0)
        as f32
}

fn x_to_tile(x: f32) -> i32 {
    (x as f64 * TILE_SCALE).floor() as i32
}

fn y_to_tile(y: f32) -> i32 {
    (y as f64 * TILE_SCALE).floor() as i32
}

fn within_radius(target_lat: f32, target_lon: f32, lat: f32, lon: f32, radius: f32) -> bool {
    let dx = lon_to_x(target_lon) * TILE_SCALE as f32 - lon_to_x(lon) * TILE_SCALE as f32;
    let dy = lat_to_y(target_lat) * TILE_SCALE as f32 - lat_to_y(lat) * TILE_SCALE as f32;
    dx * dx + dy * dy <= radius * radius
}

fn tile_key(tile_x: i32, tile_y: i32) -> i64 {
    ((tile_x as i64) << 32) | (tile_y as u32 as i64)
}

fn json_files(root: &Path) -> Result<Vec<PathBuf>, TappablesError> {
    let mut files = fs::read_dir(root)
        .map_err(|error| TappablesError::Read(root.to_path_buf(), error))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn read_json_file<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, TappablesError> {
    let data = fs::read_to_string(path).map_err(|error| TappablesError::Read(path.to_path_buf(), error))?;
    serde_json::from_str(&data).map_err(|error| TappablesError::Parse(path.to_path_buf(), error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::create_dir_all;
    use tempfile::tempdir;

    #[test]
    fn generates_tappables_and_manages_tiles() {
        let temp_dir = tempdir().expect("tempdir should exist");
        let root = temp_dir.path();
        create_dir_all(root.join("catalog")).expect("catalog dir should exist");
        create_dir_all(root.join("tappables")).expect("tappables dir should exist");
        create_dir_all(root.join("encounters")).expect("encounters dir should exist");

        fs::write(
            root.join("catalog").join("items.json"),
            r#"[{"id":"stone","rarity":"COMMON"},{"id":"diamond","rarity":"EPIC"}]"#,
        )
        .expect("items.json should be written");
        fs::write(
            root.join("tappables").join("basic.json"),
            r#"{
                "icon": "stone_icon",
                "dropSets": [{"items": ["stone", "diamond"], "chance": 1}],
                "itemCounts": {
                    "stone": {"min": 1, "max": 1},
                    "diamond": {"min": 1, "max": 1}
                }
            }"#,
        )
        .expect("tappable config should be written");
        fs::write(
            root.join("encounters").join("wolf.json"),
            r#"{
                "icon": "wolf_icon",
                "rarity": "RARE",
                "encounterBuildplateId": "bp-123",
                "duration": 120
            }"#,
        )
        .expect("encounter config should be written");

        let static_data = StaticData::load(root).expect("static data should load");
        let mut spawner = Spawner::new(&static_data, 42, 1_000_000);
        let mut active_tiles = ActiveTiles::new();
        let update = active_tiles.record_active_tile(
            ActiveTileNotification {
                x: 100,
                y: 200,
                player_id: "player-1".to_owned(),
            },
            1_000_000,
        );

        assert!(!update.active.is_empty());

        let batch = spawner
            .spawn_tiles(&update.active, 1_000_000)
            .expect("spawn should succeed");
        assert!(!batch.tappables.is_empty());

        let mut manager = TappablesManager::new();
        manager.add_spawn_batch(batch.clone());

        let tappable = batch.tappables.first().expect("tappable should exist");
        let found = manager.get_tappables_around(tappable.lat, tappable.lon, 5.0);
        assert!(!found.is_empty());
        assert!(manager.is_tappable_valid_for(
            tappable,
            tappable.spawn_time,
            tappable.lat,
            tappable.lon
        ));
    }

    #[test]
    fn active_tiles_expire() {
        let mut active_tiles = ActiveTiles::new();
        active_tiles.record_active_tile(
            ActiveTileNotification {
                x: 1,
                y: 1,
                player_id: "player-1".to_owned(),
            },
            1_000,
        );

        let active = active_tiles.get_active_tiles(1_000 + ACTIVE_TILE_EXPIRY_TIME_MS - 1);
        assert!(!active.is_empty());

        let update = active_tiles.record_active_tile(
            ActiveTileNotification {
                x: 10,
                y: 10,
                player_id: "player-2".to_owned(),
            },
            1_000 + ACTIVE_TILE_EXPIRY_TIME_MS + 1,
        );
        assert!(!update.inactive.is_empty());
    }
}
