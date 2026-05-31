## Reference: https://just.systems/man/en/

# [default] Run 'cargo check', all features and targets
check:
    cargo check --all-features --all-targets

# Format Rust code
fmt *opts:
    cargo fmt --all {{opts}}

# Run clippy with pedantic lints, all features and targets
clippy:
    cargo clippy --all-features --all-targets -- -W clippy::pedantic

# Generate documentation
doc:
    cargo doc --no-deps --document-private-items

# Detect unused dependencies (add '--fix' to remove them)
shear *opts: _require-cargo-shear
    cargo shear {{opts}}

# Run all tests, all features and targets
test:
    cargo test --all-features --all-targets

# Check formatting, run clippy, check docs, shear
lint: (fmt '--check') clippy doc shear

# Run all checks and tests
everything: lint test

alias all := everything

# Remove generated artifacts
clean:
    cargo clean

_require-cargo-shear:
    #!/usr/bin/env bash
    set -eu

    if ! cargo shear --help &>/dev/null; then
        echo >&2 'cargo-shear is not installed'
        echo >&2
        echo >&2 "Run 'cargo binstall cargo-shear' or 'cargo install cargo-shear'"
        exit 1
    fi
