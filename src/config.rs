use crate::column;

use anyhow::{Result, bail};
use serde::{Deserialize, Deserializer};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    pub port: Option<u16>,
    pub bind: Option<String>,
    pub open: Option<bool>,
    #[serde(default)]
    pub walk: WalkConfig,
    #[serde(default)]
    pub search: SearchConfig,
    #[serde(default)]
    pub explorer: ExplorerConfig,
    #[serde(default)]
    pub auth: AuthConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct WalkConfig {
    pub hidden: Option<bool>,
    pub no_ignore: Option<bool>,
    pub no_excludes: Option<bool>,
    pub extensions: Option<Vec<String>>,
    pub exclude_names: Option<Vec<String>>,
    #[serde(default)]
    pub filter: FilterConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilterConfig {
    pub enabled: Option<bool>,
    pub default_group: Option<String>,
    #[serde(default)]
    pub groups: BTreeMap<String, FilterGroupConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilterGroupConfig {
    pub label: Option<String>,
    pub globs: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct SearchConfig {
    pub max_rows: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ExplorerConfig {
    #[serde(default)]
    pub columns: ColumnConfig,
}

#[derive(Debug, Default)]
pub struct ColumnConfig {
    overrides: BTreeMap<String, bool>,
}

impl<'de> Deserialize<'de> for ColumnConfig {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let overrides = BTreeMap::<String, bool>::deserialize(deserializer)?;
        if let Some(key) = overrides
            .keys()
            .find(|key| column::def_for_config_key(key).is_none())
        {
            return Err(serde::de::Error::custom(format!(
                "unknown explorer column `{key}`"
            )));
        }
        Ok(Self { overrides })
    }
}

impl ColumnConfig {
    pub(crate) fn default_for(&self, def: &column::Def) -> bool {
        self.overrides
            .get(def.config_key)
            .copied()
            .unwrap_or(def.default_visible)
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct AuthConfig {
    pub username: Option<String>,
    pub password: Option<String>,
    pub password_hash: Option<String>,
}

pub fn default_exclude_names() -> Vec<String> {
    vec![
        "node_modules".to_string(),
        "__pycache__".to_string(),
        "target".to_string(),
        ".venv".to_string(),
        ".env".to_string(),
        ".pytest_cache".to_string(),
        ".ruff_cache".to_string(),
        ".uv-cache".to_string(),
        ".ipynb_checkpoints".to_string(),
    ]
}

impl Config {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let explicit = path.is_some();
        let Some(path) = (match path {
            Some(p) => Some(p.to_path_buf()),
            None => path_default()?,
        }) else {
            return Ok(Self::default());
        };
        if !path.is_file() {
            if explicit {
                bail!("missing config file: {}", path.display());
            }
            return Ok(Self::default());
        }
        Ok(toml::from_str(&fs::read_to_string(path)?)?)
    }
}

pub fn path_default() -> Result<Option<PathBuf>> {
    Ok(Some(crate::dirs::config()?.join("config.toml")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_filter_groups() {
        let config: Config = toml::from_str(
            r#"
                [walk.filter]
                enabled = true
                default_group = "docs"

                [walk.filter.groups.docs]
                label = "Docs"
                globs = ["*.md", "*.txt"]

                [walk.filter.groups.web]
                globs = ["*.html", "*.css"]
            "#,
        )
        .unwrap();

        assert_eq!(config.walk.filter.enabled, Some(true));
        assert_eq!(config.walk.filter.default_group.as_deref(), Some("docs"));
        assert_eq!(
            config
                .walk
                .filter
                .groups
                .get("docs")
                .unwrap()
                .label
                .as_deref(),
            Some("Docs")
        );
        assert_eq!(
            config.walk.filter.groups.get("web").unwrap().globs,
            vec!["*.html", "*.css"]
        );
    }

    #[test]
    fn rejects_unknown_filter_group_fields() {
        let err = toml::from_str::<Config>(
            r#"
                [walk.filter.groups.docs]
                label = "Docs"
                extensions = ["md"]
            "#,
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("unknown field `extensions`"));
    }

    #[test]
    fn parses_explorer_columns() {
        let config: Config = toml::from_str(
            r#"
                [explorer.columns]
                date = false
                commit_message = true
                commit_date = false
                size = true
                lines = true
            "#,
        )
        .unwrap();

        assert!(
            !config.explorer.columns.default_for(
                column::DEFS
                    .iter()
                    .find(|def| def.config_key == "date")
                    .unwrap()
            )
        );
        assert!(
            config.explorer.columns.default_for(
                column::DEFS
                    .iter()
                    .find(|def| def.config_key == "commit_message")
                    .unwrap()
            )
        );
        assert!(
            !config.explorer.columns.default_for(
                column::DEFS
                    .iter()
                    .find(|def| def.config_key == "commit_date")
                    .unwrap()
            )
        );
        assert!(
            config.explorer.columns.default_for(
                column::DEFS
                    .iter()
                    .find(|def| def.config_key == "size")
                    .unwrap()
            )
        );
        assert!(
            config.explorer.columns.default_for(
                column::DEFS
                    .iter()
                    .find(|def| def.config_key == "lines")
                    .unwrap()
            )
        );
    }

    #[test]
    fn rejects_unknown_explorer_columns() {
        let err = toml::from_str::<Config>(
            r#"
                [explorer.columns]
                bogus = true
            "#,
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("unknown explorer column `bogus`"));
    }
}
