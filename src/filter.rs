use crate::config;

use anyhow::{Result, anyhow, bail};
use ignore::types::TypesBuilder;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct GroupMeta {
    pub name: String,
    pub label: String,
    pub detail: String,
}

#[derive(Clone, Debug)]
pub struct Matcher(ignore::types::Types);

impl Matcher {
    pub fn matches(&self, path: &Path) -> bool {
        self.0.matched(path, false).is_whitelist()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Set {
    default_enabled: bool,
    default_groups: Vec<String>,
    groups: Vec<GroupMeta>,
    globs: BTreeMap<String, Vec<String>>,
}

impl Set {
    pub fn resolve(config: &config::FilterConfig) -> Result<Self> {
        let mut groups = Vec::new();
        let mut globs = BTreeMap::new();

        for (raw_name, group) in &config.groups {
            let name = normalize_group_name(raw_name)?;
            let label = normalize_label(group.label.as_deref(), &name)?;
            let globs_for_group = normalize_globs(&group.globs)?;
            groups.push(GroupMeta {
                name: name.clone(),
                label,
                detail: detail_text(&globs_for_group),
            });
            globs.insert(name, globs_for_group);
        }

        let default_groups = match config.default_group.as_deref() {
            Some(name) => {
                let name = normalize_group_name(name)?;
                if !globs.contains_key(&name) {
                    bail!("walk.filter.default_group refers to unknown group `{name}`");
                }
                vec![name]
            }
            None => Vec::new(),
        };

        Ok(Self {
            default_enabled: config.enabled.unwrap_or(false),
            default_groups,
            groups,
            globs,
        })
    }

    pub fn default_enabled(&self) -> bool {
        self.default_enabled
    }

    pub fn default_groups(&self) -> &[String] {
        &self.default_groups
    }

    pub fn groups(&self) -> &[GroupMeta] {
        &self.groups
    }

    pub fn normalize_groups(&self, raw: &[String]) -> Vec<String> {
        let mut groups = Vec::new();
        for group in raw {
            if self.globs.contains_key(group) && !groups.contains(group) {
                groups.push(group.clone());
            }
        }
        groups
    }

    pub fn matcher_for_groups(&self, groups: &[String]) -> Result<Option<Matcher>> {
        let groups = self.normalize_groups(groups);
        if groups.is_empty() {
            return Ok(None);
        }

        let mut globs = BTreeSet::new();
        for group in groups {
            for glob in &self.globs[&group] {
                globs.insert(glob.clone());
            }
        }

        build_matcher(&globs.into_iter().collect::<Vec<_>>()).map(Some)
    }
}

fn normalize_group_name(raw: &str) -> Result<String> {
    let name = raw.trim();
    if name.is_empty() {
        bail!("filter group names must not be empty");
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        bail!("invalid filter group name `{name}`");
    }
    Ok(name.to_string())
}

fn normalize_label(raw: Option<&str>, name: &str) -> Result<String> {
    match raw.map(str::trim) {
        Some("") => bail!("filter group `{name}` has an empty label"),
        Some(label) => Ok(label.to_string()),
        None => Ok(name.to_string()),
    }
}

fn normalize_globs(raw: &[String]) -> Result<Vec<String>> {
    if raw.is_empty() {
        bail!("filter groups must define at least one glob");
    }

    let mut globs = BTreeSet::new();
    for glob in raw {
        let glob = glob.trim();
        if glob.is_empty() {
            bail!("filter globs must not be empty");
        }
        globs.insert(glob.to_string());
    }
    Ok(globs.into_iter().collect())
}

fn build_matcher(globs: &[String]) -> Result<Matcher> {
    let mut builder = TypesBuilder::new();
    for glob in globs {
        builder
            .add("filter", glob)
            .map_err(|err| anyhow!("invalid filter glob `{glob}`: {err}"))?;
    }
    builder.select("filter");
    Ok(Matcher(
        builder
            .build()
            .map_err(|err| anyhow!("invalid filter matcher: {err}"))?,
    ))
}

fn detail_text(globs: &[String]) -> String {
    globs
        .iter()
        .map(|glob| {
            glob.strip_prefix("*.")
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| glob.to_string())
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combines_selected_groups() {
        let mut groups = BTreeMap::new();
        groups.insert(
            "docs".to_string(),
            config::FilterGroupConfig {
                label: Some("Docs".to_string()),
                globs: vec!["*.md".to_string()],
            },
        );
        groups.insert(
            "web".to_string(),
            config::FilterGroupConfig {
                label: Some("Web".to_string()),
                globs: vec!["*.html".to_string()],
            },
        );
        let filters = Set::resolve(&config::FilterConfig {
            enabled: Some(false),
            default_group: Some("docs".to_string()),
            groups,
        })
        .unwrap();

        let matcher = filters
            .matcher_for_groups(&["docs".to_string(), "web".to_string()])
            .unwrap()
            .unwrap();
        assert!(matcher.matches(Path::new("README.md")));
        assert!(matcher.matches(Path::new("index.html")));
        assert!(!matcher.matches(Path::new("main.rs")));
    }

    #[test]
    fn rejects_unknown_default_group() {
        let err = Set::resolve(&config::FilterConfig {
            enabled: Some(true),
            default_group: Some("docs".to_string()),
            groups: BTreeMap::new(),
        })
        .unwrap_err()
        .to_string();

        assert!(err.contains("unknown group `docs`"));
    }
}
