use comrak::adapters::CodefenceRendererAdapter;
use comrak::nodes::Sourcepos;
use comrak::options::Plugins;
use comrak::{Options, markdown_to_html_with_plugins};
use std::fmt;
use std::path::{Component, Path, PathBuf};

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

pub struct Rendered {
    pub html: String,
    pub title: String,
    pub has_mermaid: bool,
    pub has_math: bool,
    pub has_map: bool,
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
    options.extension.header_id_prefix = Some(String::new());
    options.extension.tagfilter = false;
    options.render.r#unsafe = true;
    options.render.github_pre_lang = false;

    let mut plugins = Plugins::default();
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
    let html = match path {
        Some(path) => rewrite_local_urls(&html, path),
        None => html,
    };

    let flags = Flags {
        mermaid: html.contains("ghrm-mermaid"),
        math: has_math_markers(md, &html),
        map: html.contains("ghrm-geojson") || html.contains("ghrm-topojson"),
    };

    let title = extract_title(&html).unwrap_or_else(|| "Preview".to_string());

    Rendered {
        html,
        title,
        has_mermaid: flags.mermaid,
        has_math: flags.math,
        has_map: flags.map,
    }
}

#[derive(Clone, Copy)]
pub struct RenderPath<'a> {
    pub root: &'a Path,
    pub src: &'a Path,
}

#[derive(Default)]
struct Flags {
    mermaid: bool,
    math: bool,
    map: bool,
}

fn has_math_markers(md: &str, html: &str) -> bool {
    if html.contains("ghrm-math-block") {
        return true;
    }
    // presence of any $...$ or $$...$$ or $`...`$ in source is sufficient;
    // comrak's math_dollars/math_code have already validated balanced pairs
    // by converting to spans, which our rewrite pass turned back into delimiters.
    md.contains("$$") || md.contains("$`") || has_inline_dollar_math(md)
}

fn has_inline_dollar_math(md: &str) -> bool {
    let mut chars = md.chars().peekable();
    let mut seen = 0u32;
    while let Some(c) = chars.next() {
        if c == '\\' {
            chars.next();
            continue;
        }
        if c == '$' {
            seen += 1;
            if seen >= 2 {
                return true;
            }
        }
    }
    false
}

struct GhrmBlockAdapter {
    class: &'static str,
    canvas: &'static str,
}

impl CodefenceRendererAdapter for GhrmBlockAdapter {
    fn write(
        &self,
        out: &mut dyn fmt::Write,
        _lang: &str,
        _meta: &str,
        code: &str,
        _sp: Option<Sourcepos>,
    ) -> fmt::Result {
        let escaped = html_escape::encode_text(code);
        write!(
            out,
            r#"<div class="ghrm-block {cls}"><div class="{canvas}"></div><template class="ghrm-data">{body}</template></div>"#,
            cls = self.class,
            canvas = self.canvas,
            body = escaped,
        )
    }
}

struct GhrmMathAdapter;

impl CodefenceRendererAdapter for GhrmMathAdapter {
    fn write(
        &self,
        out: &mut dyn fmt::Write,
        _lang: &str,
        _meta: &str,
        code: &str,
        _sp: Option<Sourcepos>,
    ) -> fmt::Result {
        let escaped = html_escape::encode_text(code.trim_end_matches('\n'));
        write!(
            out,
            r#"<div class="ghrm-math-block">$$
{}
$$</div>"#,
            escaped
        )
    }
}

fn rewrite_math_spans(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    loop {
        let inline = rest.find(r#"<span data-math-style="inline">"#);
        let display = rest.find(r#"<span data-math-style="display">"#);
        let code_inline = rest.find(r#"<code data-math-style="inline">"#);
        let code_display = rest.find(r#"<code data-math-style="display">"#);

        let next = [inline, display, code_inline, code_display]
            .into_iter()
            .flatten()
            .min();
        let Some(idx) = next else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..idx]);
        let at = &rest[idx..];

        let (open_tag, close_tag, delim) = if at.starts_with(r#"<span data-math-style="inline">"#) {
            (r#"<span data-math-style="inline">"#, "</span>", "$")
        } else if at.starts_with(r#"<span data-math-style="display">"#) {
            (r#"<span data-math-style="display">"#, "</span>", "$$")
        } else if at.starts_with(r#"<code data-math-style="inline">"#) {
            (r#"<code data-math-style="inline">"#, "</code>", "$")
        } else {
            (r#"<code data-math-style="display">"#, "</code>", "$$")
        };

        let after_open = &at[open_tag.len()..];
        let Some(close_idx) = after_open.find(close_tag) else {
            out.push_str(at);
            break;
        };
        let body = &after_open[..close_idx];
        out.push_str(delim);
        out.push_str(body);
        out.push_str(delim);
        rest = &after_open[close_idx + close_tag.len()..];
    }
    out
}

fn rewrite_math_display(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    loop {
        let Some(idx) = rest.find("<pre><code") else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..idx]);
        let at = &rest[idx..];
        let Some(code_idx) = at.find("<code") else {
            out.push_str(at);
            break;
        };
        let Some(code_end) = at[code_idx..].find('>') else {
            out.push_str(at);
            break;
        };
        let open_tag = &at[code_idx..=code_idx + code_end];
        if !open_tag.contains(r#"class="language-math""#) {
            out.push_str(&at[..code_idx + code_end + 1]);
            rest = &at[code_idx + code_end + 1..];
            continue;
        }
        let after_open = &at[code_idx + code_end + 1..];
        let Some(close_idx) = after_open.find("</code></pre>") else {
            out.push_str(at);
            break;
        };
        let body = &after_open[..close_idx];
        let body_trimmed = body.trim_end_matches('\n');
        out.push_str(
            r#"<div class="ghrm-math-block">$$
"#,
        );
        out.push_str(body_trimmed);
        out.push_str("\n$$</div>");
        rest = &after_open[close_idx + "</code></pre>".len()..];
    }
    out
}

fn rewrite_alerts(html: &str) -> String {
    let mut out = html.to_string();
    let kinds = ["note", "tip", "important", "warning", "caution"];
    for k in kinds {
        let needle = format!(r#"<div class="markdown-alert markdown-alert-{k}">"#);
        let replacement = format!(r#"<div class="markdown-admonition markdown-admonition-{k}">"#);
        out = out.replace(&needle, &replacement);
    }
    for k in kinds {
        let needle = format!(r#"<p class="markdown-alert-title">{}</p>"#, titlecase(k));
        let replacement = format!(
            r#"<p class="markdown-admonition-title">{svg}{title}</p>"#,
            svg = octicon_for(k),
            title = titlecase(k),
        );
        out = out.replace(&needle, &replacement);
    }
    out
}

fn rewrite_code_blocks(html: &str) -> String {
    let mut out = String::with_capacity(html.len() + 128);
    let mut rest = html;

    loop {
        let Some(pre_idx) = rest.find("<pre><code") else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..pre_idx]);
        let at = &rest[pre_idx..];
        let Some(code_idx) = at.find("<code") else {
            out.push_str(at);
            break;
        };
        let Some(code_end) = at[code_idx..].find('>') else {
            out.push_str(at);
            break;
        };
        let open_tag = &at[code_idx..=code_idx + code_end];
        let Some(close_idx) = at.find("</code></pre>") else {
            out.push_str(at);
            break;
        };
        let body = &at[code_idx + code_end + 1..close_idx];

        let lang = code_lang(open_tag);
        out.push_str(r#"<div class="highlight"><pre tabindex="0" class="chroma"><code"#);
        if let Some(lang) = lang {
            out.push_str(r#" class="language-"#);
            out.push_str(lang);
            out.push_str(r#"" data-lang=""#);
            out.push_str(lang);
            out.push('"');
        }
        out.push('>');
        out.push_str(body);
        out.push_str("</code></pre></div>");
        rest = &at[close_idx + "</code></pre>".len()..];
    }

    out
}

fn code_lang(open_tag: &str) -> Option<&str> {
    let marker = r#"class="language-"#;
    let start = open_tag.find(marker)? + marker.len();
    let rest = &open_tag[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

fn rewrite_local_urls(html: &str, path: RenderPath<'_>) -> String {
    let mut out = html.to_string();
    for attr in ["href", "src"] {
        out = rewrite_attr_urls(&out, attr, path);
    }
    out
}

fn rewrite_attr_urls(html: &str, attr: &str, path: RenderPath<'_>) -> String {
    let marker = format!(r#"{attr}=""#);
    let mut out = String::with_capacity(html.len());
    let mut rest = html;

    loop {
        let Some(idx) = rest.find(&marker) else {
            out.push_str(rest);
            break;
        };
        let value_start = idx + marker.len();
        out.push_str(&rest[..value_start]);
        let after = &rest[value_start..];
        let Some(end) = after.find('"') else {
            out.push_str(after);
            break;
        };
        let value = &after[..end];
        out.push_str(&local_url(path, value));
        rest = &after[end..];
    }

    out
}

fn local_url(path: RenderPath<'_>, dest: &str) -> String {
    if is_external_url(dest) || dest.starts_with('#') {
        return dest.to_string();
    }

    let (target, suffix) = split_suffix(dest);
    if target.is_empty() || target.starts_with('/') {
        return dest.to_string();
    }

    let Some(rel) = resolve_target(path.root, path.src, target) else {
        return dest.to_string();
    };

    let mut out = String::from("/");
    out.push_str(&rel.to_string_lossy().replace('\\', "/"));
    out.push_str(suffix);
    out
}

fn is_external_url(dest: &str) -> bool {
    if dest.starts_with("//") {
        return true;
    }
    let end = dest.find(['/', '?', '#']).unwrap_or(dest.len());
    dest[..end].contains(':')
}

fn split_suffix(dest: &str) -> (&str, &str) {
    let idx = dest.find(['?', '#']).unwrap_or(dest.len());
    (&dest[..idx], &dest[idx..])
}

fn resolve_target(root: &Path, src: &Path, target: &str) -> Option<PathBuf> {
    let src_dir = src.parent()?;
    let mut rel = src_dir.strip_prefix(root).ok()?.to_path_buf();

    for comp in Path::new(target).components() {
        match comp {
            Component::CurDir => {}
            Component::Normal(part) => rel.push(part),
            Component::ParentDir => {
                if !rel.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    Some(rel)
}

fn titlecase(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn octicon_for(kind: &str) -> &'static str {
    match kind {
        "note" => {
            r#"<svg class="octicon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"><path d="M0 8a8 8 0 1 1 16 0A8 8 0 0 1 0 8Zm8-6.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13ZM6.5 7.75A.75.75 0 0 1 7.25 7h1a.75.75 0 0 1 .75.75v2.75h.25a.75.75 0 0 1 0 1.5h-2a.75.75 0 0 1 0-1.5h.25v-2h-.25a.75.75 0 0 1-.75-.75ZM8 6a1 1 0 1 1 0-2 1 1 0 0 1 0 2Z"></path></svg>"#
        }
        "tip" => {
            r#"<svg class="octicon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"><path d="M8 1.5c-2.363 0-4 1.69-4 3.75 0 .984.424 1.625.984 2.304l.214.253c.223.264.47.556.673.848.284.411.537.896.621 1.49a.75.75 0 0 1-1.484.211c-.04-.282-.163-.547-.37-.847a8.456 8.456 0 0 0-.542-.68c-.084-.1-.173-.205-.268-.32C3.201 7.75 2.5 6.766 2.5 5.25 2.5 2.31 4.863 0 8 0s5.5 2.31 5.5 5.25c0 1.516-.701 2.5-1.328 3.259-.095.115-.184.22-.268.319-.207.245-.383.453-.541.681-.208.3-.33.565-.37.847a.751.751 0 0 1-1.485-.212c.084-.593.337-1.078.621-1.489.203-.292.45-.584.673-.848.075-.088.147-.173.213-.253.561-.679.985-1.32.985-2.304 0-2.06-1.637-3.75-4-3.75ZM5.75 12h4.5a.75.75 0 0 1 0 1.5h-4.5a.75.75 0 0 1 0-1.5ZM6 15.25a.75.75 0 0 1 .75-.75h2.5a.75.75 0 0 1 0 1.5h-2.5a.75.75 0 0 1-.75-.75Z"></path></svg>"#
        }
        "important" => {
            r#"<svg class="octicon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"><path d="M0 1.75C0 .784.784 0 1.75 0h12.5C15.216 0 16 .784 16 1.75v9.5A1.75 1.75 0 0 1 14.25 13H8.06l-2.573 2.573A1.458 1.458 0 0 1 3 14.543V13H1.75A1.75 1.75 0 0 1 0 11.25Zm1.75-.25a.25.25 0 0 0-.25.25v9.5c0 .138.112.25.25.25h2a.75.75 0 0 1 .75.75v2.19l2.72-2.72a.749.749 0 0 1 .53-.22h6.5a.25.25 0 0 0 .25-.25v-9.5a.25.25 0 0 0-.25-.25Zm7 2.25v2.5a.75.75 0 0 1-1.5 0v-2.5a.75.75 0 0 1 1.5 0ZM9 9a1 1 0 1 1-2 0 1 1 0 0 1 2 0Z"></path></svg>"#
        }
        "warning" => {
            r#"<svg class="octicon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"><path d="M6.457 1.047c.659-1.234 2.427-1.234 3.086 0l6.082 11.378A1.75 1.75 0 0 1 14.082 15H1.918a1.75 1.75 0 0 1-1.543-2.575Zm1.763.707a.25.25 0 0 0-.44 0L1.698 13.132a.25.25 0 0 0 .22.368h12.164a.25.25 0 0 0 .22-.368Zm.53 3.996v2.5a.75.75 0 0 1-1.5 0v-2.5a.75.75 0 0 1 1.5 0ZM9 11a1 1 0 1 1-2 0 1 1 0 0 1 2 0Z"></path></svg>"#
        }
        "caution" => {
            r#"<svg class="octicon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"><path d="M4.47.22A.749.749 0 0 1 5 0h6c.199 0 .389.079.53.22l4.25 4.25c.141.14.22.331.22.53v6a.749.749 0 0 1-.22.53l-4.25 4.25A.749.749 0 0 1 11 16H5a.749.749 0 0 1-.53-.22L.22 11.53A.749.749 0 0 1 0 11V5c0-.199.079-.389.22-.53Zm.84 1.28L1.5 5.31v5.38l3.81 3.81h5.38l3.81-3.81V5.31L10.69 1.5ZM8 4a.75.75 0 0 1 .75.75v3.5a.75.75 0 0 1-1.5 0v-3.5A.75.75 0 0 1 8 4Zm0 8a1 1 0 1 1 0-2 1 1 0 0 1 0 2Z"></path></svg>"#
        }
        _ => "",
    }
}

pub fn extract_title(html: &str) -> Option<String> {
    let open = html.find("<h1")?;
    let gt = html[open..].find('>')? + open + 1;
    let close = html[gt..].find("</h1>")? + gt;
    let inner = &html[gt..close];
    Some(strip_tags(inner).trim().to_string())
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    html_escape::decode_html_entities(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // preview.js configures KaTeX with `$\`...\`$` delimiters and a
        // restoreGitHubInlineMath pass that also handles `$<code>...</code>$`.
        // Either shape is fine; client does the rendering.
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
}
