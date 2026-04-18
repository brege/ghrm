use crate::assets::FAVICON_SVG_URL;

const BASE_TEMPLATE: &str = include_str!("../templates/base.html");
const PAGE_TEMPLATE: &str = include_str!("../templates/page.html");
const EXPLORER_TEMPLATE: &str = include_str!("../templates/explorer.html");
const ICONS_TEMPLATE: &str = include_str!("../templates/fragments/icons.html");
const TITLE_BLOCK_TEMPLATE: &str = include_str!("../templates/fragments/title_block.html");
const NAV_TABLE_TEMPLATE: &str = include_str!("../templates/fragments/nav_table.html");
const NAV_EMPTY_TEMPLATE: &str = include_str!("../templates/fragments/nav_empty.html");
const NAV_PARENT_ROW_TEMPLATE: &str = include_str!("../templates/fragments/nav_parent_row.html");
const NAV_ROW_TEMPLATE: &str = include_str!("../templates/fragments/nav_row.html");
const README_BOX_TEMPLATE: &str = include_str!("../templates/fragments/readme_box.html");

pub struct PageShell<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub live_reload: bool,
}

pub fn base(p: PageShell) -> String {
    let mut out = BASE_TEMPLATE.to_string();
    replace(&mut out, "{{ title }}", &html_escape::encode_text(p.title));
    replace(&mut out, "{{ favicon }}", FAVICON_SVG_URL);
    replace(&mut out, "{{ icons }}", ICONS_TEMPLATE);
    replace(&mut out, "{{ body }}", p.body);
    replace(
        &mut out,
        "{{ live_reload }}",
        if p.live_reload { "1" } else { "0" },
    );
    out
}

pub fn page(content_html: &str) -> String {
    let mut out = PAGE_TEMPLATE.to_string();
    replace(&mut out, "{{ content }}", content_html);
    out
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

pub fn explorer(ctx: ExplorerCtx) -> String {
    let mut rows = String::new();
    if ctx.has_parent {
        let mut row = NAV_PARENT_ROW_TEMPLATE.to_string();
        replace(
            &mut row,
            "{{ href }}",
            &html_escape::encode_double_quoted_attribute(ctx.parent_href),
        );
        rows.push_str(&row);
    }
    for e in ctx.entries {
        let mut row = NAV_ROW_TEMPLATE.to_string();
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
            &html_escape::encode_double_quoted_attribute(e.href),
        );
        replace(&mut row, "{{ name }}", &html_escape::encode_text(e.name));
        rows.push_str(&row);
    }

    let table_or_empty = if !ctx.entries.is_empty() || ctx.has_parent {
        let mut table = NAV_TABLE_TEMPLATE.to_string();
        replace(&mut table, "{{ rows }}", &rows);
        table
    } else {
        NAV_EMPTY_TEMPLATE.to_string()
    };

    let readme_block = if let Some(r) = ctx.readme {
        let mut readme = README_BOX_TEMPLATE.to_string();
        replace(&mut readme, "{{ name }}", &html_escape::encode_text(r.name));
        replace(&mut readme, "{{ html }}", r.html);
        readme
    } else {
        String::new()
    };

    let title_block = if ctx.show_title {
        let mut title = TITLE_BLOCK_TEMPLATE.to_string();
        replace(
            &mut title,
            "{{ title }}",
            &html_escape::encode_text(ctx.title),
        );
        title
    } else {
        String::new()
    };

    let mut out = EXPLORER_TEMPLATE.to_string();
    replace(&mut out, "{{ title_block }}", &title_block);
    replace(&mut out, "{{ table_or_empty }}", &table_or_empty);
    replace(&mut out, "{{ readme_block }}", &readme_block);
    out
}

fn replace(out: &mut String, needle: &str, value: &str) {
    *out = out.replace(needle, value);
}
