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

#[derive(serde::Serialize)]
pub struct Rendered {
    pub html: String,
    pub title: String,
    pub has_mermaid: bool,
    pub has_math: bool,
    pub has_map: bool,
}

pub fn render_text(filename: &str, text: &str) -> Rendered {
    let lang = Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .or_else(|| detect_shebang(text));
    let escaped = html_escape::encode_text(text);
    Rendered {
        title: filename.to_string(),
        html: code_block_html(lang, &escaped),
        has_mermaid: false,
        has_math: false,
        has_map: false,
    }
}

fn detect_shebang(text: &str) -> Option<&'static str> {
    let line = text.lines().next()?;
    if !line.starts_with("#!") {
        return None;
    }
    let line = line.trim_start_matches("#!");
    let mut words = line.split_whitespace();
    let first = words.next()?;

    if first.ends_with("/env") {
        let interp = words.next()?;
        return lang_from_interpreter(interp);
    }

    let bin = first.rsplit('/').next()?;
    lang_from_interpreter(bin)
}

fn lang_from_interpreter(interp: &str) -> Option<&'static str> {
    match interp {
        "sh" => Some("sh"),
        "bash" => Some("bash"),
        "zsh" => Some("zsh"),
        "ksh" => Some("ksh"),
        "csh" | "tcsh" => Some("csh"),
        "fish" => Some("fish"),
        "dash" => Some("sh"),
        "ash" => Some("sh"),
        _ if interp.starts_with("python") => Some("python"),
        _ if interp.starts_with("ruby") => Some("ruby"),
        _ if interp.starts_with("perl") => Some("perl"),
        "node" | "nodejs" | "deno" | "bun" => Some("javascript"),
        "lua" | "luajit" => Some("lua"),
        "php" => Some("php"),
        "Rscript" => Some("r"),
        "awk" | "gawk" | "mawk" | "nawk" => Some("awk"),
        "sed" | "gsed" => Some("sed"),
        "make" | "gmake" => Some("makefile"),
        "tclsh" | "wish" => Some("tcl"),
        "osascript" => Some("applescript"),
        "pwsh" | "powershell" => Some("powershell"),
        "groovy" => Some("groovy"),
        "elixir" => Some("elixir"),
        "escript" => Some("erlang"),
        "crystal" => Some("crystal"),
        "julia" => Some("julia"),
        "nim" | "nimble" => Some("nim"),
        "dart" => Some("dart"),
        "swift" => Some("swift"),
        "scala" => Some("scala"),
        "sbcl" | "clisp" | "ecl" => Some("lisp"),
        "racket" => Some("racket"),
        "guile" | "scheme" => Some("scheme"),
        "runhaskell" | "runghc" => Some("haskell"),
        "ocaml" | "ocamlrun" => Some("ocaml"),
        _ => None,
    }
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
    let title = extract_title(&html).unwrap_or_else(|| "Preview".to_string());
    let html = rewrite_heading_anchors(&html);
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
        let icon = octicon_for(k);
        let svg = if icon.is_empty() {
            String::new()
        } else {
            format!(
                "<svg class=\"octicon\" width=\"16\" height=\"16\" aria-hidden=\"true\"><use href=\"#ghrm-icon-{icon}\"></use></svg>"
            )
        };
        let replacement = format!(
            r#"<p class="markdown-admonition-title">{svg}{title}</p>"#,
            title = titlecase(k),
        );
        out = out.replace(&needle, &replacement);
    }
    out
}

fn code_block_html(lang: Option<&str>, body: &str) -> String {
    let attrs = lang
        .map(|l| format!(r#" class="language-{l}" data-lang="{l}""#))
        .unwrap_or_default();
    format!(
        r#"<div class="highlight"><pre tabindex="0" class="chroma"><code{attrs}>{body}</code></pre></div>"#
    )
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
        out.push_str(&code_block_html(lang, body));
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
        "note" => "note",
        "tip" => "tip",
        "important" => "important",
        "warning" => "warning",
        "caution" => "caution",
        _ => "",
    }
}

fn rewrite_heading_anchors(html: &str) -> String {
    let mut out = String::with_capacity(html.len() + 512);
    let mut rest = html;
    loop {
        let next = (1u8..=6)
            .filter_map(|n| rest.find(&format!("<h{n}")).map(|i| (i, n)))
            .min_by_key(|(i, _)| *i);
        let Some((idx, level)) = next else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..idx]);
        let at = &rest[idx..];
        let Some(open_end) = at.find('>') else {
            out.push_str(at);
            break;
        };
        let close_tag = format!("</h{level}>");
        let Some(close_pos) = at.find(&close_tag) else {
            out.push_str(at);
            break;
        };
        let mut open_tag = at[..=open_end].to_string();
        let (anchor_id, inner) = split_heading_anchor(&at[open_end + 1..close_pos]);
        let id = attr_value(&open_tag, "id").or(anchor_id);
        if let Some(id_value) = id.as_ref() {
            if !open_tag.contains(" id=\"") {
                open_tag.insert_str(open_tag.len() - 1, &format!(r#" id="{id_value}""#));
            }
        }
        out.push_str(&open_tag);
        out.push_str(inner);
        if let Some(id) = id {
            out.push_str("<a class=\"ghrm-anchor\" aria-hidden=\"true\" tabindex=\"-1\" href=\"#");
            out.push_str(&id);
            out.push_str("\">#</a>");
        }
        out.push_str(&close_tag);
        rest = &at[close_pos + close_tag.len()..];
    }
    out
}

fn attr_value(tag: &str, name: &str) -> Option<String> {
    let needle = format!(r#" {name}=""#);
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_string())
}

fn split_heading_anchor(inner: &str) -> (Option<String>, &str) {
    if !inner.starts_with("<a ") {
        return (None, inner);
    }
    let Some(close) = inner.find("</a>") else {
        return (None, inner);
    };
    let anchor = &inner[..close + 4];
    if !anchor.contains(r#"class="anchor""#) {
        return (None, inner);
    }
    let id = attr_value(anchor, "id").or_else(|| {
        attr_value(anchor, "href").and_then(|href| href.strip_prefix('#').map(str::to_string))
    });
    (id, &inner[close + 4..])
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
}
