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
    npm --prefix ui run build:runtime && { \
        npm --prefix ui run build:watch & \
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

# set workspace version (does not commit or tag)
bump version:
    @sed -i 's/^\(version = \)"[^"]*"/\1"{{version}}"/' Cargo.toml && \
    sed -i 's/^\(ghrm-stat = { version = \)"[^"]*"/\1"{{version}}"/' Cargo.toml && \
    echo "Workspace version set to {{version}}"

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

# run Rust and UI test suites
test: rust-test ui-test

# remove build artifacts
clean:
    cargo clean

# format Rust and UI files
fmt: rust-fmt ui-fmt

# run all Rust checks
rust:
    cargo fmt --all --check
    cargo check --workspace --locked
    cargo clippy --workspace --all-targets --locked -- --deny warnings
    cargo test --workspace --locked

# run Rust type checks
rust-type:
    cargo check --workspace --locked

# run Rust lint checks
rust-lint:
    cargo clippy --workspace --all-targets --locked -- --deny warnings

# run Rust test suites
rust-test:
    cargo test --workspace --locked

# format Rust files
rust-fmt:
    cargo fmt --all

# run all UI checks
ui: ui-lint ui-type ui-test ui-icons ui-build

# refresh generated bundle when UI source changed - only on main
ui-release:
    @branch="$(git rev-parse --abbrev-ref HEAD)"; if [[ "$branch" != "main" ]]; then echo "ui-release only runs on main"; exit 1; fi
    just ui
    @if git diff --quiet -- ui && git diff --cached --quiet -- ui && git diff --quiet HEAD^ HEAD -- ui; then echo "No UI source changes; skipping runtime asset pack"; else npm --prefix ui run build; fi

# run UI type checks
ui-type:
    npm --prefix ui run typecheck

# run UI Biome lint, formatting, and style checks
ui-lint:
    pre-commit run biome-check --all-files

# run UI tests
ui-test:
    npm --prefix ui run test

# validate UI icon assets
ui-icons:
    npm --prefix ui run icons:check

# run UI build verification
ui-build:
    npm --prefix ui run build:check

# watch UI runtime rebuilds for local dev
ui-watch:
    npm --prefix ui run build:watch

# format UI files
ui-fmt:
    npx @biomejs/biome@2.4.6 format --write ui/ assets/css
