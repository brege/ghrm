use super::Rendered;
use comrak::adapters::CodefenceRendererAdapter;
use comrak::nodes::Sourcepos;
use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

pub(super) fn render_text(filename: &str, text: &str) -> Rendered {
    let lang = detect_lang(filename, text);
    let escaped = html_escape::encode_text(text);
    Rendered {
        title: filename.to_string(),
        html: code_block_html(lang.as_deref(), &escaped),
        langs: lang.iter().cloned().collect(),
        lang,
        has_mermaid: false,
        has_math: false,
        has_map: false,
    }
}

fn detect_lang(filename: &str, text: &str) -> Option<String> {
    lang_from_filename(filename)
        .or_else(|| {
            Path::new(filename)
                .extension()
                .and_then(|s| s.to_str())
                .and_then(normalize_lang)
        })
        .or_else(|| detect_shebang(text).map(str::to_string))
}

fn lang_from_filename(filename: &str) -> Option<String> {
    let name = Path::new(filename)
        .file_name()
        .and_then(|s| s.to_str())?
        .to_ascii_lowercase();
    let lang = match name.as_str() {
        ".justfile" | "justfile" => "just",
        "cmakelists.txt" => "cmake",
        "containerfile" | "dockerfile" => "dockerfile",
        "gemfile" | "rakefile" => "ruby",
        "gnumakefile" | "makefile" => "makefile",
        _ => return None,
    };
    Some(lang.to_string())
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
        "ksh" | "zsh" => Some("bash"),
        "csh" | "fish" | "tcsh" => Some("shell"),
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
        "sed" | "gsed" => None,
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
        "racket" => Some("scheme"),
        "guile" | "scheme" => Some("scheme"),
        "runhaskell" | "runghc" => Some("haskell"),
        "ocaml" | "ocamlrun" => Some("ocaml"),
        _ => None,
    }
}

fn normalize_lang(raw: &str) -> Option<String> {
    let raw = raw.trim().trim_start_matches('.');
    if raw.is_empty() {
        return None;
    }
    let lang = raw.to_ascii_lowercase();
    let normalized = match lang.as_str() {
        "applescript" | "scpt" => "applescript",
        "ksh" | "zsh" => "bash",
        "csh" | "fish" | "tcsh" => "shell",
        "clj" | "cljc" | "cljs" | "clojure" | "edn" => "clojure",
        "cmake" => "cmake",
        "containerfile" | "dockerfile" => "dockerfile",
        "cr" | "crystal" => "crystal",
        "dart" => "dart",
        "ex" | "exs" | "elixir" => "elixir",
        "erl" | "erlang" | "hrl" => "erlang",
        "gradle" | "groovy" => "groovy",
        "hs" | "haskell" => "haskell",
        "htm" | "html" | "svg" | "xhtml" | "xml" => "xml",
        "jl" | "julia" => "julia",
        "just" | "justfile" => "just",
        "lisp" | "el" | "elisp" | "lsp" => "lisp",
        "mak" | "make" | "makefile" | "mk" => "makefile",
        "ml" | "mli" | "ocaml" => "ocaml",
        "nim" => "nim",
        "nix" => "nix",
        "ps" | "ps1" | "psd1" | "psm1" | "powershell" | "pwsh" => "powershell",
        "racket" | "rkt" | "scm" | "scheme" | "ss" => "scheme",
        "scala" => "scala",
        "sed" => return None,
        "tcl" => "tcl",
        _ => lang.as_str(),
    };
    Some(normalized.to_string())
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
        let lang = code_lang(open_tag).and_then(normalize_lang);
        out.push_str(&code_block_html(lang.as_deref(), body));
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

pub(super) fn extract_langs(html: &str) -> Vec<String> {
    let mut langs = BTreeSet::new();
    let mut rest = html;
    let marker = r#" data-lang=""#;

    while let Some(idx) = rest.find(marker) {
        let after = &rest[idx + marker.len()..];
        let Some(end) = after.find('"') else {
            break;
        };
        if let Some(lang) = normalize_lang(&after[..end]) {
            langs.insert(lang);
        }
        rest = &after[end + 1..];
    }

    langs.into_iter().collect()
}
