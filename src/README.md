# src

The Rust source tree keeps app-wide contracts flat and groups feature code under namespace files.

## Root

Shared app contracts and binary-level support stay flat.

| Path | Purpose |
| --- | --- |
| main.rs | binary entry |
| config.rs | config schema and loading |
| dirs.rs | OS app directories |
| paths.rs | path safety helpers |
| runtime.rs | runtime paths shown in UI |
| repo.rs | git source metadata |
| render.rs | Markdown and source rendering |
| tmpl.rs | Askama template structs |

## HTTP

Local web-serving code sits behind the HTTP namespace.

| Path | Purpose |
| --- | --- |
| http.rs | HTTP namespace |
| http/server.rs | Axum router and server state |
| http/api.rs | JSON and fragment API routes |
| http/auth.rs | auth middleware and handlers |
| http/delivery.rs | native file delivery |
| http/shell.rs | full page and fragment responses |
| http/theme.rs | embedded app asset cache |
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
