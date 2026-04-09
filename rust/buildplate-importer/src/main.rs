use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use anyhow::{bail, Context};
use clap::Parser;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zip::ZipArchive;

const REQUIRED_WORLD_FILES: [&str; 8] = [
    "region/r.0.0.mca",
    "region/r.0.-1.mca",
    "region/r.-1.0.mca",
    "region/r.-1.-1.mca",
    "entities/r.0.0.mca",
    "entities/r.0.-1.mca",
    "entities/r.-1.0.mca",
    "entities/r.-1.-1.mca",
];

#[derive(Debug, Parser)]
#[command(name = "vienna-buildplate-importer")]
#[command(about = "Rust rewrite scaffold for the Vienna buildplate importer")]
struct Args {
    #[arg(long, default_value = "./earth.db")]
    db: PathBuf,

    #[arg(long, default_value = "localhost:5396")]
    objectstore: String,

    #[arg(long, default_value = "localhost:5532")]
    eventbus: String,

    #[arg(long = "player-id")]
    player_id: String,

    #[arg(long = "world-file")]
    world_file: PathBuf,
}

#[derive(Debug, Deserialize)]
struct BuildplateMetadataVersion {
    version: i32,
}

#[derive(Debug, Deserialize)]
struct BuildplateMetadataV1 {
    size: i32,
    offset: i32,
    night: bool,
}

#[derive(Debug, Serialize)]
struct ImportPreview {
    buildplate_id: String,
    player_id: String,
    db: PathBuf,
    objectstore: String,
    eventbus: String,
    world_file: PathBuf,
    size: i32,
    offset: i32,
    night: bool,
    required_file_count: usize,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let world = read_world_file(&args.world_file)?;

    let preview = ImportPreview {
        buildplate_id: Uuid::new_v4().to_string(),
        player_id: args.player_id.to_lowercase(),
        db: args.db,
        objectstore: args.objectstore,
        eventbus: args.eventbus,
        world_file: args.world_file,
        size: world.size,
        offset: world.offset,
        night: world.night,
        required_file_count: world.required_files.len(),
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&preview).context("failed to serialize import preview")?
    );

    Ok(())
}

struct WorldData {
    size: i32,
    offset: i32,
    night: bool,
    required_files: HashMap<String, Vec<u8>>,
}

fn read_world_file(path: &PathBuf) -> anyhow::Result<WorldData> {
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut archive = ZipArchive::new(file).context("failed to open world zip archive")?;

    let mut world_files = HashMap::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).context("failed to read zip entry")?;
        if entry.is_dir() {
            continue;
        }

        let name = entry.name().to_owned();
        if name == "buildplate_metadata.json" || REQUIRED_WORLD_FILES.iter().any(|required| *required == name)
        {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)
                .with_context(|| format!("failed to read {name}"))?;
            world_files.insert(name, bytes);
        }
    }

    for required in REQUIRED_WORLD_FILES {
        if !world_files.contains_key(required) {
            bail!("world file is missing required entry {required}");
        }
    }

    let (size, offset, night) = if let Some(metadata_bytes) = world_files.get("buildplate_metadata.json") {
        let version: BuildplateMetadataVersion =
            serde_json::from_slice(metadata_bytes).context("failed to parse buildplate metadata version")?;
        match version.version {
            1 => {
                let metadata: BuildplateMetadataV1 =
                    serde_json::from_slice(metadata_bytes).context("failed to parse buildplate metadata")?;
                (metadata.size, metadata.offset, metadata.night)
            }
            other => bail!("unsupported buildplate metadata version {other}"),
        }
    } else {
        (16, 63, false)
    };

    if !matches!(size, 8 | 16 | 32) {
        bail!("invalid buildplate size {size}");
    }

    let required_files = REQUIRED_WORLD_FILES
        .iter()
        .map(|required| {
            (
                (*required).to_owned(),
                world_files
                    .remove(*required)
                    .expect("required file should exist after validation"),
            )
        })
        .collect::<HashMap<_, _>>();

    Ok(WorldData {
        size,
        offset,
        night,
        required_files,
    })
}
