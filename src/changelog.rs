use color_eyre::eyre::{self, eyre};
use semver::Version;

/// Metadata extracted from a CHANGELOG.md file.
pub struct ChangelogInfo {
    /// Most recent released version, from the first `## [x.y.z]` header.
    pub latest_version: Version,
    /// URL from the `[unreleased]:` link definition.
    pub unreleased_url: String,
}

/// Parses `content` and returns the latest released version and `[unreleased]` URL.
pub fn changelog_parse(content: &str) -> eyre::Result<ChangelogInfo> {
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

/// Returns a copy of `content` with a new version header inserted and link definitions updated.
pub fn changelog_apply_prerelease(
    content: &str,
    new_version: &Version,
    today: &str,
) -> eyre::Result<String> {
    let info = changelog_parse(content)?;
    let (new_ver_url, new_unrel_url) = derive_compare_urls(&info.unreleased_url, new_version)?;

    let new_header = format!("## [{new_version}] - {today}");
    let new_link = format!("[{new_version}]: {new_ver_url}");
    let new_unrel_line = format!("[unreleased]: {new_unrel_url}");

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

/// Derives `(new_version_url, new_unreleased_url)` from the current `[unreleased]` compare URL.
pub fn derive_compare_urls(
    unreleased_url: &str,
    new_version: &Version,
) -> eyre::Result<(String, String)> {
    let new_tag = format!("v{new_version}");

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
    let new_ver_url = format!("{base}...{new_tag}");

    // new unreleased URL: https://host/compare/vNEW...HEAD
    let prefix = base
        .rsplit_once('/')
        .map(|(p, _)| p)
        .ok_or_else(|| eyre!("cannot extract URL prefix from: {}", base))?;
    let new_unrel_url = format!("{prefix}/{new_tag}...HEAD");

    Ok((new_ver_url, new_unrel_url))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ver(s: &str) -> Version {
        s.parse().unwrap()
    }

    // Ascending: oldest version first, [unreleased] last.
    const ASCENDING: &str = r"## [Unreleased]

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
";

    // Descending: [unreleased] first, oldest version last.
    const DESCENDING: &str = r"## [Unreleased]

### Changed

- Something new.

## [1.2.3] - 2026-01-01

- A bug fix.

[unreleased]: https://github.com/owner/repo/compare/v1.2.3...HEAD
[1.2.3]: https://github.com/owner/repo/compare/v1.2.2...v1.2.3
[1.2.2]: https://github.com/owner/repo/releases/tag/v1.2.2
";

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
}
