# chver

Coordinates version strings between `crates/Cargo.toml` and `CHANGELOG.md` for Rust workspace
projects. The name stands for **changelog version**.

## Commands

### `chver pre-release [major|minor|patch]`

Finalises the current development cycle (default bump: `minor`):

1. Reads the latest semver from `CHANGELOG.md` and advances it by the given increment.
2. Verifies `crates/Cargo.toml` carries `<new-version>-sid`; fails unless `-f`/`--force`.
3. Inserts `## [<new-version>] - <today>` under `## [Unreleased]`.
4. Inserts `[<new-version>]: <compare-url>` adjacent to the `[unreleased]` link definition
   (above it for ascending-order changelogs; below it for descending-order).
5. Updates the `[unreleased]` link to point at the new version.
6. Strips `-sid` from the Cargo.toml version.

### `chver sid [major|minor|patch]`

Prepares the next development cycle (default bump: `minor`):

1. Reads the latest semver from `CHANGELOG.md` and advances it.
2. Sets `version` in the Cargo.toml manifest to `<new-version>-sid`.

The `-sid` suffix stands for **still in development**.

## Options

| Flag | Description |
|------|-------------|
| `-C <dir>` / `--directory <dir>` | Run as if started in `<dir>` |
| `-f` / `--force` | (`pre-release`) Override Cargo.toml version mismatch |

## Cargo.toml detection

The tool looks for `crates/Cargo.toml` first (workspace layout), then `Cargo.toml`.
The version is read from `[workspace.package]` or `[package]`.

## CHANGELOG.md format

[Keep a Changelog](https://keepachangelog.com/) format is expected. Link definitions at the
end of the file must include a `[unreleased]:` entry pointing to a GitHub compare URL:

```
[unreleased]: https://github.com/<owner>/<repo>/compare/v<version>...HEAD
```

Both **ascending** (oldest version first, `[unreleased]` last) and **descending**
(`[unreleased]` first) link-definition orders are supported.

## Tests

Unit tests for bump logic and manifest editing run with `cargo test`.

The two snapshot tests (`prerelease_ascending`, `prerelease_descending`) require an initial
`cargo insta review` to accept the generated snapshots.
