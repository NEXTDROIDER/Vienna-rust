use std::env;
use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(name = "vienna-buildplate-launcher")]
#[command(about = "Rust rewrite scaffold for the Vienna buildplate launcher")]
struct Args {
    #[arg(long, default_value = "localhost:5532")]
    eventbus: String,

    #[arg(long = "public-address")]
    public_address: String,

    #[arg(long = "bridge-jar")]
    bridge_jar: PathBuf,

    #[arg(long = "server-template-dir")]
    server_template_dir: PathBuf,

    #[arg(long = "fabric-jar-name")]
    fabric_jar_name: String,

    #[arg(long = "connector-plugin-jar")]
    connector_plugin_jar: PathBuf,
}

#[derive(Debug, Serialize)]
struct LauncherConfig {
    eventbus: String,
    public_address: String,
    java_command: String,
    bridge_jar: PathBuf,
    server_template_dir: PathBuf,
    fabric_jar_name: String,
    connector_plugin_jar: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = LauncherConfig {
        eventbus: args.eventbus,
        public_address: args.public_address,
        java_command: locate_java(),
        bridge_jar: args.bridge_jar,
        server_template_dir: args.server_template_dir,
        fabric_jar_name: args.fabric_jar_name,
        connector_plugin_jar: args.connector_plugin_jar,
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&config).context("failed to serialize launcher config")?
    );

    Ok(())
}

fn locate_java() -> String {
    if let Ok(java_home) = env::var("JAVA_HOME") {
        let mut path = PathBuf::from(java_home);
        path.push("bin");
        path.push(if cfg!(windows) { "java.exe" } else { "java" });
        return path.display().to_string();
    }

    if cfg!(windows) {
        "java.exe".to_owned()
    } else {
        "java".to_owned()
    }
}

