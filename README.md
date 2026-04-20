# ghrm

Preview GitHub-flavored Markdown locally, offline, and in your browser. It renders admonitions, Mermaid diagrams, KaTeX math, GeoJSON/TopoJSON maps, and light/dark theme toggling all whilst matching GitHub's README and File Explorer style.

## Ethos

People who lose internet and power often: this tool is for you. It renders Markdown the exact same way as GitHub would. If you are offline and still need to make meaningful contributions to your projects, focusing on documentation is often the cromulent choice when online resources are unavailable.

## Install

```bash
cargo install --git https://github.com/brege/ghrm ghrm
```

## Usage

```bash
# one file
ghrm README.md

# multiple files, recursively
ghrm .
```

Opens a live-reloading preview in your browser. Edits to the file in your editor are reflected automatically on save.

## Neovim

Add to your lazy.nvim config:

```lua
{ "brege/ghrm", ft = "markdown", config = function() require("ghrm").setup() end }
```

Commands: `:Ghrm` to start, `:GhrmStop` to stop, or just exit nvim.

## Supported Features

- **Works offline**
- Syntax highlighting
- Focus filters for Markdown, source, and hidden files
- GitHub alert admonitions (`[!NOTE]`, `[!TIP]`, `[!WARNING]`, etc.)
- Collapsible `<details>` sections and normal Markdown formatting and highlighting
- Mermaid diagrams
- KaTeX math (inline, display, and fenced `math` blocks)
- GeoJSON and TopoJSON maps
- Light/dark theme toggle

### Examples

- [Basics](smoke/basics.md)
- [Diagrams](smoke/diagrams.md)
- [Languages](smoke/languages.md)

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

## Roadmap

- add code numbers to file-view on non-markdown files
- detect languages via shebang
- build up config.toml capabilities
- add toggle for gitignore'd items in UI

## License

[GPL-3.0](LICENSE)
