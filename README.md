<p align="center">
  <a href="https://github.com/brege/ghrm"><img src="assets/img/favicon.svg" width="96" height="96" alt="ghrm logo"></a>
  <h1 align="center">ghrm</h1>
</p>

<p align="center">
  <a href="https://github.com/brege/ghrm/actions/workflows/ci.yml"><img src="https://github.com/brege/ghrm/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0-blue" alt="License"></a>
</p>

Ghrm supports all **G**it**H**ub-flavored Markdown features, **R**ead**m**e and source code viewing, detects repositories, has instant file retrieval, and works completely offline. It makes file browsing and file reading feel more continuous and much easier to work on projects that mix images, HTML, large tables, JSON, and other assets your browser handles well. You get both the backend performance of Rust and the multimedia performance of your browser.

## Supported Features

- **Works offline**
- Live reloading
- Preview Markdown, source code, and images
- Light-and-dark theme
- Chromeless PDF printing

### File explorer

- Search instantly by path and content
- Git history metadata
- Filters for gitignores, excludes, custom groups
- Sorting and breadcrumb navigation

### Markdown

- Syntax highlighting
- Admonitions `[!NOTE]`
- Mermaid diagrams
- KaTeX math
- GeoJSON and TopoJSON maps (exp.)

### Terminal

```bash
ghrm .
```

### Neovim

```lua
:Ghrm
```

## Ethos

I made Ghrm because when I lose internet/power, I often turn toward documentation and repo-gardening to stay occupied while online resources are unavailable. There's zero mystery what your docs are going to look like after you push.

> [!IMPORTANT]
> On first run, ghrm locally downloads the browser libraries from CDNs and (outside of sources or maps you may already have embedded in your Markdown file) never touches the internet again. It renders Markdown and file trees the way GitHub does.

> [!NOTE]
> Ghrm is non-mutating. It's not meant to be a general git repo manager.

You can use `--bind 0.0.0.0` to connect to a ghrm instance from other devices in your network. It's automatically password protected, set via [`config.toml`](config.example.toml).

## Install

### Binary (recommended)

Download from the [releases page](https://github.com/brege/ghrm/releases/latest), extract, and add to PATH.

### From source

```bash
cargo install --git https://github.com/brege/ghrm ghrm
```

crates.io coming soon.

## Usage

One file.
```bash
ghrm README.md
```

Multiple files, recursively.
```bash
ghrm ~/src
```

Opens a live-reloading preview in your browser. Edits to the file in your editor are reflected automatically on save.

## Neovim

Add to your lazy.nvim config:

```lua
{ "brege/ghrm" }
```

Commands: `:Ghrm` to start, `:GhrmStop` to stop, or just exit nvim.

### Examples

- [Basics](smoke/basics.md)
- [Diagrams](smoke/diagrams.md)
- [Languages](smoke/languages.md)

```bash
ghrm smoke/basics.md
ghrm smoke/diagrams.md
ghrm smoke/languages.md
```

## Uninstall

```bash
ghrm --clean
cargo uninstall ghrm
```

## Inspiration

- [fd](https://github.com/sharkdp/fd)
- [ripgrep](https://github.com/BurntSushi/ripgrep)
- [tokei](https://github.com/XAMPPRocky/tokei)
- [onefetch](https://github.com/o2sh/onefetch)

## Reference

- [Server](src/README.md)
- [UI](ui/README.md)
- [Benchmarks](bench/README.md)

## License

[GPL-3.0](LICENSE)
