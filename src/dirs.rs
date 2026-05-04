use anyhow::{Result, anyhow};
use directories::ProjectDirs;
use std::path::PathBuf;

const APP: &str = "ghrm";

fn project() -> Result<ProjectDirs> {
    ProjectDirs::from("", "", APP).ok_or_else(|| anyhow!("missing project directories"))
}

pub(crate) fn cache() -> Result<PathBuf> {
    Ok(project()?.cache_dir().to_path_buf())
}

pub(crate) fn data() -> Result<PathBuf> {
    Ok(project()?.data_dir().to_path_buf())
}

pub(crate) fn config() -> Result<PathBuf> {
    Ok(project()?.config_dir().to_path_buf())
}
