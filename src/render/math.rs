use comrak::adapters::CodefenceRendererAdapter;
use comrak::nodes::Sourcepos;
use std::fmt;

pub(super) struct GhrmMathAdapter;

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

pub(super) fn has_math_markers(md: &str, html: &str) -> bool {
    if html.contains("ghrm-math-block") {
        return true;
    }
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

pub(super) fn rewrite_math_spans(html: &str) -> String {
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

pub(super) fn rewrite_math_display(html: &str) -> String {
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
