use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StaticDataError {
    #[error("failed to read static data from {0}")]
    Read(PathBuf, #[source] std::io::Error),
    #[error("failed to parse JSON from {0}")]
    Parse(PathBuf, #[source] serde_json::Error),
    #[error("missing item count for {item_id} in config {icon}")]
    MissingItemCount { icon: String, item_id: String },
    #[error("levels are not strictly increasing")]
    InvalidLevels,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    Oobe,
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug)]
pub struct StaticData {
    pub catalog: Catalog,
    pub levels: Levels,
    pub tappables_config: TappablesConfig,
    pub encounters_config: EncountersConfig,
}

impl StaticData {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, StaticDataError> {
        let root = root.as_ref();
        let catalog = Catalog::load(root.join("catalog"))?;
        let levels = Levels::load(root.join("levels"))?;
        let tappables_config = TappablesConfig::load(root.join("tappables"))?;
        let encounters_config = EncountersConfig::load(root.join("encounters"))?;

        for tappable in &tappables_config.tappables {
            for drop_set in &tappable.drop_sets {
                for item_id in &drop_set.items {
                    if !tappable.item_counts.contains_key(item_id) {
                        return Err(StaticDataError::MissingItemCount {
                            icon: tappable.icon.clone(),
                            item_id: item_id.clone(),
                        });
                    }
                }
            }
        }

        Ok(Self {
            catalog,
            levels,
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
    fn load(root: PathBuf) -> Result<Self, StaticDataError> {
        let path = root.join("items.json");
        let items: Vec<CatalogItem> = read_json_file(&path)?;
        let items_by_id = items
            .into_iter()
            .map(|item| (item.id.clone(), item))
            .collect::<HashMap<_, _>>();
        Ok(Self { items_by_id })
    }

    pub fn get_item(&self, item_id: &str) -> Option<&CatalogItem> {
        self.items_by_id.get(item_id)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CatalogItem {
    pub id: String,
    pub rarity: Rarity,
    #[serde(default)]
    pub experience: Experience,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Experience {
    #[serde(default)]
    pub tappable: u32,
    #[serde(default)]
    pub encounter: u32,
    #[serde(default)]
    pub crafting: u32,
    #[serde(default)]
    pub journal: u32,
}

#[derive(Clone, Debug)]
pub struct Levels {
    pub levels: Vec<Level>,
}

impl Levels {
    fn load(root: PathBuf) -> Result<Self, StaticDataError> {
        let mut levels = Vec::new();
        let mut level_number = 2;
        loop {
            let path = root.join(format!("{level_number}.json"));
            if !path.is_file() {
                break;
            }

            levels.push(read_json_file::<Level>(&path)?);
            level_number += 1;
        }

        if levels
            .windows(2)
            .any(|window| window[1].experience_required <= window[0].experience_required)
        {
            return Err(StaticDataError::InvalidLevels);
        }

        Ok(Self { levels })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Level {
    #[serde(rename = "experienceRequired")]
    pub experience_required: u32,
    pub rubies: u32,
    pub items: Vec<LevelItem>,
    pub buildplates: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LevelItem {
    pub id: String,
    pub count: u32,
}

#[derive(Clone, Debug)]
pub struct TappablesConfig {
    pub tappables: Vec<TappableConfig>,
}

impl TappablesConfig {
    fn load(root: PathBuf) -> Result<Self, StaticDataError> {
        let mut tappables = Vec::new();
        for file in json_files(&root)? {
            tappables.push(read_json_file::<TappableConfig>(&file)?);
        }
        Ok(Self { tappables })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TappableConfig {
    pub icon: String,
    #[serde(rename = "dropSets")]
    pub drop_sets: Vec<DropSet>,
    #[serde(rename = "itemCounts")]
    pub item_counts: HashMap<String, ItemCount>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DropSet {
    pub items: Vec<String>,
    pub chance: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ItemCount {
    pub min: u32,
    pub max: u32,
}

#[derive(Clone, Debug)]
pub struct EncountersConfig {
    pub encounters: Vec<EncounterConfig>,
}

impl EncountersConfig {
    fn load(root: PathBuf) -> Result<Self, StaticDataError> {
        let mut encounters = Vec::new();
        for file in json_files(&root)? {
            encounters.push(read_json_file::<EncounterConfig>(&file)?);
        }
        Ok(Self { encounters })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EncounterConfig {
    pub icon: String,
    pub rarity: Rarity,
    #[serde(rename = "encounterBuildplateId")]
    pub encounter_buildplate_id: String,
    pub duration: u64,
}

fn json_files(root: &Path) -> Result<Vec<PathBuf>, StaticDataError> {
    let mut files = fs::read_dir(root)
        .map_err(|error| StaticDataError::Read(root.to_path_buf(), error))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn read_json_file<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, StaticDataError> {
    let data = fs::read_to_string(path).map_err(|error| StaticDataError::Read(path.to_path_buf(), error))?;
    serde_json::from_str(&data).map_err(|error| StaticDataError::Parse(path.to_path_buf(), error))
}

#[cfg(test)]
mod tests {
    use std::fs::{create_dir_all, write};

    use tempfile::tempdir;

    use super::StaticData;

    #[test]
    fn loads_static_data_layout() {
        let temp_dir = tempdir().expect("tempdir should exist");
        let root = temp_dir.path();
        create_dir_all(root.join("catalog")).expect("catalog dir should exist");
        create_dir_all(root.join("levels")).expect("levels dir should exist");
        create_dir_all(root.join("tappables")).expect("tappables dir should exist");
        create_dir_all(root.join("encounters")).expect("encounters dir should exist");

        write(
            root.join("catalog").join("items.json"),
            r#"[{"id":"stone","rarity":"COMMON","experience":{"tappable":5}}]"#,
        )
        .expect("items should be written");
        write(
            root.join("levels").join("2.json"),
            r#"{"experienceRequired":100,"rubies":1,"items":[],"buildplates":[]}"#,
        )
        .expect("level 2 should be written");
        write(
            root.join("tappables").join("basic.json"),
            r#"{"icon":"stone","dropSets":[{"items":["stone"],"chance":1}],"itemCounts":{"stone":{"min":1,"max":2}}}"#,
        )
        .expect("tappable config should be written");
        write(
            root.join("encounters").join("basic.json"),
            r#"{"icon":"wolf","rarity":"RARE","encounterBuildplateId":"bp-1","duration":60}"#,
        )
        .expect("encounter config should be written");

        let data = StaticData::load(root).expect("static data should load");
        assert_eq!(data.levels.levels.len(), 1);
        assert!(data.catalog.get_item("stone").is_some());
    }
}

