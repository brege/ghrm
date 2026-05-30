# ghrm Agent Notes

@README.md  
@Cargo.toml

[@README.md]: README.md  
[@Cargo.toml]: Cargo.toml  

For mixed changes, use the justfile recipe for all configured pre-commit hooks:

```bash
just precommit
```

Prefer `just` recipes before direct `npm`, `cargo`, `pre-commit`, `uv`, or ASV commands when a recipe exists. Run `just --list` when choosing a workflow.

## Repo Rules

Keep the current architecture crisp. Recent work deliberately removed fallback
paths, catchall files, duplicate asset contracts, and browser-side detection
that belonged in Rust.

- prefer existing libraries and reference implementations over local taxonomy
  code
- validate at entry points, then let internal contracts hold
- do not add fallback or backward-compatibility branches for removed
  intermediate states
- keep native browser-renderable files native; do not inline arbitrary file
  formats into app HTML
- htmx owns boosted navigation; do not duplicate navigation behavior in custom
  browser code
- server-rendered templates define structure; JavaScript enhances behavior
- `assets/config.json` is the vendor asset source of truth
- do not edit `assets/vendor/` by hand
- declare vendor URLs and generated assets through the asset schema, not
  scattered relative URLs
- dirty-tree benchmark runs are smoke checks only; ASV result history belongs
  to commits

## Rust

@src/README.md

[@src/README.md]: src/README.md

After main-crate Rust changes, prefer `just rust` for the full Rust check path. Use the configured Rust pre-commit hooks when checking hook parity or when the change needs a scoped hook run:

```bash
pre-commit run cargo-check --all-files
pre-commit run cargo-fmt --all-files
pre-commit run cargo-test --all-files
pre-commit run cargo-clippy --all-files
```

After `ghrm-stat/` changes, run the configured `ghrm-stat` hooks:

```bash
pre-commit run ghrm-stat-check --all-files
pre-commit run ghrm-stat-fmt --all-files
pre-commit run ghrm-stat-test --all-files
pre-commit run ghrm-stat-clippy --all-files
```

For repo-wide Rust changes, run `just rust` or `just precommit` depending on whether UI, benchmark, or generated-hook coverage is also needed.

Keep `src/` organized by feature boundary. Do not turn root modules into
catchall files. Preserve the current split between `http`, `explorer`,
`search`, `render`, and root-level core modules.

ghrm uses a column-registry style for explorer and search tables. New table
columns should stay declarative and avoid base-code branching.

The root Rust files should remain small coordination or core modules. Feature
behavior belongs under the feature boundary that owns it.

## UI

@ui/README.md

[@ui/README.md]: ui/README.md

The frontend is organized by feature boundary:

```text
assets/
├── config.json
├── css/
├── img/
├── js.sha256.json
├── js.tar.zst
└── templates/
ui/src/
```

After UI changes, prefer the justfile UI recipes:

```bash
just ui
just ui-test
just ui-build
just ui-watch
just dev-ui <PATH>
just ui-release
```

`just ui` runs Biome, TypeScript, Vitest, icon validation, and Vite build verification. The Biome pre-commit hook only covers lint, formatting, and style checks for `assets/css/` and `ui/`.

`just ui-build` only verifies a disposable Vite build. `just ui-release` on `main` is the normal path that refreshes `assets/js.tar.zst` and `assets/js.sha256.json`.

Do not edit generated runtime JS under `assets/js/` by hand. Keep browser
source changes under `ui/src/`, CSS changes under `assets/css/`, and runtime archive changes flowing through the UI build and release recipes.

Use `just ui-release` to refresh tracked runtime assets on `main`. Do not refresh `assets/js.tar.zst` or `assets/js.sha256.json` through ad hoc `npm` commands unless intentionally debugging the release recipe itself.

Template changes should preserve the Rust data contract. Do not move structure
into JavaScript just to avoid changing a template or view model.

## Benchmarks

@bench/README.md

[@bench/README.md]: bench/README.md

After benchmark Python changes, run the configured Ruff hooks:

```bash
pre-commit run ruff-check --all-files
pre-commit run ruff-format --all-files
```

Do not record benchmarks for uncommitted changes. Recorded benchmark history
belongs to commits.

For benchmark-affecting changes, run the current-tree ASV smoke check without
recording results:

```bash
just bench
```

`just bench-record` records benchmark history for the current tip. Run it only
when explicitly asked.

## Build / Install / Run

Prefer the shared `justfile` recipes for common workflows. Use `just --list` when the relevant recipe is not obvious:

```bash
just check
just test
just fmt
just precommit
just build
just dev <PATH>
just dev-ui <PATH>
just run <PATH>
just dump-config <PATH>
just install
just ui
just ui-build
just ui-test
just ui-watch
just ui-release
```

Use `just install` when an install is explicitly requested.

## Reference Implementations

Reference checkouts may be placed in `refs/`.

### Rust Core Utilities

Three projects have shaped ghrm's search, file, and content retrieval:

- fd: @refs/fd/README.md
- ripgrep: @refs/ripgrep/README.md
- tokei: @refs/tokei/README.md

[@refs/fd/README.md]: refs/fd/README.md
[@refs/ripgrep/README.md]: refs/ripgrep/README.md
[@refs/tokei/README.md]: refs/tokei/README.md

Onefetch has been inspirational for the `ghrm-stat` crate:

- onefetch: @refs/onefetch/README.md

### Resources

Some upstream-derived assets should be checked from time to time. When updating the referenced projects above, check whether these need to be refreshed:

- `license.cache.zstd`: AskAlono license cache copied from Onefetch
- `languages.json`: tokei's project extension and shebang database

### Benchmark Utility

For details not available in `bench/README.md`:

- ASV: @refs/asv/README.md
