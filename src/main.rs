mod changelog;
mod cli;
mod manifest;

use std::path::{Path, PathBuf};

use clap::Parser;
use color_eyre::eyre;
use fs_err as fs;

use crate::{
    changelog::{changelog_apply_prerelease, changelog_parse},
    cli::{Bump, Cli, Command},
    manifest::{manifest_read_version, manifest_write_version},
};

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();
    let dir = cli.directory.unwrap_or_else(|| PathBuf::from("."));

    let changelog = dir.join("CHANGELOG.md");
    eyre::ensure!(
        changelog.exists(),
        "CHANGELOG.md not found in {}",
        dir.display()
    );
    let manifest = resolve_manifest(&dir)?;

    match cli.command {
        Command::PreRelease { bump, force } => cmd_pre_release(&changelog, &manifest, bump, force),
        Command::Sid { bump } => cmd_sid(&changelog, &manifest, bump),
    }
}

fn resolve_manifest(dir: &Path) -> eyre::Result<PathBuf> {
    for candidate in [
        dir.join("Cargo.toml"),
        dir.join("crates").join("Cargo.toml"),
    ] {
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    eyre::bail!(
        "neither Cargo.toml nor crates/Cargo.toml found in {}",
        dir.display()
    )
}

fn cmd_pre_release(
    changelog_path: &Path,
    manifest_path: &Path,
    bump: Bump,
    force: bool,
) -> eyre::Result<()> {
    let changelog = fs::read_to_string(changelog_path)?;
    let info = changelog_parse(&changelog)?;
    let new_version = bump.apply(&info.latest_version);

    let cargo_version = manifest_read_version(manifest_path)?;
    let expected = format!("{new_version}-sid");
    if cargo_version != expected {
        if force {
            eprintln!(
                "warning: expected `{expected}` in {}, found `{cargo_version}`",
                manifest_path.display(),
            );
        } else {
            eyre::bail!(
                "version in {} is `{cargo_version}`, expected `{expected}`; use -f/--force to override",
                manifest_path.display(),
            );
        }
    }

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let new_changelog = changelog_apply_prerelease(&changelog, &new_version, &today)?;
    fs::write(changelog_path, new_changelog)?;

    manifest_write_version(manifest_path, &new_version.to_string())
}

fn cmd_sid(changelog_path: &Path, manifest_path: &Path, bump: Bump) -> eyre::Result<()> {
    let changelog = fs::read_to_string(changelog_path)?;
    let info = changelog_parse(&changelog)?;
    let new_version = bump.apply(&info.latest_version);
    manifest_write_version(manifest_path, &format!("{new_version}-sid"))
}
