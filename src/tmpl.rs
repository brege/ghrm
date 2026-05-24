use crate::explorer::column;

use anyhow::Result;
use askama::Template;

#[derive(Template)]
#[template(path = "base.html")]
pub struct PageShell<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub source: &'a str,
    pub about: &'a str,
    pub show_logout: bool,
    pub gist_nav: &'a str,
    pub asset_json: &'a str,
    pub vendor_styles: &'a [String],
    pub vendor_scripts: &'a [String],
}

#[derive(Template)]
#[template(path = "fragments/about.html")]
pub struct AboutPeek<'a> {
    pub detail_sections: &'a [AboutDetailSection],
    pub stats_loaded: bool,
    pub stats: &'a AboutStats,
    pub local_path: &'a str,
    pub project_href: &'a str,
    pub project_release_href: &'a str,
    pub project_version: &'static str,
}

#[derive(Default)]
pub struct AboutStats {
    pub metadata: Vec<AboutStatRow>,
    pub stats: Vec<AboutStatRow>,
    pub languages: Vec<AboutLanguage>,
    pub language_total: String,
}

impl AboutStats {
    pub fn has_summary(&self) -> bool {
        !self.metadata.is_empty() || !self.stats.is_empty() || !self.languages.is_empty()
    }
}

pub struct AboutStatRow {
    pub label: String,
    pub value: String,
    pub title: String,
    pub title_ts: Option<u64>,
    pub parts: Vec<AboutStatPart>,
    pub icon: &'static str,
    pub href: String,
    pub items: Vec<AboutStatItem>,
}

pub struct AboutStatPart {
    pub value: String,
    pub separator: bool,
}

pub struct AboutStatItem {
    pub label: String,
    pub value: String,
    pub href: String,
    pub metrics: Vec<AboutStatMetric>,
}

pub struct AboutStatMetric {
    pub value: String,
    pub label: String,
    pub title: String,
}

pub struct AboutLanguage {
    pub name: String,
    pub value: String,
    pub lines: String,
    pub color: String,
    pub style: String,
    pub title: String,
}

pub struct AboutDetailSection {
    pub heading: String,
    pub class_name: &'static str,
    pub rows: Vec<AboutDetailRow>,
}

pub struct AboutDetailRow {
    pub label: String,
    pub value: String,
    pub title: String,
    pub cells: Vec<String>,
}

#[derive(Template)]
#[template(path = "page.html")]
pub struct PageCtx<'a> {
    pub features: &'a str,
    pub crumbs: &'a str,
    pub preview_html: &'a str,
    pub raw_html: &'a str,
    pub view_attrs: &'a str,
    pub preview_hidden: bool,
    pub raw_hidden: bool,
}

#[derive(Template)]
#[template(path = "explorer.html")]
pub struct ExplorerCtx<'a> {
    pub article_class: &'a str,
    pub features: &'a str,
    pub crumbs: &'a str,
    pub current_path: &'a str,
    pub archive_zip_href: &'a str,
    pub archive_tar_zst_href: &'a str,
    pub has_parent: bool,
    pub parent_href: &'a str,
    pub filter_menu_active: bool,
    pub filter_controls: &'a [FilterControl],
    pub sort_menu_active: bool,
    pub sort_controls: &'a [SortControl],
    pub sort_dir_control: &'a SortDirControl,
    pub column_menu_active: bool,
    pub column_controls: &'a [ColumnControl],
    pub headers_control: &'a ColumnControl,
    pub column_defs: &'a [column::Def],
    pub show_headers: bool,
    pub empty_cells: &'a [column::Cell],
    pub entries: &'a [ExplorerEntry],
    pub readme: Option<ExplorerReadme<'a>>,
}

pub struct FilterControl {
    pub href: String,
    pub label: String,
    pub title: String,
    pub active: bool,
    pub hidden: bool,
    pub group: bool,
}

pub struct SortControl {
    pub href: String,
    pub label: &'static str,
    pub title: &'static str,
    pub active: bool,
    pub hidden: bool,
}

pub struct SortDirControl {
    pub href: String,
    pub label: &'static str,
    pub icon: &'static str,
    pub active: bool,
}

pub struct ColumnControl {
    pub href: String,
    pub key: &'static str,
    pub label: &'static str,
    pub title: &'static str,
    pub active: bool,
    pub edge: bool,
}

pub struct ExplorerEntry {
    pub name: String,
    pub href: String,
    pub is_dir: bool,
    pub cells: Vec<column::Cell>,
}

pub struct ExplorerReadme<'a> {
    pub name: &'a str,
    pub html: &'a str,
}

#[derive(Template)]
#[template(path = "gist.html")]
pub struct GistCtx<'a> {
    pub has_paste: bool,
    pub paste_id: &'a str,
    pub page_href: &'a str,
    pub raw_href: &'a str,
    pub stash_href: &'a str,
    pub paste_body: &'a str,
    pub raw_html: &'a str,
}

#[derive(Template)]
#[template(path = "gist_stash.html")]
pub struct GistStashCtx<'a> {
    pub entries: &'a [GistStashEntry],
}

pub struct GistStashEntry {
    pub id: String,
    pub name: String,
    pub href: String,
    pub modified: Option<u64>,
    pub size: String,
    pub lines: String,
    pub current: bool,
}

#[derive(Template)]
#[template(path = "fragments/search/path.html")]
pub struct PathSearchCtx<'a> {
    pub pending: bool,
    pub rows: &'a [PathSearchRow<'a>],
    pub empty_colspan: usize,
}

pub struct PathSearchRow<'a> {
    pub href: String,
    pub html: String,
    pub is_dir: bool,
    pub cells: &'a [column::Cell],
}

#[derive(Template)]
#[template(path = "fragments/search/content.html")]
pub struct ContentSearchCtx<'a> {
    pub rows: &'a [ContentSearchRow],
    pub truncated: bool,
    pub max_rows: usize,
    pub empty_colspan: usize,
    pub content_colspan: usize,
    pub summary_colspan: usize,
}

pub struct ContentSearchRow {
    pub href: String,
    pub path: String,
    pub line: u64,
    pub html: String,
    pub modified: Option<u64>,
}

pub fn base(p: PageShell) -> Result<String> {
    Ok(p.render()?)
}

pub fn about(p: AboutPeek) -> Result<String> {
    Ok(p.render()?)
}

pub fn page(ctx: PageCtx<'_>) -> Result<String> {
    Ok(ctx.render()?)
}

pub fn explorer(ctx: ExplorerCtx) -> Result<String> {
    Ok(ctx.render()?)
}

pub fn gist(ctx: GistCtx<'_>) -> Result<String> {
    Ok(ctx.render()?)
}

pub fn gist_stash(ctx: GistStashCtx<'_>) -> Result<String> {
    Ok(ctx.render()?)
}

pub fn path_search(ctx: PathSearchCtx<'_>) -> Result<String> {
    Ok(ctx.render()?)
}

pub fn content_search(ctx: ContentSearchCtx<'_>) -> Result<String> {
    Ok(ctx.render()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explorer::column;

    fn columns_all_visible() -> column::Set {
        column::Set::from_defaults(|_| true)
    }

    fn empty_shell() -> PageShell<'static> {
        PageShell {
            title: "Test",
            body: "<article class=\"markdown-body\">content</article>",
            source: "",
            about: "",
            show_logout: false,
            gist_nav: "",
            asset_json: "{}",
            vendor_styles: &[],
            vendor_scripts: &[],
        }
    }

    struct ExplorerFixture {
        sort_dir_control: SortDirControl,
        headers_control: ColumnControl,
    }

    impl ExplorerFixture {
        fn new() -> Self {
            Self {
                sort_dir_control: SortDirControl {
                    href: "?sort_dir=desc".to_string(),
                    label: "Sort ascending",
                    icon: "ghrm-icon-chevron-up",
                    active: false,
                },
                headers_control: ColumnControl {
                    href: "?headers=1".to_string(),
                    key: "headers",
                    label: "Show column headers",
                    title: "Show column headers",
                    active: false,
                    edge: false,
                },
            }
        }

        fn ctx(&self) -> ExplorerCtx<'_> {
            static EMPTY_FILTER: &[FilterControl] = &[];
            static EMPTY_SORT: &[SortControl] = &[];
            static EMPTY_COLUMN: &[ColumnControl] = &[];
            static EMPTY_CELLS: &[column::Cell] = &[];

            ExplorerCtx {
                article_class: "markdown-body",
                features: "",
                crumbs: "<span>root</span>",
                current_path: "/test",
                archive_zip_href: "/_ghrm/archive?fmt=zip",
                archive_tar_zst_href: "/_ghrm/archive?fmt=tar.zst",
                has_parent: false,
                parent_href: "",
                filter_menu_active: false,
                filter_controls: EMPTY_FILTER,
                sort_menu_active: false,
                sort_controls: EMPTY_SORT,
                sort_dir_control: &self.sort_dir_control,
                column_menu_active: false,
                column_controls: EMPTY_COLUMN,
                headers_control: &self.headers_control,
                column_defs: column::DEFS,
                show_headers: true,
                empty_cells: EMPTY_CELLS,
                entries: &[],
                readme: None,
            }
        }
    }

    fn minimal_page_ctx() -> PageCtx<'static> {
        PageCtx {
            features: "code",
            crumbs: "<span>file.rs</span>",
            preview_html: "<pre>code</pre>",
            raw_html: "<pre>raw</pre>",
            view_attrs: "data-ghrm-view-kind=\"source\"",
            preview_hidden: false,
            raw_hidden: true,
        }
    }

    fn minimal_gist_ctx() -> GistCtx<'static> {
        GistCtx {
            has_paste: true,
            paste_id: "test-paste",
            page_href: "/_ghrm/gist",
            raw_href: "/_ghrm/gist/raw",
            stash_href: "/_ghrm/gist/stash",
            paste_body: "hello world",
            raw_html: "<pre>hello world</pre>",
        }
    }

    fn minimal_gist_stash_ctx() -> GistStashCtx<'static> {
        static ENTRIES: &[GistStashEntry] = &[];
        GistStashCtx { entries: ENTRIES }
    }

    mod base_shell {
        use super::*;

        #[test]
        fn entry_script_urls() {
            let html = base(empty_shell()).unwrap();
            assert!(
                html.contains("src=\"/_ghrm/assets/js/preview.js\""),
                "shell missing preview.js entry script"
            );
            assert!(
                html.contains("src=\"/_ghrm/assets/js/main.js\""),
                "shell missing main.js entry script"
            );
            assert!(
                html.contains("src=\"/_ghrm/assets/js/gist.js\""),
                "shell missing gist.js entry script"
            );
        }

        #[test]
        fn entry_scripts_are_modules() {
            let html = base(empty_shell()).unwrap();
            assert!(
                html.contains("type=\"module\" src=\"/_ghrm/assets/js/preview.js\""),
                "preview.js must be loaded as module"
            );
            assert!(
                html.contains("type=\"module\" src=\"/_ghrm/assets/js/main.js\""),
                "main.js must be loaded as module"
            );
            assert!(
                html.contains("type=\"module\" src=\"/_ghrm/assets/js/gist.js\""),
                "gist.js must be loaded as module"
            );
        }

        #[test]
        fn htmx_body_attributes() {
            let html = base(empty_shell()).unwrap();
            assert!(html.contains("hx-boost=\"true\""), "body missing hx-boost");
            assert!(
                html.contains("hx-target=\"article.markdown-body\""),
                "body missing hx-target for article swap"
            );
            assert!(
                html.contains("hx-swap=\"outerHTML show:none\""),
                "body missing hx-swap"
            );
            assert!(
                html.contains("hx-push-url=\"true\""),
                "body missing hx-push-url"
            );
        }

        #[test]
        fn ghrm_assets_script() {
            let html = base(empty_shell()).unwrap();
            assert!(
                html.contains("id=\"ghrm-assets\""),
                "missing #ghrm-assets script for asset manifest"
            );
            assert!(
                html.contains("type=\"application/json\""),
                "#ghrm-assets must be application/json"
            );
        }

        #[test]
        fn path_search_elements() {
            let html = base(empty_shell()).unwrap();
            assert!(
                html.contains("id=\"ghrm-path-search\""),
                "missing #ghrm-path-search container"
            );
            assert!(
                html.contains("id=\"ghrm-path-search-input\""),
                "missing #ghrm-path-search-input"
            );
            assert!(
                html.contains("id=\"ghrm-path-search-toggle\""),
                "missing #ghrm-path-search-toggle"
            );
            assert!(
                html.contains("id=\"ghrm-search-mode\""),
                "missing #ghrm-search-mode button"
            );
            assert!(
                html.contains("id=\"ghrm-path-search-status\""),
                "missing #ghrm-path-search-status"
            );
        }

        #[test]
        fn theme_toggle() {
            let html = base(empty_shell()).unwrap();
            assert!(
                html.contains("id=\"theme-toggle\""),
                "missing #theme-toggle button"
            );
        }

        #[test]
        fn doc_chrome_toggle() {
            let html = base(empty_shell()).unwrap();
            assert!(
                html.contains("id=\"doc-chrome-toggle\""),
                "missing #doc-chrome-toggle button"
            );
        }

        #[test]
        fn toc_panel() {
            let html = base(empty_shell()).unwrap();
            assert!(
                html.contains("id=\"ghrm-toc-panel\""),
                "missing #ghrm-toc-panel nav"
            );
        }
    }

    mod explorer_contracts {
        use super::*;

        #[test]
        fn article_explorer_attrs() {
            let fix = ExplorerFixture::new();
            let html = explorer(fix.ctx()).unwrap();
            assert!(
                html.contains("data-explorer=\"true\""),
                "explorer article missing data-explorer"
            );
            assert!(
                html.contains("data-current-path=\"/test\""),
                "explorer article missing data-current-path"
            );
            assert!(
                html.contains("hx-history-elt"),
                "explorer article missing hx-history-elt"
            );
            assert!(
                html.contains("data-ghrm-features"),
                "explorer article missing data-ghrm-features"
            );
        }

        #[test]
        fn archive_progress_element() {
            let fix = ExplorerFixture::new();
            let html = explorer(fix.ctx()).unwrap();
            assert!(
                html.contains("<ghrm-archive-progress>"),
                "missing ghrm-archive-progress element"
            );
        }

        #[test]
        fn view_menu_elements() {
            let fix = ExplorerFixture::new();
            let html = explorer(fix.ctx()).unwrap();
            assert!(
                html.contains("id=\"ghrm-view-menu-toggle\""),
                "missing #ghrm-view-menu-toggle"
            );
            assert!(
                html.contains("id=\"ghrm-view-menu\""),
                "missing #ghrm-view-menu"
            );
        }

        #[test]
        fn sort_menu_elements() {
            let fix = ExplorerFixture::new();
            let html = explorer(fix.ctx()).unwrap();
            assert!(
                html.contains("id=\"ghrm-sort-menu-toggle\""),
                "missing #ghrm-sort-menu-toggle"
            );
            assert!(
                html.contains("id=\"ghrm-sort-menu\""),
                "missing #ghrm-sort-menu"
            );
            assert!(
                html.contains("id=\"ghrm-sort-dir-toggle\""),
                "missing #ghrm-sort-dir-toggle"
            );
        }

        #[test]
        fn archive_menu_elements() {
            let fix = ExplorerFixture::new();
            let html = explorer(fix.ctx()).unwrap();
            assert!(
                html.contains("id=\"ghrm-archive-menu-toggle\""),
                "missing #ghrm-archive-menu-toggle"
            );
            assert!(
                html.contains("id=\"ghrm-archive-menu\""),
                "missing #ghrm-archive-menu"
            );
            assert!(
                html.contains("data-ghrm-archive-url"),
                "missing data-ghrm-archive-url on archive buttons"
            );
        }

        #[test]
        fn column_menu_elements() {
            let fix = ExplorerFixture::new();
            let html = explorer(fix.ctx()).unwrap();
            assert!(
                html.contains("id=\"ghrm-column-menu-toggle\""),
                "missing #ghrm-column-menu-toggle"
            );
            assert!(
                html.contains("id=\"ghrm-column-menu\""),
                "missing #ghrm-column-menu"
            );
        }

        #[test]
        fn column_toggle_attrs() {
            let fix = ExplorerFixture::new();
            let col_control = ColumnControl {
                href: "?col=date".to_string(),
                key: "date",
                label: "Modified date",
                title: "Show file dates",
                active: true,
                edge: true,
            };
            let ctx = ExplorerCtx {
                column_controls: std::slice::from_ref(&col_control),
                ..fix.ctx()
            };
            let html = explorer(ctx).unwrap();
            assert!(
                html.contains("data-column-toggle=\"date\""),
                "column control missing data-column-toggle"
            );
            assert!(
                html.contains("data-column-edge=\"1\""),
                "edge column missing data-column-edge"
            );
        }

        #[test]
        fn table_column_key_attrs() {
            let fix = ExplorerFixture::new();
            let cells = columns_all_visible().empty_cells();
            let ctx = ExplorerCtx {
                empty_cells: &cells,
                has_parent: true,
                ..fix.ctx()
            };
            let html = explorer(ctx).unwrap();
            assert!(
                html.contains("data-column-key=\"date\""),
                "table cells missing data-column-key"
            );
        }

        #[test]
        fn column_headers_element() {
            let fix = ExplorerFixture::new();
            let ctx = ExplorerCtx {
                has_parent: true,
                ..fix.ctx()
            };
            let html = explorer(ctx).unwrap();
            assert!(
                html.contains("class=\"ghrm-column-headers\""),
                "missing .ghrm-column-headers"
            );
        }

        #[test]
        fn entry_with_timestamp() {
            let fix = ExplorerFixture::new();
            let cells = vec![column::Cell {
                key: "date",
                class: "ghrm-nav-meta ghrm-nav-meta-time",
                text_class: None,
                text: None,
                timestamp: Some(1700000000),
                hidden: false,
            }];
            let entries = [ExplorerEntry {
                name: "test.txt".to_string(),
                href: "/test.txt".to_string(),
                is_dir: false,
                cells,
            }];
            let ctx = ExplorerCtx {
                entries: &entries,
                ..fix.ctx()
            };
            let html = explorer(ctx).unwrap();
            assert!(
                html.contains("data-ts=\"1700000000\""),
                "timestamp cell missing data-ts"
            );
        }
    }

    mod file_view_contracts {
        use super::*;

        #[test]
        fn page_shell_view_kind() {
            let html = page(minimal_page_ctx()).unwrap();
            assert!(
                html.contains("class=\"ghrm-page-shell\""),
                "missing .ghrm-page-shell"
            );
            assert!(
                html.contains("data-ghrm-view-kind"),
                "page shell missing data-ghrm-view-kind"
            );
        }

        #[test]
        fn preview_and_raw_panes() {
            let html = page(minimal_page_ctx()).unwrap();
            assert!(
                html.contains("data-ghrm-preview-pane"),
                "missing [data-ghrm-preview-pane]"
            );
            assert!(
                html.contains("data-ghrm-raw-pane"),
                "missing [data-ghrm-raw-pane]"
            );
        }

        #[test]
        fn toc_button() {
            let html = page(minimal_page_ctx()).unwrap();
            assert!(
                html.contains("data-ghrm-toc-btn"),
                "missing [data-ghrm-toc-btn]"
            );
        }

        #[test]
        fn article_history_elt() {
            let html = page(minimal_page_ctx()).unwrap();
            assert!(
                html.contains("hx-history-elt"),
                "page article missing hx-history-elt"
            );
        }

        #[test]
        fn features_attr() {
            let html = page(minimal_page_ctx()).unwrap();
            assert!(
                html.contains("data-ghrm-features"),
                "page article missing data-ghrm-features"
            );
        }
    }

    mod gist_contracts {
        use super::*;

        #[test]
        fn gist_article_attrs() {
            let html = gist(minimal_gist_ctx()).unwrap();
            assert!(
                html.contains("data-ghrm-gist"),
                "gist article missing data-ghrm-gist"
            );
            assert!(
                html.contains("data-ghrm-gist-page"),
                "gist article missing data-ghrm-gist-page"
            );
            assert!(
                html.contains("data-ghrm-gist-id"),
                "gist article missing data-ghrm-gist-id"
            );
        }

        #[test]
        fn gist_form_elements() {
            let html = gist(minimal_gist_ctx()).unwrap();
            assert!(
                html.contains("data-ghrm-gist-form"),
                "missing [data-ghrm-gist-form]"
            );
            assert!(
                html.contains("data-ghrm-gist-editor"),
                "missing [data-ghrm-gist-editor]"
            );
        }

        #[test]
        fn gist_controls() {
            let html = gist(minimal_gist_ctx()).unwrap();
            assert!(
                html.contains("data-ghrm-gist-wrap"),
                "missing [data-ghrm-gist-wrap] toggle"
            );
            assert!(
                html.contains("data-ghrm-gist-copy"),
                "missing [data-ghrm-gist-copy] button"
            );
            assert!(
                html.contains("data-ghrm-gist-save-control"),
                "missing [data-ghrm-gist-save-control]"
            );
            assert!(
                html.contains("data-ghrm-gist-save"),
                "missing [data-ghrm-gist-save] button"
            );
        }

        #[test]
        fn gist_name_input() {
            let html = gist(minimal_gist_ctx()).unwrap();
            assert!(
                html.contains("data-ghrm-gist-name"),
                "missing [data-ghrm-gist-name] input"
            );
        }

        #[test]
        fn gist_status() {
            let html = gist(minimal_gist_ctx()).unwrap();
            assert!(
                html.contains("data-ghrm-gist-status"),
                "missing [data-ghrm-gist-status]"
            );
        }

        #[test]
        fn gist_stash_article() {
            let html = gist_stash(minimal_gist_stash_ctx()).unwrap();
            assert!(
                html.contains("data-ghrm-gist-stash"),
                "stash article missing data-ghrm-gist-stash"
            );
        }

        #[test]
        fn gist_stash_row_attrs() {
            let ctx = GistStashCtx {
                entries: &[GistStashEntry {
                    id: "abc123".to_string(),
                    name: "test".to_string(),
                    href: "/_ghrm/gist?id=abc123".to_string(),
                    modified: Some(1700000000),
                    size: "100 B".to_string(),
                    lines: "10".to_string(),
                    current: false,
                }],
            };
            let html = gist_stash(ctx).unwrap();
            assert!(
                html.contains("data-ghrm-gist-row"),
                "stash row missing data-ghrm-gist-row"
            );
            assert!(
                html.contains("data-ghrm-gist-id=\"abc123\""),
                "stash row missing data-ghrm-gist-id"
            );
            assert!(
                html.contains("data-ghrm-gist-row-link"),
                "stash row missing data-ghrm-gist-row-link"
            );
            assert!(
                html.contains("data-ghrm-gist-rename-start"),
                "stash row missing data-ghrm-gist-rename-start"
            );
        }
    }

    mod search_fragment_contracts {
        use super::*;

        fn path_ctx_with_row() -> PathSearchCtx<'static> {
            static CELLS: [column::Cell; 1] = [column::Cell {
                key: "date",
                class: "ghrm-nav-meta ghrm-nav-meta-time",
                text_class: None,
                text: None,
                timestamp: Some(1700000000),
                hidden: false,
            }];
            static ROWS: [PathSearchRow<'static>; 1] = [PathSearchRow {
                href: String::new(),
                html: String::new(),
                is_dir: false,
                cells: &CELLS,
            }];
            PathSearchCtx {
                pending: false,
                rows: &ROWS,
                empty_colspan: 5,
            }
        }

        #[test]
        fn path_search_row_structure() {
            let html = path_search(path_ctx_with_row()).unwrap();
            assert!(
                html.contains("class=\"ghrm-nav-icon\""),
                "path row missing .ghrm-nav-icon cell"
            );
            assert!(
                html.contains("class=\"ghrm-nav-name\""),
                "path row missing .ghrm-nav-name cell"
            );
            assert!(
                html.contains("class=\"ghrm-search-path\""),
                "path row missing .ghrm-search-path link"
            );
        }

        #[test]
        fn path_search_column_attrs() {
            let html = path_search(path_ctx_with_row()).unwrap();
            assert!(
                html.contains("data-column-key=\"date\""),
                "path row cells missing data-column-key"
            );
            assert!(
                html.contains("data-ts=\"1700000000\""),
                "path row cells missing data-ts"
            );
        }

        #[test]
        fn path_search_empty_state() {
            let ctx = PathSearchCtx {
                pending: false,
                rows: &[],
                empty_colspan: 5,
            };
            let html = path_search(ctx).unwrap();
            assert!(
                html.contains("class=\"ghrm-search-empty\""),
                "empty path search missing .ghrm-search-empty"
            );
        }

        #[test]
        fn path_search_pending_state() {
            let ctx = PathSearchCtx {
                pending: true,
                rows: &[],
                empty_colspan: 5,
            };
            let html = path_search(ctx).unwrap();
            assert!(
                html.contains("Indexing paths"),
                "pending path search should show indexing message"
            );
        }

        #[test]
        fn content_search_row_structure() {
            let ctx = ContentSearchCtx {
                rows: &[ContentSearchRow {
                    href: "/file.rs#L10".to_string(),
                    path: "file.rs".to_string(),
                    line: 10,
                    html: "match".to_string(),
                    modified: Some(1700000000),
                }],
                truncated: false,
                max_rows: 100,
                empty_colspan: 5,
                content_colspan: 3,
                summary_colspan: 4,
            };
            let html = content_search(ctx).unwrap();
            assert!(
                html.contains("class=\"ghrm-content-result\""),
                "content row missing .ghrm-content-result"
            );
            assert!(
                html.contains("class=\"ghrm-content-path\""),
                "content row missing .ghrm-content-path"
            );
            assert!(
                html.contains("class=\"ghrm-content-line\""),
                "content row missing .ghrm-content-line"
            );
            assert!(
                html.contains("class=\"ghrm-content-text\""),
                "content row missing .ghrm-content-text"
            );
        }

        #[test]
        fn content_search_timestamp() {
            let ctx = ContentSearchCtx {
                rows: &[ContentSearchRow {
                    href: "/file.rs".to_string(),
                    path: "file.rs".to_string(),
                    line: 1,
                    html: "match".to_string(),
                    modified: Some(1700000000),
                }],
                truncated: false,
                max_rows: 100,
                empty_colspan: 5,
                content_colspan: 3,
                summary_colspan: 4,
            };
            let html = content_search(ctx).unwrap();
            assert!(
                html.contains("data-ts=\"1700000000\""),
                "content row missing data-ts"
            );
        }

        #[test]
        fn content_search_empty_state() {
            let ctx = ContentSearchCtx {
                rows: &[],
                truncated: false,
                max_rows: 100,
                empty_colspan: 5,
                content_colspan: 3,
                summary_colspan: 4,
            };
            let html = content_search(ctx).unwrap();
            assert!(
                html.contains("class=\"ghrm-search-empty\""),
                "empty content search missing .ghrm-search-empty"
            );
        }

        #[test]
        fn content_search_truncated_state() {
            let ctx = ContentSearchCtx {
                rows: &[ContentSearchRow {
                    href: "/file.rs".to_string(),
                    path: "file.rs".to_string(),
                    line: 1,
                    html: "match".to_string(),
                    modified: None,
                }],
                truncated: true,
                max_rows: 100,
                empty_colspan: 5,
                content_colspan: 3,
                summary_colspan: 4,
            };
            let html = content_search(ctx).unwrap();
            assert!(
                html.contains("class=\"ghrm-search-truncated\""),
                "truncated content search missing .ghrm-search-truncated"
            );
        }

        #[test]
        fn content_search_summary() {
            let ctx = ContentSearchCtx {
                rows: &[ContentSearchRow {
                    href: "/file.rs".to_string(),
                    path: "file.rs".to_string(),
                    line: 1,
                    html: "match".to_string(),
                    modified: None,
                }],
                truncated: false,
                max_rows: 100,
                empty_colspan: 5,
                content_colspan: 3,
                summary_colspan: 4,
            };
            let html = content_search(ctx).unwrap();
            assert!(
                html.contains("class=\"ghrm-search-summary\""),
                "content search missing .ghrm-search-summary"
            );
            assert!(
                html.contains("class=\"ghrm-search-summary-count\""),
                "content search missing .ghrm-search-summary-count"
            );
        }
    }
}
