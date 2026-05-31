use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "chver",
    about = "Coordinate semver between Cargo.toml and CHANGELOG.md"
)]
pub struct Cli {
    #[arg(
        short = 'C',
        long,
        value_name = "DIR",
        help = "Run as if started in DIR"
    )]
    pub directory: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Finalise a release: stamp CHANGELOG.md and strip -sid from Cargo.toml
    PreRelease {
        #[arg(default_value = "minor")]
        bump: Bump,
        /// Override Cargo.toml version mismatch (prints warning to stderr)
        #[arg(short, long)]
        force: bool,
    },
    /// Advance Cargo.toml version to `<next>-sid` (sid = still in development)
    Sid {
        #[arg(default_value = "minor")]
        bump: Bump,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum Bump {
    Major,
    #[default]
    Minor,
    Patch,
}

impl Bump {
    /// Advances `v` by this increment, stripping any pre-release suffix.
    pub fn apply(self, v: &semver::Version) -> semver::Version {
        let mut next = semver::Version::new(v.major, v.minor, v.patch);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ver(s: &str) -> semver::Version {
        s.parse().unwrap()
    }

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
}
