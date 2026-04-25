use anyhow::{Result, bail};
use serde::Deserialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    pub port: Option<u16>,
    pub bind: Option<String>,
    pub open: Option<bool>,
    #[serde(default)]
    pub walk: WalkConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct WalkConfig {
    pub hidden: Option<bool>,
    pub no_ignore: Option<bool>,
    pub no_excludes: Option<bool>,
    pub extensions: Option<Vec<String>>,
    pub exclude_names: Option<Vec<String>>,
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
    if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
        return Ok(Some(PathBuf::from(path).join("ghrm/config.toml")));
    }
    if let Some(home) = env::var_os("HOME") {
        return Ok(Some(PathBuf::from(home).join(".config/ghrm/config.toml")));
    }
    bail!("missing HOME and XDG_CONFIG_HOME");
}
