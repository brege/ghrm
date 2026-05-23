set shell := ["bash", "-euo", "pipefail", "-c"]

# list benchmark recipes
[private]
default:
    @just --list

# check benchmark Python files with Ruff
[private]
lint:
    uv run ruff check --fix

# format benchmark Python files with Ruff
[private]
fmt:
    uv run ruff format

# check benchmark Python formatting
[private]
fmt-check:
    uv run ruff format --check

# validate ASV benchmark discovery
check:
    uv run asv check

# smoke-run release ASV without recording benchmark history
run:
    cd .. && cargo build --release --locked
    ASV_BUILD_DIR="$PWD/.." uv run asv run --python=same --quick --show-stderr --dry-run

# alias for the ASV smoke run
dry: run

# record the latest commit benchmark and print the report
record:
    uv run asv run HEAD^!
    uv run python report.py
