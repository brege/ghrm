use super::*;
use std::path::Path;

#[test]
fn mermaid_wraps_ghrm_block() {
    let md = "```mermaid\ngraph TD\n  A --> B\n```\n";
    let r = render_at(md, None);
    assert!(r.has_mermaid);
    assert!(r.html.contains(r#"<div class="ghrm-block ghrm-mermaid">"#));
    assert!(
        r.html
            .contains(r#"<div class="ghrm-mermaid-diagram"></div>"#)
    );
    assert!(r.html.contains(r#"<template class="ghrm-data">"#));
    assert!(!r.html.contains("language-mermaid"));
}

#[test]
fn geojson_wraps_ghrm_block() {
    let md = "```geojson\n{\"type\":\"FeatureCollection\"}\n```\n";
    let r = render_at(md, None);
    assert!(r.has_map);
    assert!(r.html.contains(r#"<div class="ghrm-block ghrm-geojson">"#));
    assert!(r.html.contains(r#"<div class="ghrm-map-canvas"></div>"#));
}

#[test]
fn topojson_wraps_ghrm_block() {
    let md = "```topojson\n{}\n```\n";
    let r = render_at(md, None);
    assert!(r.has_map);
    assert!(r.html.contains(r#"<div class="ghrm-block ghrm-topojson">"#));
}

#[test]
fn fenced_math_wraps_ghrm_math_block() {
    let md = "```math\nE = mc^2\n```\n";
    let r = render_at(md, None);
    assert!(r.has_math);
    assert!(r.html.contains(r#"<div class="ghrm-math-block">$$"#));
    assert!(r.html.contains("E = mc^2"));
}

#[test]
fn dollar_block_math_sets_flag() {
    let md = "$$\nE = mc^2\n$$\n";
    let r = render_at(md, None);
    assert!(r.has_math);
}

#[test]
fn alert_note_has_admonition_classes_and_svg() {
    let md = "> [!NOTE]\n> body\n";
    let r = render_at(md, None);
    assert!(
        r.html
            .contains(r#"markdown-admonition markdown-admonition-note"#)
    );
    assert!(r.html.contains(r#"markdown-admonition-title"#));
    assert!(r.html.contains(r#"class="octicon""#));
    assert!(!r.html.contains("markdown-alert-note"));
}

#[test]
fn regular_blockquote_unchanged() {
    let md = "> hello\n";
    let r = render_at(md, None);
    assert!(r.html.contains("<blockquote"));
    assert!(!r.html.contains("markdown-admonition"));
}

#[test]
fn emoji_shortcodes_render() {
    let md = ":wave: :rocket: :white_check_mark:\n";
    let r = render_at(md, None);
    assert!(r.html.contains("\u{1f44b}"));
    assert!(r.html.contains("\u{1f680}"));
    assert!(r.html.contains("\u{2705}"));
    assert!(!r.html.contains(":wave:"));
}

#[test]
fn code_block_plain_no_inline_style() {
    let md = "```python\nprint('hi')\n```\n";
    let r = render_at(md, None);
    assert!(r.html.contains(r#"<div class="highlight">"#));
    assert!(r.html.contains(r#"<pre tabindex="0" class="chroma">"#));
    assert!(
        r.html
            .contains(r#"<code class="language-python" data-lang="python">"#)
    );
    assert!(!r.html.contains("background-color:"));
}

#[test]
fn local_image_url_rewrites_from_source_dir() {
    let md = "![img](./assets/diagram.png)\n";
    let root = Path::new("/repo");
    let src = Path::new("/repo/docs/README.md");
    let r = render_at(md, Some(RenderPath { root, src }));
    assert!(r.html.contains(r#"src="/docs/assets/diagram.png""#));
}

#[test]
fn nested_dir_image_url_keeps_dir_prefix() {
    let md = "![img](docs/img/github.png)\n";
    let root = Path::new("/repo");
    let src = Path::new("/repo/arch/README.md");
    let r = render_at(md, Some(RenderPath { root, src }));
    assert!(r.html.contains(r#"src="/arch/docs/img/github.png""#));
}

#[test]
fn local_markdown_url_rewrites_from_source_dir() {
    let md = "[next](../guide/intro.md)\n";
    let root = Path::new("/repo");
    let src = Path::new("/repo/docs/start/README.md");
    let r = render_at(md, Some(RenderPath { root, src }));
    assert!(r.html.contains(r#"href="/docs/guide/intro.md""#));
}

#[test]
fn inline_math_delimited_for_katex() {
    let md = "text $\\sqrt{x}$ text\n";
    let r = render_at(md, None);
    assert!(r.has_math);
    assert!(r.html.contains(r"$\sqrt{x}$"));
    assert!(!r.html.contains("data-math-style"));
}

#[test]
fn github_backtick_math_preserves_delimiters() {
    let md = "text $`\\int x dx`$ text\n";
    let r = render_at(md, None);
    assert!(r.has_math);
    assert!(r.html.contains(r"$`\int x dx`$") || r.html.contains(r"$<code>\int x dx</code>$"));
    assert!(!r.html.contains("data-math-style"));
}

#[test]
fn title_from_h1() {
    let md = "# Hello World\n\nBody.\n";
    let r = render_at(md, None);
    assert_eq!(r.title, "Hello World");
}

#[test]
fn headings_get_hash_anchor_before_close() {
    let md = "## Taxonomy\n";
    let r = render_at(md, None);
    assert!(r.html.contains(r#"<h2 id="taxonomy">"#));
    assert!(r.html.contains(r#"class="ghrm-anchor""#));
    assert!(r.html.contains(r##"href="#taxonomy">#"##));
    assert!(!r.html.contains(r#"class="anchor""#));
}

#[test]
fn shebang_detection() {
    let cases = [
        ("#!/bin/bash\n", "bash"),
        ("#!/bin/sh\n", "sh"),
        ("#!/bin/zsh\n", "zsh"),
        ("#!/usr/bin/env python3\n", "python"),
        ("#!/usr/bin/env python3 -u\n", "python"),
        ("#!/usr/bin/env node\n", "javascript"),
        ("#!/usr/bin/env ruby\n", "ruby"),
        ("#!/usr/bin/perl\n", "perl"),
        ("#!/bin/awk -f\n", "awk"),
        ("#!/usr/bin/env lua\n", "lua"),
        ("#!/usr/bin/env deno run\n", "javascript"),
        ("#!/usr/local/bin/bash\n", "bash"),
    ];
    for (shebang, expected) in cases {
        let r = render_text("script", shebang);
        let needle = format!(r#"class="language-{expected}""#);
        assert!(
            r.html.contains(&needle),
            "shebang {shebang:?} should detect {expected}"
        );
    }

    let r = render_text("script.sh", "#!/bin/bash\n");
    assert!(
        r.html.contains(r#"class="language-sh""#),
        "extension wins over shebang"
    );

    let r = render_text("notes", "plain text\n");
    assert!(
        !r.html.contains("language-"),
        "no shebang or extension means no lang"
    );
}
