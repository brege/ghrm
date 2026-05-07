use super::Rendered;
use comrak::adapters::CodefenceRendererAdapter;
use comrak::nodes::Sourcepos;
use std::fmt;
use std::path::Path;

pub(super) fn render_text(filename: &str, text: &str) -> Rendered {
    let lang = Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .or_else(|| detect_shebang(text));
    let escaped = html_escape::encode_text(text);
    Rendered {
        title: filename.to_string(),
        html: code_block_html(lang, &escaped),
        lang: lang.map(String::from),
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

pub(super) struct GhrmBlockAdapter {
    pub(super) class: &'static str,
    pub(super) canvas: &'static str,
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

fn code_block_html(lang: Option<&str>, body: &str) -> String {
    let attrs = lang
        .map(|l| format!(r#" class="language-{l}" data-lang="{l}""#))
        .unwrap_or_default();
    format!(
        r#"<div class="highlight"><pre tabindex="0" class="chroma"><code{attrs}>{body}</code></pre></div>"#
    )
}

pub(super) fn rewrite_code_blocks(html: &str) -> String {
    // lol_html cannot select a parent from a child match because :has() is not
    // supported, and code fences need to replace the whole pre/code pair.
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
