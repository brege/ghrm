# ui

The UI workspace owns maintained browser source, icon declarations, and the validation and release steps that turn `ui/src/` into the runtime files served at `/_ghrm/assets/js/...`. Stable browser URLs stay under `assets/js`, but the maintained source lives under `ui/`.

## Root

| Path | Purpose |
| --- | --- |
| package.json | workspace scripts and Node `>=24` and npm `>=11.10` engine floor |
| tsconfig.json | TypeScript config for source, icons, and tests |
| vite.config.ts | build graph from `ui/src/` to `assets/js/` plus chunk layout and sprite emission |
| vitest.config.ts | Happy DOM test harness |
| icons.tsx | reviewed icon declarations that generate `assets/js/icons.svg` |
| scripts/check.ts | build output verification and release archive packing |
| scripts/icons.ts | icon sprite generation and icon contract checks |
| types/ghrm.d.ts | browser globals and project type declarations |

## Entries

| Path | Purpose |
| --- | --- |
| src/main.ts | full document entry for browser features and custom elements |
| src/preview.ts | preview-only entry |
| src/gist.ts | gist page entry |

## Runtime

Root modules coordinate across features; feature behavior lives under the folder that owns it.

| Path | Purpose |
| --- | --- |
| src/features.ts | ordered browser feature registry |
| src/runtime.ts | initial and refresh lifecycle runner used by htmx refreshes and tests |
| src/dom.ts | shared DOM helpers |
| src/live.ts | live reload behavior |
| src/vendor.ts | vendor asset loading plan |
| src/adapters/* | page-specific adapters for copy, math, mermaid, and map behavior |

## Feature Boundaries

| Path | Purpose |
| --- | --- |
| src/editor/ | shared editing surface, inline rename lifecycle, and tab indentation |
| src/explorer/ | explorer page setup, controls, path copy, and edit controls |
| src/file/ | rendered file view, inline file editing, compare bar, and table of contents |
| src/gist/ | gist editor and stash islands |
| src/search/ | search coordination and the search panel island |
| src/shell/ | boosted navigation, server status, preferences, and the shared menu island |
| src/archive/ | archive download progress island |

The global `ghrm-menus` island controls elements that declare `data-ghrm-menu-toggle`, `data-ghrm-menu-panel`, and optional nested `data-ghrm-menu-disclosure` attributes. It owns open state, dismissal, focus restoration, and floating-panel positioning. About and statistics expansions retain their dedicated behavior.

## Tests

`src/test/` mirrors the runtime layout. Vitest runs `src/**/*.test.ts` in Happy DOM and covers the runtime registry, document setup, and Lit island contracts.

## Authored Source And Generated Runtime

- `ui/src/` is maintained TypeScript only. `ui/scripts/check.ts` fails the build if `.js` source files remain under `ui/src/`.
- Vite writes disposable ES module output to ignored `assets/js/` using stable entry names `preview.js`, `main.js`, and `gist.js`, shared chunks under `assets/js/chunks/`, and the generated `icons.svg` sprite.
- `assets/templates/` and Rust templates keep pointing at `/_ghrm/assets/js/...`. The build keeps those URLs stable even though the maintained source moved under `ui/`.
- Generated runtime files are banner-marked as generated. Edit `ui/src/` or `icons.tsx`, not `assets/js/`.

## Daily Workflows

- `just ui` runs the default validation gate before review or release work.
- `just ui-test` runs the UI test suite only.
- `just ui-build` verifies the Vite output shape in a disposable build directory and does not refresh tracked release artifacts.
- `just ui-watch` keeps runtime builds fresh while editing browser code.
- `just dev-ui <PATH>` runs one runtime build, starts the Vite watcher, and then runs the Rust server without a browser.

## Checks

- `just ui` is the default UI gate. It runs Biome, TypeScript, Vitest with Happy DOM, icon validation, and a Vite build check without rewriting tracked release artifacts.
- `ui/scripts/check.ts` is the runtime asset contract. It validates expected entry files, chunk files, source language boundaries, and archive parity.
- `ui/scripts/icons.ts` is the icon contract gate. It validates icon declarations, generated sprite shape, and consumer coverage.

## Release Refresh

- `assets/js/` is ignored disposable build output for local development and source checks.
- `assets/js.tar.zst` and `assets/js.sha256.json` are the tracked release bundle and manifest that Rust serves.
- `just ui-release` is the main-only path that refreshes the tracked archive and manifest after `ui/` source changes.
- `ui/scripts/check.ts` owns the contract between disposable `assets/js/` output and the tracked archive: `source` checks a fresh build, `pack` refreshes the archive and manifest, and the default check compares a fresh build against the tracked bundle.

## Template And Island Boundary

[`assets/templates/`](../assets/templates/) and Rust view models still own document structure and data contracts. TypeScript runtime modules and Lit islands enhance behavior after render, so structural changes should move through templates and Rust data providers instead of shifting markup into browser code.

Template authoring rules for Askama formatting and macro extraction live in [assets/templates/README.md](../assets/templates/README.md).

## Icons

- `ui/icons.tsx` is the reviewed declaration source for runtime icons.
- `ui/scripts/icons.ts` turns those declarations into the generated sprite contract.
- `assets/js/icons.svg` is generated runtime output and should not be hand-edited.
