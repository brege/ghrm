<p align="center">
  <a href="https://github.com/brege/ghrm"><img src="assets/img/favicon.svg" width="96" height="96" alt="ghrm logo"></a>
  <h1 align="center">ghrm</h1>
</p>

Explore your filesystem as if it were on GitHub.

Ghrm supports all **G**it**H**ub-flavored Markdown features, **R**ead**m**e and source code viewing, detects repositories, has instant file retrieval, and works completely offline. It makes file browsing and file reading feel more continuous and much easier to work on projects mixing images, HTML, large tables and JSON and other assets in your browser. You get both the backend performance of Rust and the multimedia performance of your browser.

## Supported Features

- **Works offline**
- Live reloading
- Preview Markdown, source code, and images
- Light-and-dark theme

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

I made Ghrm because when I lose internet/power, I often turn toward documentation and repo-gardening to stay occupied while online resources are unavailable. On first run, ghrm locally downloads the browser libraries from CDNs and, outside of sources or maps you may already have in your Markdown, never touches the internet again. It renders Markdown and file trees the exact same way as GitHub does. There's zero mystery what your docs are going to look like after you push.

Ghrm is non-mutating. It's not meant to be a general git repo manager.

You can use `--bind 0.0.0.0` to connect to a ghrm instance from other devices in your network. It's automatically password protected, set via [`config.toml`](config.example.toml).

## Install

```bash
cargo install --git https://github.com/brege/ghrm ghrm
```

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
{ "brege/ghrm" end }
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
- [Benchmarks](bench/README.md)

## License

[GPL-3.0](LICENSE)
