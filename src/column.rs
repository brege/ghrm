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
    pub(crate) default_visible: bool,
}

pub(crate) const DEFS: &[Def] = &[
    Def {
        id: Id::ModifiedDate,
        key: "date",
        label: "Date",
        title: "Show file dates",
        default_visible: true,
    },
    Def {
        id: Id::CommitMessage,
        key: "commit",
        label: "Commit message",
        title: "Show commit messages",
        default_visible: true,
    },
    Def {
        id: Id::CommitDate,
        key: "commit_date",
        label: "Commit date",
        title: "Show commit dates",
        default_visible: false,
    },
];

pub(crate) fn default_visible(id: Id) -> bool {
    DEFS.iter()
        .find(|def| def.id == id)
        .map(|def| def.default_visible)
        .unwrap_or(false)
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
}
