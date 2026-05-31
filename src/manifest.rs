use std::path::Path;

use color_eyre::eyre::{self, WrapErr as _, eyre};
use fs_err as fs;

/// Reads the `version` field from `[workspace.package]` or `[package]` in `path`.
pub fn manifest_read_version(path: &Path) -> eyre::Result<String> {
    let content = fs::read_to_string(path)?;
    version_from_toml(&content).wrap_err_with(|| format!("parsing {}", path.display()))
}

/// Writes `new_version` to the `version` field in `[workspace.package]` or `[package]` at `path`.
pub fn manifest_write_version(path: &Path, new_version: &str) -> eyre::Result<()> {
    let content = fs::read_to_string(path)?;
    let updated = set_version_in_toml(&content, new_version)
        .wrap_err_with(|| format!("parsing {}", path.display()))?;
    fs::write(path, updated)?;
    Ok(())
}

/// Extracts the version string from TOML content; checks `[workspace.package]` then `[package]`.
pub fn version_from_toml(content: &str) -> eyre::Result<String> {
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

/// Returns a copy of `content` with the `version` field replaced by `new_version`.
pub fn set_version_in_toml(content: &str, new_version: &str) -> eyre::Result<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let result = set_version_in_toml(toml, "1.2.3").unwrap();
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
        let result = set_version_in_toml(toml, "0.2.0").unwrap();
        assert!(result.contains(r#"version = "0.2.0""#), "got: {result}");
    }
}
