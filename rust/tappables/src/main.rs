use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use vienna_tappables::{ActiveTile, Spawner, StaticData};

#[derive(Debug, Parser)]
#[command(name = "vienna-tappablesgenerator")]
#[command(about = "Rust rewrite of the Vienna tappables generator")]
struct Args {
    #[arg(long = "static-data", default_value = "./data")]
    static_data: PathBuf,

    #[arg(long)]
    tile_x: i32,

    #[arg(long)]
    tile_y: i32,

    #[arg(long, default_value_t = 1_710_000_000_000)]
    time_ms: u64,

    #[arg(long, default_value_t = 42)]
    seed: u64,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let static_data = StaticData::load(&args.static_data)
        .with_context(|| format!("failed to load static data from {}", args.static_data.display()))?;

    let mut spawner = Spawner::new(&static_data, args.seed, args.time_ms);
    let batch = spawner
        .spawn_tiles(
            &[ActiveTile {
                tile_x: args.tile_x,
                tile_y: args.tile_y,
                first_active_time: args.time_ms,
                latest_active_time: args.time_ms,
            }],
            args.time_ms,
        )
        .context("failed to generate tappables for tile")?;

    println!(
        "{}",
        serde_json::to_string_pretty(&batch).context("failed to serialize spawn batch")?
    );

    Ok(())
}
