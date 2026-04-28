# ghrm

Explore your filesystem and render Markdown as if it were on GitHub.

Ghrm supports all **G**it**H**ub-flavored Markdown features, **R**ead**m**e and source code viewing, detects repositories, has instant file retrieval, and works 100% **offline**. It makes file browsing and file reading feel more continuous and much easier to work on projects mixing images, HTML, large tables and JSON and other assets in your browser. You get both the backend performance of Rust and the multimedia performance of your browser.

## Supported Features

- **Works offline**
- File explorer with [fd](https://github.com/sharkdp/fd) semantics
- Syntax highlighting
- Focus filters for Markdown, source, and hidden files
- Admonitions `[!NOTE]`
- Mermaid diagrams
- KaTeX math
- GeoJSON and TopoJSON maps
- Light/dark theme toggle

## Ethos

I made Ghrm because when I lose internet/power, I often turn toward documentation and repo-gardening to stay occupied while online resources are unavailable. It locally downloads the JavaScript libraries on first install and never touches the internet again, except for URL sources that may already be in your Markdown.  It renders Markdown and file trees the exact same way as GitHub would so there's zero mystery how your documentation is going to look like on GitHub.

Ghrm is not meant to be a general git repository manager.

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
ghrm .       # ~, ~/Documents, ~/.config, etc
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
ghrm README.md
ghrm smoke/basics.md
ghrm smoke/diagrams.md
ghrm .
```

## Uninstall

```bash
cargo uninstall ghrm
rm -r ~/.cache/ghrm
```

## Roadmap

- Add metadata configuration in header-actions (test these out one by one and bench)
  - last commit message for each file/directory
  - fs timestamp date or git timestamp cwitch for each file/directory
  - line count for files
  - size on disk of file or directory
- Polish
  - favicon.svg
  - demo.webm

## License

[GPL-3.0](LICENSE)
