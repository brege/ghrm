use anyhow::{Context, Result};
use std::path::Path;

const FAVICON_SVG_URL: &str = "%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%20512%20512%22%20fill%3D%22%232ea043%22%3E%3Cpath%20d%3D%22M240%20216V32H92a12%2012%200%200%200-12%2012v424a12%2012%200%200%200%2012%2012h328a12%2012%200%200%200%2012-12V224H248a8%208%200%200%201-8-8z%22%2F%3E%3Cpath%20d%3D%22M272%2041.69V188a4%204%200%200%200%204%204h146.31a2%202%200%200%200%201.42-3.41L275.41%2040.27a2%202%200%200%200-3.41%201.42z%22%2F%3E%3C%2Fsvg%3E";

pub struct PageShell<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub source: &'a str,
    pub default_scope: &'a str,
    pub has_ext_filter: bool,
    pub has_mermaid: bool,
    pub has_math: bool,
    pub has_map: bool,
}

pub struct PageCtx<'a> {
    pub crumbs: &'a str,
    pub preview_html: &'a str,
    pub raw_html: &'a str,
    pub view_attrs: &'a str,
    pub preview_hidden: bool,
    pub raw_hidden: bool,
}

pub struct ExplorerCtx<'a> {
    pub crumbs: &'a str,
    pub current_path: &'a str,
    pub has_parent: bool,
    pub parent_href: &'a str,
    pub entries: &'a [ExplorerEntry],
    pub readme: Option<ExplorerReadme<'a>>,
}

pub struct ExplorerEntry {
    pub name: String,
    pub href: String,
    pub is_dir: bool,
    pub modified: Option<u64>,
}

pub struct ExplorerReadme<'a> {
    pub name: &'a str,
    pub html: &'a str,
}

pub fn base(p: PageShell) -> Result<String> {
    let dir = crate::theme::dir()?.join("templates");
    let mut out = read_tmpl(&dir.join("base.html"))?;
    replace(&mut out, "{{ title }}", &html_escape::encode_text(p.title));
    replace(&mut out, "{{ favicon }}", FAVICON_SVG_URL);
    replace(
        &mut out,
        "{{ icons }}",
        &read_tmpl(&dir.join("fragments/icons.html"))?,
    );
    replace(&mut out, "{{ source }}", p.source);
    replace(&mut out, "{{ body }}", p.body);
    replace(
        &mut out,
        "{{ default_scope }}",
        &html_escape::encode_double_quoted_attribute(p.default_scope),
    );
    replace(
        &mut out,
        "{{ has_ext_filter }}",
        if p.has_ext_filter { "1" } else { "0" },
    );
    replace(
        &mut out,
        "{{ css_math }}",
        if p.has_math {
            r#"  <link rel="stylesheet" href="/vendor/katex/katex.min.css">
"#
        } else {
            ""
        },
    );
    replace(
        &mut out,
        "{{ css_map }}",
        if p.has_map {
            r#"  <link rel="stylesheet" href="/vendor/leaflet/leaflet.css">
"#
        } else {
            ""
        },
    );
    replace(
        &mut out,
        "{{ vendor_mermaid }}",
        if p.has_mermaid {
            r#"  <script defer src="/vendor/mermaid.js"></script>
  <script defer src="/vendor/svg-pan-zoom.min.js"></script>"#
        } else {
            ""
        },
    );
    replace(
        &mut out,
        "{{ vendor_math }}",
        if p.has_math {
            r#"<script defer src="/vendor/katex/katex.min.js"></script>
  <script defer src="/vendor/katex/auto-render.min.js"></script>"#
        } else {
            ""
        },
    );
    replace(
        &mut out,
        "{{ vendor_map }}",
        if p.has_map {
            r#"<script defer src="/vendor/leaflet/leaflet.js"></script>
  <script defer src="/vendor/topojson-client.min.js"></script>"#
        } else {
            ""
        },
    );
    Ok(out)
}

pub fn page(ctx: PageCtx<'_>) -> Result<String> {
    let path = crate::theme::dir()?.join("templates/page.html");
    let mut out = read_tmpl(&path)?;
    replace(&mut out, "{{ crumbs }}", ctx.crumbs);
    replace(&mut out, "{{ preview_html }}", ctx.preview_html);
    replace(&mut out, "{{ raw_html }}", ctx.raw_html);
    replace(&mut out, "{{ view_attrs }}", ctx.view_attrs);
    replace(
        &mut out,
        "{{ preview_hidden }}",
        if ctx.preview_hidden { " hidden" } else { "" },
    );
    replace(
        &mut out,
        "{{ raw_hidden }}",
        if ctx.raw_hidden { " hidden" } else { "" },
    );
    Ok(out)
}

pub fn explorer(ctx: ExplorerCtx) -> Result<String> {
    let dir = crate::theme::dir()?.join("templates");
    let nav_row_tmpl = read_tmpl(&dir.join("fragments/nav_row.html"))?;
    let nav_parent_row = read_tmpl(&dir.join("fragments/nav_parent_row.html"))?;
    let nav_table_tmpl = read_tmpl(&dir.join("fragments/nav_table.html"))?;
    let nav_empty = read_tmpl(&dir.join("fragments/nav_empty.html"))?;
    let readme_box_tmpl = read_tmpl(&dir.join("fragments/readme_box.html"))?;
    let title_block_tmpl = read_tmpl(&dir.join("fragments/title_block.html"))?;
    let explorer_tmpl = read_tmpl(&dir.join("explorer.html"))?;

    let mut rows = String::new();
    if ctx.has_parent {
        let mut row = nav_parent_row;
        replace(
            &mut row,
            "{{ href }}",
            &html_escape::encode_double_quoted_attribute(ctx.parent_href),
        );
        rows.push_str(&row);
    }
    for e in ctx.entries {
        let mut row = nav_row_tmpl.clone();
        replace(
            &mut row,
            "{{ icon }}",
            if e.is_dir {
                "ghrm-icon-dir"
            } else {
                "ghrm-icon-file"
            },
        );
        replace(
            &mut row,
            "{{ href }}",
            &html_escape::encode_double_quoted_attribute(&e.href),
        );
        replace(&mut row, "{{ name }}", &html_escape::encode_text(&e.name));
        replace(
            &mut row,
            "{{ modified }}",
            &e.modified.map(|ts| ts.to_string()).unwrap_or_default(),
        );
        rows.push_str(&row);
    }

    let table_or_empty = if !ctx.entries.is_empty() || ctx.has_parent {
        let mut table = nav_table_tmpl;
        replace(&mut table, "{{ rows }}", &rows);
        table
    } else {
        nav_empty
    };

    let readme_block = if let Some(r) = ctx.readme {
        let mut readme = readme_box_tmpl;
        replace(&mut readme, "{{ name }}", &html_escape::encode_text(r.name));
        replace(&mut readme, "{{ html }}", r.html);
        readme
    } else {
        String::new()
    };

    let mut title_block = title_block_tmpl;
    replace(&mut title_block, "{{ crumbs }}", ctx.crumbs);

    let mut out = explorer_tmpl;
    replace(
        &mut out,
        "{{ current_path }}",
        &html_escape::encode_double_quoted_attribute(ctx.current_path),
    );
    replace(&mut out, "{{ title_block }}", &title_block);
    replace(&mut out, "{{ table_or_empty }}", &table_or_empty);
    replace(&mut out, "{{ readme_block }}", &readme_block);
    Ok(out)
}

fn read_tmpl(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("template missing: {}", path.display()))
}

fn replace(out: &mut String, needle: &str, value: &str) {
    *out = out.replace(needle, value);
}
