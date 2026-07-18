// Rendered Markdown is shaped here before frontend adapters enhance it.
// lol_html handles same-element rewrites, but parent rewrites that depend on
// child code elements stay explicit because :has() is not supported upstream.
mod alert;
mod anchor;
mod code;
mod math;
mod path;

use alert::rewrite_alerts;
use anchor::{HeadingAnchors, extract_title};
use code::{GhrmBlockAdapter, extract_langs, rewrite_code_blocks};
use comrak::options::Plugins;
use comrak::{Options, markdown_to_html_with_plugins};
use math::{GhrmMathAdapter, has_math_markers, rewrite_math_display, rewrite_math_spans};
pub use path::RenderPath;
use path::rewrite_local_urls;

static MERMAID_ADAPTER: GhrmBlockAdapter = GhrmBlockAdapter {
    class: "ghrm-mermaid",
    canvas: "ghrm-mermaid-diagram",
};
static GEOJSON_ADAPTER: GhrmBlockAdapter = GhrmBlockAdapter {
    class: "ghrm-geojson",
    canvas: "ghrm-map-canvas",
};
static TOPOJSON_ADAPTER: GhrmBlockAdapter = GhrmBlockAdapter {
    class: "ghrm-topojson",
    canvas: "ghrm-map-canvas",
};
static MATH_ADAPTER: GhrmMathAdapter = GhrmMathAdapter;

#[derive(serde::Serialize)]
pub struct Rendered {
    pub html: String,
    pub title: String,
    pub langs: Vec<String>,
    pub lang: Option<String>,
    pub has_mermaid: bool,
    pub has_math: bool,
    pub has_map: bool,
}

pub fn render_text(filename: &str, text: &str) -> Rendered {
    code::render_text(filename, text)
}

pub fn render_at(md: &str, path: Option<RenderPath<'_>>) -> Rendered {
    let mut options = Options::default();
    options.extension.alerts = true;
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.autolink = true;
    options.extension.math_dollars = true;
    options.extension.math_code = false;
    options.extension.shortcodes = true;
    options.extension.tagfilter = true;
    options.render.r#unsafe = true;
    options.render.github_pre_lang = false;

    let heading_anchors = HeadingAnchors::default();
    let mut plugins = Plugins::default();
    plugins.render.heading_adapter = Some(&heading_anchors);
    plugins
        .render
        .codefence_renderers
        .insert("mermaid".to_string(), &MERMAID_ADAPTER);
    plugins
        .render
        .codefence_renderers
        .insert("geojson".to_string(), &GEOJSON_ADAPTER);
    plugins
        .render
        .codefence_renderers
        .insert("topojson".to_string(), &TOPOJSON_ADAPTER);
    plugins
        .render
        .codefence_renderers
        .insert("math".to_string(), &MATH_ADAPTER);

    let raw = markdown_to_html_with_plugins(md, &options, &plugins);
    let html = rewrite_math_display(&raw);
    let html = rewrite_math_spans(&html);
    let html = rewrite_alerts(&html);
    let html = rewrite_code_blocks(&html);
    let langs = extract_langs(&html);
    let title = extract_title(&html).unwrap_or_else(|| "Preview".to_string());
    let html = match path {
        Some(path) => rewrite_local_urls(&html, path),
        None => html,
    };

    let flags = Flags {
        mermaid: html.contains("ghrm-mermaid"),
        math: has_math_markers(md, &html),
        map: html.contains("ghrm-geojson") || html.contains("ghrm-topojson"),
    };

    Rendered {
        html,
        title,
        langs,
        lang: None,
        has_mermaid: flags.mermaid,
        has_math: flags.math,
        has_map: flags.map,
    }
}

#[derive(Default)]
struct Flags {
    mermaid: bool,
    math: bool,
    map: bool,
}

#[cfg(test)]
mod tests;
