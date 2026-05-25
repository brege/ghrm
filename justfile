set shell := ["bash", "-euo", "pipefail", "-c"]

[private]
mod benchfile 'bench/.justfile'

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

# run ghrm with ui rebuild watcher
dev-ui target=".":
    npx --prefix ui vite build --config ui/vite.config.js && { \
        npx --prefix ui vite build --config ui/vite.config.js --watch & \
        VITE_PID=$!; \
        trap "kill $VITE_PID 2>/dev/null" EXIT; \
        cargo run --locked -- --no-browser "{{target}}"; \
    }

# print the resolved ghrm configuration
dump-config target=".":
    cargo run --locked -- --dump-config "{{target}}"

# install ghrm from this checkout
install:
    cargo install --locked --path .

# smoke-run release ASV without recording benchmark history
bench: benchfile::run

# validate ASV benchmark discovery
bench-check: benchfile::check

# alias for the ASV smoke run
bench-dry: benchfile::dry

# record the latest commit benchmark and print the report
bench-record: benchfile::record

# run all configured pre-commit hooks
precommit:
    pre-commit run --all-files

# check Rust and UI files
check: rust ui

# run the main crate and ghrm-stat test suites
test:
    cargo test --locked
    cargo test --manifest-path ghrm-stat/Cargo.toml --locked

# remove build artifacts
clean:
    cargo clean
    cargo clean --manifest-path ghrm-stat/Cargo.toml

# format Rust and UI files
fmt: rust-fmt ui-fmt

# run all Rust checks
rust:
    cargo fmt --all --check
    cargo fmt --manifest-path ghrm-stat/Cargo.toml --check
    cargo check --locked
    cargo check --manifest-path ghrm-stat/Cargo.toml --locked
    cargo clippy --all-targets --locked -- --deny warnings
    cargo clippy --manifest-path ghrm-stat/Cargo.toml --all-targets --locked -- --deny warnings
    cargo test --locked
    cargo test --manifest-path ghrm-stat/Cargo.toml --locked

# run Rust type checks
rust-type:
    cargo check --locked
    cargo check --manifest-path ghrm-stat/Cargo.toml --locked

# run Rust lint checks
rust-lint:
    cargo clippy --all-targets --locked -- --deny warnings
    cargo clippy --manifest-path ghrm-stat/Cargo.toml --all-targets --locked -- --deny warnings

# format Rust files
rust-fmt:
    cargo fmt --all
    cargo fmt --manifest-path ghrm-stat/Cargo.toml

# run all UI checks
ui: ui-fmt
    pre-commit run biome-check --all-files
    npm --prefix ui run typecheck
    npm --prefix ui run test
    npm --prefix ui run icons:check
    npm --prefix ui run build:check

# refresh generated bundle when UI source changed - only on main
ui-release:
    @branch="$(git rev-parse --abbrev-ref HEAD)"; if [[ "$branch" != "main" ]]; then echo "ui-release only runs on main"; exit 1; fi
    just ui
    @if git diff --quiet -- ui && git diff --cached --quiet -- ui && git diff --quiet HEAD^ HEAD -- ui; then echo "No UI source changes; skipping runtime asset pack"; else npm --prefix ui run build; fi

# run UI type checks
ui-type:
    npm --prefix ui run typecheck

# run UI lint and format checks
ui-lint:
    pre-commit run biome-check --all-files

# format UI files
ui-fmt:
    npx @biomejs/biome@2.4.6 format --write ui/ assets/css
