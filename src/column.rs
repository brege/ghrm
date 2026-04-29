use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Id {
    CommitMessage,
    CommitDate,
    ModifiedDate,
}

pub(crate) struct Def {
    pub(crate) id: Id,
    pub(crate) key: &'static str,
    pub(crate) label: &'static str,
    pub(crate) title: &'static str,
    pub(crate) cell_class: &'static str,
    pub(crate) text_class: Option<&'static str>,
    pub(crate) edge: bool,
    pub(crate) default_visible: bool,
}

pub(crate) const DEFS: &[Def] = &[
    Def {
        id: Id::CommitMessage,
        key: "commit",
        label: "Commit message",
        title: "Show commit messages",
        cell_class: "ghrm-nav-meta ghrm-nav-meta-text ghrm-nav-middle-meta",
        text_class: Some("ghrm-nav-meta-text-value"),
        edge: false,
        default_visible: true,
    },
    Def {
        id: Id::CommitDate,
        key: "commit_date",
        label: "Commit date",
        title: "Show commit dates",
        cell_class: "ghrm-nav-meta ghrm-nav-meta-time ghrm-nav-edge-meta",
        text_class: None,
        edge: true,
        default_visible: false,
    },
    Def {
        id: Id::ModifiedDate,
        key: "date",
        label: "Modified date",
        title: "Show file dates",
        cell_class: "ghrm-nav-meta ghrm-nav-meta-time ghrm-nav-edge-meta",
        text_class: None,
        edge: true,
        default_visible: true,
    },
];

pub(crate) fn default_visible(id: Id) -> bool {
    DEFS.iter()
        .find(|def| def.id == id)
        .map(|def| def.default_visible)
        .unwrap_or(false)
}

pub(crate) fn client_json(defaults: &Set) -> String {
    let columns = DEFS
        .iter()
        .map(|def| {
            serde_json::json!({
                "key": def.key,
                "defaultVisible": defaults.is_visible(def.id),
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
    visible: BTreeSet<Id>,
}

impl Set {
    pub(crate) fn from_defaults(default_for: impl Fn(Id) -> bool) -> Self {
        let visible = DEFS
            .iter()
            .filter_map(|def| default_for(def.id).then_some(def.id))
            .collect();
        Self { visible }
    }

    pub(crate) fn is_visible(&self, id: Id) -> bool {
        self.visible.contains(&id)
    }

    pub(crate) fn set_visible(&mut self, id: Id, visible: bool) {
        if visible {
            self.visible.insert(id);
        } else {
            self.visible.remove(&id);
        }
    }

    pub(crate) fn article_class(&self, base: &str) -> String {
        let mut class = base.to_string();
        if DEFS.iter().any(|def| def.edge && self.is_visible(def.id)) {
            class.push_str(" ghrm-has-edge-meta");
        }
        class
    }

    pub(crate) fn cell(&self, def: &Def, text: Option<String>, timestamp: Option<u64>) -> Cell {
        Cell {
            key: def.key,
            class: def.cell_class,
            text_class: def.text_class,
            text,
            timestamp,
            hidden: !self.is_visible(def.id),
        }
    }

    pub(crate) fn empty_cells(&self) -> Vec<Cell> {
        DEFS.iter().map(|def| self.cell(def, None, None)).collect()
    }

    pub(crate) fn path_cells(&self, modified: Option<u64>) -> Vec<Cell> {
        DEFS.iter()
            .map(|def| match def.id {
                Id::ModifiedDate => self.cell(def, None, modified),
                Id::CommitMessage | Id::CommitDate => Cell {
                    hidden: true,
                    ..self.cell(def, None, None)
                },
            })
            .collect()
    }
}
