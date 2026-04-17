use crate::assets::FAVICON_SVG_URL;

pub struct PageShell<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub has_math: bool,
    pub has_map: bool,
    pub has_mermaid: bool,
    pub live_reload: bool,
}

pub fn base(p: PageShell) -> String {
    let mut out = String::with_capacity(p.body.len() + 4096);
    out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
    out.push_str("  <meta charset=\"utf-8\">\n");
    out.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    out.push_str(&format!(
        "  <title>{}</title>\n",
        html_escape::encode_text(p.title)
    ));
    out.push_str(&format!(
        "  <link rel=\"icon\" href=\"data:image/svg+xml,{}\">\n",
        FAVICON_SVG_URL
    ));
    out.push_str(&format!(
        "  <link rel=\"shortcut icon\" href=\"data:image/svg+xml,{}\">\n",
        FAVICON_SVG_URL
    ));
    out.push_str(
        "  <script>\n  (function() {\n    var stored = localStorage.getItem('ghrm-theme');\n    var system = window.matchMedia('(prefers-color-scheme: dark)').matches\n      ? 'dark'\n      : 'light';\n    document.documentElement.setAttribute('data-theme', stored || system);\n  })();\n  </script>\n",
    );
    out.push_str("  <link rel=\"stylesheet\" href=\"/_ghrm/css/bundle.css\">\n");
    out.push_str(
        "  <link id=\"ghrm-hljs-light\" rel=\"stylesheet\" href=\"/vendor/highlightjs/github.min.css\">\n",
    );
    out.push_str(
        "  <link id=\"ghrm-hljs-dark\" rel=\"stylesheet\" href=\"/vendor/highlightjs/github-dark.min.css\" disabled>\n",
    );
    out.push_str("</head>\n<body>\n");
    out.push_str(p.body);
    out.push_str("\n  <button id=\"theme-toggle\" type=\"button\" aria-label=\"Toggle theme\">\n");
    out.push_str("    <svg class=\"icon-sun\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"12\" cy=\"12\" r=\"5\"/><line x1=\"12\" y1=\"1\" x2=\"12\" y2=\"3\"/><line x1=\"12\" y1=\"21\" x2=\"12\" y2=\"23\"/><line x1=\"4.22\" y1=\"4.22\" x2=\"5.64\" y2=\"5.64\"/><line x1=\"18.36\" y1=\"18.36\" x2=\"19.78\" y2=\"19.78\"/><line x1=\"1\" y1=\"12\" x2=\"3\" y2=\"12\"/><line x1=\"21\" y1=\"12\" x2=\"23\" y2=\"12\"/><line x1=\"4.22\" y1=\"19.78\" x2=\"5.64\" y2=\"18.36\"/><line x1=\"18.36\" y1=\"5.64\" x2=\"19.78\" y2=\"4.22\"/></svg>\n");
    out.push_str("    <svg class=\"icon-moon\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z\"/></svg>\n");
    out.push_str("  </button>\n");
    if p.has_math {
        out.push_str("  <link rel=\"stylesheet\" href=\"/vendor/katex/katex.min.css\">\n");
    }
    if p.has_map {
        out.push_str("  <link rel=\"stylesheet\" href=\"/vendor/leaflet/leaflet.css\">\n");
    }
    if p.has_mermaid {
        out.push_str("  <script defer src=\"/vendor/mermaid.js\"></script>\n");
        out.push_str("  <script defer src=\"/vendor/svg-pan-zoom.min.js\"></script>\n");
    }
    if p.has_math {
        out.push_str("  <script defer src=\"/vendor/katex/katex.min.js\"></script>\n");
        out.push_str("  <script defer src=\"/vendor/katex/auto-render.min.js\"></script>\n");
    }
    if p.has_map {
        out.push_str("  <script defer src=\"/vendor/leaflet/leaflet.js\"></script>\n");
        out.push_str("  <script defer src=\"/vendor/topojson-client.min.js\"></script>\n");
    }
    out.push_str("  <script defer src=\"/vendor/highlightjs/highlight.min.js\"></script>\n");
    out.push_str("  <script type=\"module\" src=\"/js/preview.js\"></script>\n");
    out.push_str("  <script>\n  (function() {\n    document.addEventListener('DOMContentLoaded', function() {\n      var btn = document.getElementById('theme-toggle');\n      if (!btn) return;\n      btn.addEventListener('click', function() {\n        var current = document.documentElement.getAttribute('data-theme');\n        var next = current === 'dark' ? 'light' : 'dark';\n        document.documentElement.setAttribute('data-theme', next);\n        localStorage.setItem('ghrm-theme', next);\n        document.dispatchEvent(\n          new CustomEvent('ghrm:themechange', { detail: { theme: next } })\n        );\n      });\n    });\n  })();\n  </script>\n");
    if p.live_reload {
        out.push_str("  <script>\n  (function() {\n    var proto = location.protocol === 'https:' ? 'wss:' : 'ws:';\n    var url = proto + '//' + location.host + '/_ghrm/ws';\n    function connect() {\n      var ws = new WebSocket(url);\n      ws.onmessage = function(ev) { if (ev.data === 'reload') location.reload(); };\n      ws.onclose = function() { setTimeout(connect, 1000); };\n    }\n    connect();\n  })();\n  </script>\n");
    }
    out.push_str("</body>\n</html>\n");
    out
}

pub fn page(content_html: &str) -> String {
    format!(
        "<article class=\"markdown-body\">\n{}\n</article>\n",
        content_html
    )
}

pub struct ExplorerCtx<'a> {
    pub show_title: bool,
    pub title: &'a str,
    pub has_parent: bool,
    pub parent_href: &'a str,
    pub entries: &'a [ExplorerEntry<'a>],
    pub readme: Option<ExplorerReadme<'a>>,
}

pub struct ExplorerEntry<'a> {
    pub name: &'a str,
    pub href: &'a str,
    pub is_dir: bool,
}

pub struct ExplorerReadme<'a> {
    pub name: &'a str,
    pub html: &'a str,
}

const ICON_DIR: &str = r##"<svg aria-hidden="true" focusable="false" class="octicon octicon-file-directory-fill icon-directory" viewBox="0 0 16 16"><path d="M1.75 1A1.75 1.75 0 0 0 0 2.75v10.5C0 14.216.784 15 1.75 15h12.5A1.75 1.75 0 0 0 16 13.25v-8.5A1.75 1.75 0 0 0 14.25 3H7.5a.25.25 0 0 1-.2-.1l-.9-1.2C6.07 1.26 5.55 1 5 1H1.75Z"></path></svg>"##;
const ICON_FILE: &str = r##"<svg aria-hidden="true" focusable="false" class="octicon octicon-file color-fg-muted" viewBox="0 0 16 16"><path d="M2 1.75C2 .784 2.784 0 3.75 0h6.586c.464 0 .909.184 1.237.513l2.914 2.914c.329.328.513.773.513 1.237v9.586A1.75 1.75 0 0 1 13.25 16h-9.5A1.75 1.75 0 0 1 2 14.25Zm1.75-.25a.25.25 0 0 0-.25.25v12.5c0 .138.112.25.25.25h9.5a.25.25 0 0 0 .25-.25V6h-2.75A1.75 1.75 0 0 1 9 4.25V1.5Zm6.75.062V4.25c0 .138.112.25.25.25h2.688l-.011-.013-2.914-2.914-.013-.011Z"></path></svg>"##;
const ICON_BOOK: &str = r##"<svg aria-hidden="true" focusable="false" class="octicon octicon-book" viewBox="0 0 16 16" width="16" height="16" fill="currentColor" display="inline-block" overflow="visible" style="vertical-align:text-bottom"><path d="M0 1.75A.75.75 0 0 1 .75 1h4.253c1.227 0 2.317.59 3 1.501A3.743 3.743 0 0 1 11.006 1h4.245a.75.75 0 0 1 .75.75v10.5a.75.75 0 0 1-.75.75h-4.507a2.25 2.25 0 0 0-1.591.659l-.622.621a.75.75 0 0 1-1.06 0l-.622-.621A2.25 2.25 0 0 0 5.258 13H.75a.75.75 0 0 1-.75-.75Zm7.251 10.324.004-5.073-.002-2.253A2.25 2.25 0 0 0 5.003 2.5H1.5v9h3.757a3.75 3.75 0 0 1 1.994.574ZM8.755 4.75l-.004 7.322a3.752 3.752 0 0 1 1.992-.572H14.5v-9h-3.495a2.25 2.25 0 0 0-2.25 2.25Z"></path></svg>"##;

pub fn explorer(ctx: ExplorerCtx) -> String {
    let mut out = String::with_capacity(2048);
    out.push_str("<article class=\"markdown-body\">\n");
    if ctx.show_title {
        out.push_str(&format!(
            "  <h1>{}</h1>\n",
            html_escape::encode_text(ctx.title)
        ));
    }
    if !ctx.entries.is_empty() || ctx.has_parent {
        out.push_str("  <table class=\"ghrm-nav-table\">\n    <tbody>\n");
        if ctx.has_parent {
            out.push_str("      <tr>\n");
            out.push_str(&format!(
                "        <td class=\"ghrm-nav-icon\">{}</td>\n",
                ICON_DIR
            ));
            out.push_str(&format!(
                "        <td class=\"ghrm-nav-name\"><a href=\"{}\">..</a></td>\n",
                html_escape::encode_double_quoted_attribute(ctx.parent_href)
            ));
            out.push_str("      </tr>\n");
        }
        for e in ctx.entries {
            out.push_str("      <tr>\n");
            out.push_str(&format!(
                "        <td class=\"ghrm-nav-icon\">{}</td>\n",
                if e.is_dir { ICON_DIR } else { ICON_FILE }
            ));
            out.push_str(&format!(
                "        <td class=\"ghrm-nav-name\"><a href=\"{}\">{}</a></td>\n",
                html_escape::encode_double_quoted_attribute(e.href),
                html_escape::encode_text(e.name)
            ));
            out.push_str("      </tr>\n");
        }
        out.push_str("    </tbody>\n  </table>\n");
    } else {
        out.push_str("  <p>No markdown files found.</p>\n");
    }
    if let Some(r) = ctx.readme {
        out.push_str("  <div class=\"ghrm-readme-box\">\n");
        out.push_str("    <div class=\"ghrm-readme-header\">\n");
        out.push_str(&format!("      {}\n", ICON_BOOK));
        out.push_str(&format!("      {}\n", html_escape::encode_text(r.name)));
        out.push_str("    </div>\n");
        out.push_str("    <div class=\"ghrm-readme-content\">\n");
        out.push_str(r.html);
        out.push_str("\n    </div>\n  </div>\n");
    }
    out.push_str("</article>\n");
    out
}
