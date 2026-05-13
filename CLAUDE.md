# ghrm Agent Notes

@README.md  
@Cargo.toml

For mixed changes to source code, always run all configured pre-commit hooks:

```bash
pre-commit run --all-files
```

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

After main-crate Rust changes, run the configured Rust pre-commit hooks:

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

For repo-wide Rust changes, run both hook groups or `pre-commit run --all-files`.

Keep `src/` organized by feature boundary. Do not turn root modules into
catchall files. Preserve the current split between `http`, `explorer`,
`search`, `render`, and root-level core modules.

ghrm uses a column-registry style for explorer and search tables. New table
columns should stay declarative and avoid base-code branching.

The root Rust files should remain small coordination or core modules. Feature
behavior belongs under the feature boundary that owns it.

## UI

The frontend is organized by feature boundary:

```text
assets/
├── config.json
├── css/
├── js/
├── templates/
└── vendor/
```

After UI changes, run the configured Biome pre-commit hook:

```bash
pre-commit run biome-check --all-files
```

Do not recreate catchall files in `assets/js/` or `assets/css/`. Keep CSS and
JS changes within the existing feature files unless the feature boundary itself
is being revised.

Template changes should preserve the Rust data contract. Do not move structure
into JavaScript just to avoid changing a template or view model.

## Benchmarks

@bench/README.md

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

Prefer the shared `justfile` recipes for common workflows:

```bash
just build
just dev <PATH>
just run <PATH>
just dump-config <PATH>
just install
```

Use `just install` when an install is explicitly requested.

## Reference Implementations

Reference checkouts may be placed in `refs/`.

### Rust Core Utilities

Three projects have shaped ghrm's search, file, and content retrieval:

- fd: @refs/fd/README.md
- ripgrep: @refs/ripgrep/README.md
- tokei: @refs/tokei/README.md

Onefetch has been inspirational for the `ghrm-stat` crate:

- onefetch: @refs/onefetch/README.md

### Resources

Some upstream-derived assets should be checked from time to time. When updating the referenced projects above, check whether these need to be refreshed:

- `license.cache.zstd`: AskAlono license cache copied from Onefetch
- `languages.json`: tokei's project extension and shebang database

### Benchmark Utility

For details not available in `bench/README.md`:

- ASV: @refs/asv/README.md
