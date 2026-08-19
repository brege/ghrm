# src

The Rust source tree keeps app-wide contracts flat and groups feature code under namespace files.

## Root

Shared app contracts and binary-level support stay flat.

| Path | Purpose |
| --- | --- |
| main.rs | binary entry |
| config.rs | config schema and loading |
| dirs.rs | OS app directories |
| options.rs | CLI and config resolution |
| paths.rs | path safety helpers |
| runtime.rs | runtime paths shown in UI |
| tmpl.rs | Askama template structs |

## Render

Markdown rendering code sits behind the render namespace.

| Path | Purpose |
| --- | --- |
| render.rs | render pipeline and public API |
| render/alert.rs | GitHub alert blocks |
| render/anchor.rs | headings and page title |
| render/code.rs | code and source blocks |
| render/math.rs | markdown math blocks |
| render/path.rs | local URL rewriting |
| render/tests.rs | render behavior tests |

## HTTP

Local web-serving code sits behind the HTTP namespace.

| Path | Purpose |
| --- | --- |
| http.rs | HTTP namespace |
| http/server.rs | Axum router and server state |
| http/api.rs | JSON and fragment API routes |
| http/archive.rs | directory archive downloads |
| http/auth.rs | auth middleware and handlers |
| http/delivery.rs | native file delivery |
| http/diff.rs | file compare views |
| http/shell.rs | full page and fragment responses |
| http/assets.rs | embedded runtime asset cache |
| http/vendor.rs | downloaded vendor asset cache |

## Explorer

Filesystem browser code sits behind the explorer namespace.

| Path | Purpose |
| --- | --- |
| explorer.rs | explorer rendering entry point |
| explorer/column.rs | explorer column definitions |
| explorer/crumbs.rs | breadcrumb links |
| explorer/filter.rs | explorer filter groups |
| explorer/view.rs | explorer URL state |
| explorer/walk.rs | filesystem nav tree |
| explorer/watch.rs | file watch updates |

## Search

Search code sits behind the search namespace.

| Path | Purpose |
| --- | --- |
| search.rs | search namespace |
| search/content.rs | repository content grep |
| search/path.rs | path query ranking |
| search/view.rs | HTML fragments for search results |

## Repo

Git repository code sits behind the repo namespace.

| Path | Purpose |
| --- | --- |
| repo.rs | repo namespace |
| repo/diff.rs | unified diff engine |
| repo/history.rs | commit walk and file history |
| repo/refs.rs | refs and commit metadata |
| repo/remote.rs | remote URL and forge detection |
| repo/root.rs | repository discovery and source metadata |

## Stat

Repository statistics code sits behind the stat namespace.

| Path | Purpose |
| --- | --- |
| stat.rs | statistics report types, config, and resolver |
| stat/tools.rs | statistics tool registry |
| stat/tools/ | per-tool collectors for history, languages, license, and metadata |
| filesystem.rs | filesystem scan totals, sizes, and filter summaries |

## Cargo features

The default Cargo build enables every optional feature below.

| Capability | Optional | Cargo feature |
| --- | :---: | --- |
| File and directory browsing | | Core |
| Markdown rendering | | Core |
| Native file delivery | | Core |
| Path search | | Core |
| HTTP server | | Core |
| Directory archive generation and downloads | ✓ | `archive` |
| File-content search | ✓ | `content-search` |
| In-browser editing of text files, subject to runtime configuration | ✓ | `edit` |
| Local paste and stash support, subject to runtime configuration | ✓ | `gist` |
| Repository discovery, commit metadata, remotes, and file comparison | ✓ | `repo` |
| Repository statistics; enables `repo` | ✓ | `stats` |
| Filesystem watching and automatic browser updates | ✓ | `watch` |

**Example.** The following release build includes repository support, content search, source watching, and archives without statistics, gist, or edit support:

```bash
cargo build --locked --release --no-default-features \
  --features "archive,content-search,repo,watch"
```
