use std::{
    fs,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand, ValueEnum};
use color_eyre::eyre::{self, WrapErr as _, eyre};
use semver::Version;

// ── CLI ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
#[command(
    name = "chver",
    about = "Coordinate semver between Cargo.toml and CHANGELOG.md"
)]
struct Cli {
    #[arg(
        short = 'C',
        long,
        value_name = "DIR",
        help = "Run as if started in DIR"
    )]
    directory: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Finalise a release: stamp CHANGELOG.md and strip -sid from Cargo.toml
    PreRelease {
        #[arg(default_value = "minor")]
        bump: Bump,
        /// Override Cargo.toml version mismatch (prints warning to stderr)
        #[arg(short, long)]
        force: bool,
    },
    /// Advance Cargo.toml version to <next>-sid (sid = still in development, à la Debian)
    Sid {
        #[arg(default_value = "minor")]
        bump: Bump,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
enum Bump {
    Major,
    #[default]
    Minor,
    Patch,
}

impl Bump {
    fn apply(self, v: &Version) -> Version {
        let mut next = Version::new(v.major, v.minor, v.patch);
        match self {
            Self::Major => {
                next.major += 1;
                next.minor = 0;
                next.patch = 0;
            }
            Self::Minor => {
                next.minor += 1;
                next.patch = 0;
            }
            Self::Patch => {
                next.patch += 1;
            }
        }
        next
    }
}

// ── entry point ──────────────────────────────────────────────────────────────

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();
    let dir = cli.directory.unwrap_or_else(|| PathBuf::from("."));

    let changelog_path = dir.join("CHANGELOG.md");
    if !changelog_path.exists() {
        eyre::bail!("CHANGELOG.md not found in {}", dir.display());
    }
    let manifest_path = resolve_manifest(&dir)?;

    match cli.command {
        Command::PreRelease { bump, force } => {
            cmd_pre_release(&changelog_path, &manifest_path, bump, force)
        }
        Command::Sid { bump } => cmd_sid(&changelog_path, &manifest_path, bump),
    }
}

fn resolve_manifest(dir: &Path) -> eyre::Result<PathBuf> {
    for candidate in [
        dir.join("crates").join("Cargo.toml"),
        dir.join("Cargo.toml"),
    ] {
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    eyre::bail!(
        "neither crates/Cargo.toml nor Cargo.toml found in {}",
        dir.display()
    )
}

// ── commands ─────────────────────────────────────────────────────────────────

fn cmd_pre_release(
    changelog_path: &Path,
    manifest_path: &Path,
    bump: Bump,
    force: bool,
) -> eyre::Result<()> {
    let changelog = fs::read_to_string(changelog_path).wrap_err("failed to read CHANGELOG.md")?;

    let info = changelog_parse(&changelog)?;
    let new_version = bump.apply(&info.latest_version);

    let cargo_version = manifest_read_version(manifest_path)?;
    let expected = format!("{}-sid", new_version);
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
    fs::write(changelog_path, &new_changelog).wrap_err("failed to write CHANGELOG.md")?;

    manifest_write_version(manifest_path, &new_version.to_string())
}

fn cmd_sid(changelog_path: &Path, manifest_path: &Path, bump: Bump) -> eyre::Result<()> {
    let changelog = fs::read_to_string(changelog_path).wrap_err("failed to read CHANGELOG.md")?;

    let info = changelog_parse(&changelog)?;
    let new_version = bump.apply(&info.latest_version);
    manifest_write_version(manifest_path, &format!("{}-sid", new_version))
}

// ── CHANGELOG ────────────────────────────────────────────────────────────────

struct ChangelogInfo {
    latest_version: Version,
    unreleased_url: String,
}

fn changelog_parse(content: &str) -> eyre::Result<ChangelogInfo> {
    let mut latest: Option<Version> = None;
    let mut unreleased_url: Option<String> = None;

    for line in content.lines() {
        if latest.is_none()
            && let Some(v) = version_from_header(line)
        {
            latest = Some(v);
        }
        if let Some(url) = unreleased_url_from_line(line) {
            unreleased_url = Some(url.to_owned());
        }
    }

    Ok(ChangelogInfo {
        latest_version: latest
            .ok_or_else(|| eyre!("no semver version headers found in CHANGELOG.md"))?,
        unreleased_url: unreleased_url
            .ok_or_else(|| eyre!("[unreleased] link definition not found in CHANGELOG.md"))?,
    })
}

fn changelog_apply_prerelease(
    content: &str,
    new_version: &Version,
    today: &str,
) -> eyre::Result<String> {
    let info = changelog_parse(content)?;
    let (new_ver_url, new_unrel_url) = derive_compare_urls(&info.unreleased_url, new_version)?;

    let new_header = format!("## [{}] - {}", new_version, today);
    let new_link = format!("[{}]: {}", new_version, new_ver_url);
    let new_unrel_line = format!("[unreleased]: {}", new_unrel_url);

    let lines: Vec<&str> = content.lines().collect();
    let trailing_newline = content.ends_with('\n');

    let unrel_header_idx = lines
        .iter()
        .position(|l| l.trim().eq_ignore_ascii_case("## [unreleased]"))
        .ok_or_else(|| eyre!("## [Unreleased] header not found in CHANGELOG.md"))?;

    let unrel_link_idx = lines
        .iter()
        .rposition(|l| is_unreleased_link_def(l))
        .ok_or_else(|| eyre!("[unreleased]: link definition not found in CHANGELOG.md"))?;

    // Descending: [unreleased] comes before the first version link.
    let descending = lines
        .iter()
        .enumerate()
        .find(|(_, l)| is_version_link_def(l))
        .is_some_and(|(i, _)| i > unrel_link_idx);

    let mut out = String::with_capacity(content.len() + 128);
    for (i, &line) in lines.iter().enumerate() {
        if i == unrel_header_idx {
            out.push_str(line);
            out.push('\n');
            out.push('\n');
            out.push_str(&new_header);
        } else if i == unrel_link_idx {
            if descending {
                out.push_str(&new_unrel_line);
                out.push('\n');
                out.push_str(&new_link);
            } else {
                out.push_str(&new_link);
                out.push('\n');
                out.push_str(&new_unrel_line);
            }
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }

    if !trailing_newline {
        out.pop();
    }
    Ok(out)
}

fn version_from_header(line: &str) -> Option<Version> {
    // ## [x.y.z] - date
    let rest = line.strip_prefix("## [")?;
    let end = rest.find(']')?;
    rest[..end].parse().ok()
}

fn is_unreleased_link_def(line: &str) -> bool {
    const K: &str = "[unreleased]:";
    line.len() >= K.len() && line[..K.len()].eq_ignore_ascii_case(K)
}

fn unreleased_url_from_line(line: &str) -> Option<&str> {
    if is_unreleased_link_def(line) {
        Some(line["[unreleased]:".len()..].trim())
    } else {
        None
    }
}

fn is_version_link_def(line: &str) -> bool {
    let Some(rest) = line.strip_prefix('[') else {
        return false;
    };
    let Some(end) = rest.find(']') else {
        return false;
    };
    rest[..end].parse::<Version>().is_ok()
}

fn derive_compare_urls(
    unreleased_url: &str,
    new_version: &Version,
) -> eyre::Result<(String, String)> {
    let new_tag = format!("v{}", new_version);

    let (base, _) = unreleased_url
        .rsplit_once("...")
        .filter(|(_, t)| *t == "HEAD")
        .ok_or_else(|| {
            eyre!(
                "[unreleased] URL does not match expected `<url>...HEAD` pattern: {}",
                unreleased_url
            )
        })?;

    // new version compare URL: https://host/compare/vPREV...vNEW
    let new_ver_url = format!("{}...{}", base, new_tag);

    // new unreleased URL: https://host/compare/vNEW...HEAD
    let prefix = base
        .rsplit_once('/')
        .map(|(p, _)| p)
        .ok_or_else(|| eyre!("cannot extract URL prefix from: {}", base))?;
    let new_unrel_url = format!("{}/{}...HEAD", prefix, new_tag);

    Ok((new_ver_url, new_unrel_url))
}

// ── Cargo.toml manifest ──────────────────────────────────────────────────────

fn manifest_read_version(path: &Path) -> eyre::Result<String> {
    let content =
        fs::read_to_string(path).wrap_err_with(|| format!("failed to read {}", path.display()))?;
    version_from_toml(&content).wrap_err_with(|| format!("parsing {}", path.display()))
}

fn manifest_write_version(path: &Path, new_version: &str) -> eyre::Result<()> {
    let content =
        fs::read_to_string(path).wrap_err_with(|| format!("failed to read {}", path.display()))?;
    let updated = version_to_toml(&content, new_version)
        .wrap_err_with(|| format!("parsing {}", path.display()))?;
    fs::write(path, updated).wrap_err_with(|| format!("failed to write {}", path.display()))
}

fn version_from_toml(content: &str) -> eyre::Result<String> {
    let doc: toml_edit::DocumentMut = content.parse().wrap_err("failed to parse TOML")?;
    let v = doc
        .get("workspace")
        .and_then(|i| i.get("package"))
        .and_then(|i| i.get("version"))
        .or_else(|| doc.get("package").and_then(|i| i.get("version")))
        .and_then(|i| i.as_str())
        .ok_or_else(|| eyre!("version field not found"))?;
    Ok(v.to_owned())
}

fn version_to_toml(content: &str, new_version: &str) -> eyre::Result<String> {
    let mut doc: toml_edit::DocumentMut = content.parse().wrap_err("failed to parse TOML")?;

    if doc
        .get("workspace")
        .and_then(|i| i.get("package"))
        .and_then(|i| i.get("version"))
        .is_some()
    {
        doc["workspace"]["package"]["version"] = toml_edit::value(new_version);
    } else if doc.get("package").and_then(|i| i.get("version")).is_some() {
        doc["package"]["version"] = toml_edit::value(new_version);
    } else {
        eyre::bail!("version field not found");
    }
    Ok(doc.to_string())
}

// ── tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn ver(s: &str) -> Version {
        s.parse().unwrap()
    }

    // ── Bump ─────────────────────────────────────────────────────────────────

    #[test]
    fn bump_major() {
        assert_eq!(Bump::Major.apply(&ver("1.2.3")), ver("2.0.0"));
    }

    #[test]
    fn bump_minor() {
        assert_eq!(Bump::Minor.apply(&ver("1.2.3")), ver("1.3.0"));
    }

    #[test]
    fn bump_patch() {
        assert_eq!(Bump::Patch.apply(&ver("1.2.3")), ver("1.2.4"));
    }

    #[test]
    fn bump_strips_prerelease() {
        assert_eq!(Bump::Patch.apply(&ver("1.2.3-alpha.1")), ver("1.2.4"));
    }

    // ── CHANGELOG ────────────────────────────────────────────────────────────

    // Ascending: oldest version first, [unreleased] last.
    const ASCENDING: &str = r#"## [Unreleased]

### Changed

- Something new.

## [1.2.3] - 2026-01-01

### Fixed

- A bug.

## [1.2.2] - 2025-12-01

- Initial.

[1.2.2]: https://github.com/owner/repo/releases/tag/v1.2.2
[1.2.3]: https://github.com/owner/repo/compare/v1.2.2...v1.2.3
[unreleased]: https://github.com/owner/repo/compare/v1.2.3...HEAD
"#;

    // Descending: [unreleased] first, oldest version last.
    const DESCENDING: &str = r#"## [Unreleased]

### Changed

- Something new.

## [1.2.3] - 2026-01-01

- A bug fix.

[unreleased]: https://github.com/owner/repo/compare/v1.2.3...HEAD
[1.2.3]: https://github.com/owner/repo/compare/v1.2.2...v1.2.3
[1.2.2]: https://github.com/owner/repo/releases/tag/v1.2.2
"#;

    #[test]
    fn parses_latest_version() {
        let info = changelog_parse(ASCENDING).unwrap();
        assert_eq!(info.latest_version, ver("1.2.3"));
    }

    #[test]
    fn parses_unreleased_url() {
        let info = changelog_parse(ASCENDING).unwrap();
        assert_eq!(
            info.unreleased_url,
            "https://github.com/owner/repo/compare/v1.2.3...HEAD"
        );
    }

    #[test]
    fn derives_compare_urls() {
        let (new_ver, new_unrel) = derive_compare_urls(
            "https://github.com/owner/repo/compare/v1.2.3...HEAD",
            &ver("1.3.0"),
        )
        .unwrap();
        assert_eq!(
            new_ver,
            "https://github.com/owner/repo/compare/v1.2.3...v1.3.0"
        );
        assert_eq!(
            new_unrel,
            "https://github.com/owner/repo/compare/v1.3.0...HEAD"
        );
    }

    #[test]
    fn prerelease_ascending() {
        let result = changelog_apply_prerelease(ASCENDING, &ver("1.3.0"), "2026-05-31").unwrap();
        insta::assert_snapshot!(result);
    }

    #[test]
    fn prerelease_descending() {
        let result = changelog_apply_prerelease(DESCENDING, &ver("1.3.0"), "2026-05-31").unwrap();
        insta::assert_snapshot!(result);
    }

    // ── manifest ─────────────────────────────────────────────────────────────

    #[test]
    fn reads_workspace_version() {
        let toml = r#"[workspace.package]
version = "1.2.3-sid"
"#;
        assert_eq!(version_from_toml(toml).unwrap(), "1.2.3-sid");
    }

    #[test]
    fn reads_package_version() {
        let toml = r#"[package]
name = "foo"
version = "0.5.0"
"#;
        assert_eq!(version_from_toml(toml).unwrap(), "0.5.0");
    }

    #[test]
    fn writes_workspace_version() {
        let toml = r#"[workspace.package]
version = "1.2.3-sid"
edition = "2021"
"#;
        let result = version_to_toml(toml, "1.2.3").unwrap();
        assert!(result.contains(r#"version = "1.2.3""#), "got: {result}");
        assert!(!result.contains("sid"), "got: {result}");
        assert!(result.contains("edition"), "got: {result}");
    }

    #[test]
    fn writes_package_version() {
        let toml = r#"[package]
name = "bar"
version = "0.1.0"
"#;
        let result = version_to_toml(toml, "0.2.0").unwrap();
        assert!(result.contains(r#"version = "0.2.0""#), "got: {result}");
    }
}
