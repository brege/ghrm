use serde::Serialize;
use std::collections::BTreeSet;
use std::ops::BitOr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MetaReq(u8);

impl MetaReq {
    pub(crate) const NONE: Self = Self(0);
    pub(crate) const LINES: Self = Self(1 << 0);
    pub(crate) const COMMIT: Self = Self(1 << 1);

    pub(crate) fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for MetaReq {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

pub(crate) struct Def {
    pub(crate) key: &'static str,
    pub(crate) config_key: &'static str,
    pub(crate) label: &'static str,
    pub(crate) title: &'static str,
    pub(crate) cell_class: &'static str,
    pub(crate) text_class: Option<&'static str>,
    pub(crate) edge: bool,
    pub(crate) default_visible: bool,
    pub(crate) requires: MetaReq,
    render: fn(&RowMeta<'_>) -> CellValue,
}

pub(crate) const DEFS: &[Def] = &[
    Def {
        key: "commit",
        config_key: "commit_message",
        label: "Commit message",
        title: "Show commit messages",
        cell_class: "ghrm-nav-meta ghrm-nav-meta-text ghrm-nav-middle-meta",
        text_class: Some("ghrm-nav-meta-text-value"),
        edge: false,
        default_visible: false,
        requires: MetaReq::COMMIT,
        render: render_commit_subject,
    },
    Def {
        key: "commit_date",
        config_key: "commit_date",
        label: "Commit date",
        title: "Show commit dates",
        cell_class: "ghrm-nav-meta ghrm-nav-meta-time ghrm-nav-edge-meta",
        text_class: None,
        edge: true,
        default_visible: false,
        requires: MetaReq::COMMIT,
        render: render_commit_timestamp,
    },
    Def {
        key: "size",
        config_key: "size",
        label: "Size",
        title: "Show file sizes",
        cell_class: "ghrm-nav-meta ghrm-nav-meta-number ghrm-nav-edge-meta",
        text_class: None,
        edge: true,
        default_visible: false,
        requires: MetaReq::NONE,
        render: render_size,
    },
    Def {
        key: "lines",
        config_key: "lines",
        label: "Lines",
        title: "Show line counts",
        cell_class: "ghrm-nav-meta ghrm-nav-meta-number ghrm-nav-edge-meta",
        text_class: None,
        edge: true,
        default_visible: false,
        requires: MetaReq::LINES,
        render: render_lines,
    },
    Def {
        key: "date",
        config_key: "date",
        label: "Modified date",
        title: "Show file dates",
        cell_class: "ghrm-nav-meta ghrm-nav-meta-time ghrm-nav-edge-meta",
        text_class: None,
        edge: true,
        default_visible: true,
        requires: MetaReq::NONE,
        render: render_modified,
    },
];

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RowMeta<'a> {
    pub(crate) modified: Option<u64>,
    pub(crate) size: Option<u64>,
    pub(crate) lines: Option<u64>,
    pub(crate) commit_subject: Option<&'a str>,
    pub(crate) commit_timestamp: Option<u64>,
}

impl RowMeta<'_> {
    pub(crate) fn cells(&self, columns: &Set) -> Vec<Cell> {
        DEFS.iter().map(|def| columns.cell(def, self)).collect()
    }
}

#[derive(Clone, Debug)]
struct CellValue {
    text: Option<String>,
    timestamp: Option<u64>,
}

impl CellValue {
    fn text(text: Option<String>) -> Self {
        Self {
            text,
            timestamp: None,
        }
    }

    fn timestamp(timestamp: Option<u64>) -> Self {
        Self {
            text: None,
            timestamp,
        }
    }
}

fn render_commit_subject(meta: &RowMeta<'_>) -> CellValue {
    CellValue::text(meta.commit_subject.map(str::to_string))
}

fn render_commit_timestamp(meta: &RowMeta<'_>) -> CellValue {
    CellValue::timestamp(meta.commit_timestamp)
}

fn render_size(meta: &RowMeta<'_>) -> CellValue {
    CellValue::text(size_text(meta.size))
}

fn render_lines(meta: &RowMeta<'_>) -> CellValue {
    CellValue::text(count_text(meta.lines))
}

fn render_modified(meta: &RowMeta<'_>) -> CellValue {
    CellValue::timestamp(meta.modified)
}

pub(crate) fn def_for_config_key(key: &str) -> Option<&'static Def> {
    DEFS.iter().find(|def| def.config_key == key)
}

pub(crate) fn required_meta(columns: &Set) -> MetaReq {
    DEFS.iter()
        .filter(|def| columns.is_visible(def))
        .fold(MetaReq::NONE, |acc, def| acc | def.requires)
}

pub(crate) fn client_json(defaults: &Set) -> String {
    let columns = DEFS
        .iter()
        .map(|def| {
            serde_json::json!({
                "key": def.key,
                "defaultVisible": defaults.is_visible(def),
                "edge": def.edge,
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&columns).expect("column config serializes")
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Cell {
    pub(crate) key: &'static str,
    pub(crate) class: &'static str,
    pub(crate) text_class: Option<&'static str>,
    pub(crate) text: Option<String>,
    pub(crate) timestamp: Option<u64>,
    pub(crate) hidden: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Set {
    visible: BTreeSet<&'static str>,
}

impl Set {
    pub(crate) fn from_defaults(default_for: impl Fn(&Def) -> bool) -> Self {
        let visible = DEFS
            .iter()
            .filter_map(|def| default_for(def).then_some(def.key))
            .collect();
        Self { visible }
    }

    pub(crate) fn is_visible(&self, def: &Def) -> bool {
        self.visible.contains(def.key)
    }

    pub(crate) fn set_visible(&mut self, def: &'static Def, visible: bool) {
        if visible {
            self.visible.insert(def.key);
        } else {
            self.visible.remove(def.key);
        }
    }

    pub(crate) fn article_class(&self, base: &str) -> String {
        let mut class = base.to_string();
        if DEFS.iter().any(|def| def.edge && self.is_visible(def)) {
            class.push_str(" ghrm-has-edge-meta");
        }
        class
    }

    pub(crate) fn cell(&self, def: &Def, meta: &RowMeta<'_>) -> Cell {
        let value = (def.render)(meta);
        Cell {
            key: def.key,
            class: def.cell_class,
            text_class: def.text_class,
            text: value.text,
            timestamp: value.timestamp,
            hidden: !self.is_visible(def),
        }
    }

    pub(crate) fn empty_cells(&self) -> Vec<Cell> {
        RowMeta::default().cells(self)
    }
}

pub(crate) fn count_text(count: Option<u64>) -> Option<String> {
    count.map(|count| count.to_string())
}

pub(crate) fn size_text(size: Option<u64>) -> Option<String> {
    let size = size?;
    if size < 1024 {
        return Some(format!("{size} B"));
    }

    let mut value = size as f64;
    let mut unit = "B";
    for next_unit in ["KB", "MB", "GB", "TB"] {
        if value < 1024.0 {
            break;
        }
        value /= 1024.0;
        unit = next_unit;
    }

    if value < 10.0 {
        Some(format!("{value:.1} {unit}"))
    } else {
        Some(format!("{value:.0} {unit}"))
    }
}
