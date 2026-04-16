use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context};
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "vienna-mcemod-packager")]
#[command(about = "Builds a Rust cdylib and packages it as a .mcemod file")]
struct Args {
    #[arg(long)]
    package: String,

    #[arg(long, default_value = "./mods")]
    out_dir: PathBuf,

    #[arg(long)]
    name: Option<String>,

    #[arg(long)]
    release: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let profile = if args.release { "release" } else { "debug" };
    let workspace_root = workspace_root();

    let mut build_command = Command::new("cargo");
    build_command.arg("build").arg("-p").arg(&args.package);
    build_command.current_dir(&workspace_root);
    if args.release {
        build_command.arg("--release");
    }

    let status = build_command
        .status()
        .context("failed to start cargo build for .mcemod packaging")?;
    if !status.success() {
        bail!("cargo build failed for package {}", args.package);
    }

    let dll_name = format!("{}.dll", args.package.replace('-', "_"));
    let source_path = workspace_root.join("target").join(profile).join(&dll_name);
    if !source_path.exists() {
        bail!(
            "compiled cdylib was not found at {}",
            source_path.display()
        );
    }

    let out_dir = resolve_workspace_path(&workspace_root, &args.out_dir);
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create output directory {}", out_dir.display()))?;

    let artifact_name = args.name.unwrap_or_else(|| args.package.clone());
    let target_path = out_dir.join(format!("{artifact_name}.mcemod"));

    fs::copy(&source_path, &target_path).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source_path.display(),
            target_path.display()
        )
    })?;

    println!("packaged {}", target_path.display());
    Ok(())
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should exist")
        .to_path_buf()
}

fn resolve_workspace_path(workspace_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    }
}
