set shell := ["bash", "-euo", "pipefail", "-c"]

# list available recipes
default:
    @just --list

# build the debug binary
build:
    cargo build --locked

# build and run ghrm against a target path
run target=".":
    cargo run --locked -- "{{target}}"

# run ghrm without opening a browser
dev target=".":
    cargo run --locked -- --no-browser "{{target}}"

# print the resolved ghrm configuration
dump-config target=".":
    cargo run --locked -- --dump-config "{{target}}"

# install ghrm from this checkout
install:
    cargo install --locked --path .

# run all configured pre-commit hooks
hooks:
    pre-commit run --all-files

# check Rust, UI, and benchmark Python files
check: _check-ghrm _check-stat biome-check ruff-check

# run the main crate and ghrm-stat test suites
test: _test-ghrm _test-stat

# run clippy for the main crate and ghrm-stat
clippy: _clippy-ghrm _clippy-stat

# format Rust, UI, and benchmark Python files
fmt:
    cargo fmt --all
    cargo fmt --manifest-path ghrm-stat/Cargo.toml
    npx @biomejs/biome@2.4.6 check --write assets/js assets/css
    uv run --project bench ruff format

# check Rust, UI, and benchmark Python formatting
fmt-check: biome-check
    cargo fmt --all --check
    cargo fmt --manifest-path ghrm-stat/Cargo.toml --check
    uv run --project bench ruff format --check

# check UI assets with Biome
biome-check:
    pre-commit run biome-check --all-files

# check benchmark Python files with Ruff
ruff-check:
    pre-commit run ruff-check --all-files

# run the ghrm-stat printer
stat target=".":
    cargo run --manifest-path ghrm-stat/Cargo.toml --locked -- "{{target}}"

# run the ghrm-stat printer as JSON
stat-json target=".":
    cargo run --manifest-path ghrm-stat/Cargo.toml --locked -- "{{target}}" --json

# validate ASV benchmark discovery
bench-check:
    cd bench && uv run asv check

# smoke-run release ASV without recording benchmark history
bench:
    cargo build --release --locked
    cd bench && ASV_BUILD_DIR="$PWD/.." uv run asv run --python=same --quick --show-stderr --dry-run

# alias for the ASV smoke run
bench-dry: bench

# record the latest commit benchmark and print the report
bench-record:
    cd bench && uv run asv run HEAD^!
    cd bench && uv run python report.py

_check-ghrm:
    cargo check --locked

_check-stat:
    cargo check --manifest-path ghrm-stat/Cargo.toml --locked

_test-ghrm:
    cargo test --locked

_test-stat:
    cargo test --manifest-path ghrm-stat/Cargo.toml --locked

_clippy-ghrm:
    cargo clippy --all-targets --locked -- --deny warnings

_clippy-stat:
    cargo clippy --manifest-path ghrm-stat/Cargo.toml --all-targets --locked -- --deny warnings
