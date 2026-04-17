# ghrm

Preview GitHub-flavored Markdown locally, offline, and in your browser. It renders admonitions, Mermaid diagrams, KaTeX math, GeoJSON/TopoJSON maps, and light/dark theme toggling all whilst matching GitHub's README style.

## Ethos

People who lose internet and power often: this tool is for you. It renders Markdown the exact same way as GitHub would. If you are offline and still need to make meaningful contributions, focusing on documentation is often the cromulent choice when online resources are unavailable.

## Install

```bash
cargo install --path .

# or directly from git
cargo install --git https://github.com/brege/ghrm-rs ghrm
```

## Usage

```bash
# one file
ghrm README.md

# multiple files, recursively
ghrm .
```

Opens a live-reloading preview in your browser. Edits to the file are reflected automatically on saves.

## Neovim

Add to your lazy.nvim config:

```lua
{ "brege/ghrm", ft = "markdown", config = function() require("ghrm").setup() end }
```

Commands: `:Ghrm` to start, `:GhrmStop` to stop, or just exit nvim.

## Supported Features

- **Works offline**
- GitHub alert admonitions (`[!NOTE]`, `[!TIP]`, `[!WARNING]`, etc.)
- Collapsible `<details>` sections and normal Markdown formatting and highlighting
- Mermaid diagrams
- KaTeX math (inline, display, and fenced `math` blocks)
- GeoJSON and TopoJSON maps
- Light/dark theme toggle

### Examples

- [Basics](smoke/basics.md)
- [Diagrams](smoke/diagrams.md)

```bash
ghrm README.md
ghrm smoke/basics.md
ghrm smoke/diagrams.md
ghrm .
```

## Uninstall

```bash
cargo uninstall ghrm
rm --recursive ~/.cache/ghrm
```
