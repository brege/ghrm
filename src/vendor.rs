use anyhow::{Result, bail};
use std::{
    env, fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

pub fn dir() -> Result<PathBuf> {
    if let Some(path) = env::var_os("XDG_CACHE_HOME") {
        return Ok(PathBuf::from(path).join("ghrm"));
    }
    if let Some(home) = env::var_os("HOME") {
        return Ok(PathBuf::from(home).join(".cache/ghrm"));
    }
    bail!("missing HOME and XDG_CACHE_HOME");
}

pub fn path(rel: &str) -> Result<PathBuf> {
    let rel = Path::new(rel);
    for part in rel.components() {
        match part {
            Component::Normal(_) => {}
            _ => bail!("invalid vendor path"),
        }
    }
    Ok(dir()?.join(rel))
}

pub fn sync(refresh: bool) -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let vendor_dir = dir()?;
    let manifest = load_manifest(&root)?;
    for item in manifest["files"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("missing files"))?
    {
        let rel = item["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing path"))?;
        let path = vendor_dir.join(rel);
        if !refresh && path.is_file() {
            continue;
        }
        fs::create_dir_all(
            path.parent()
                .ok_or_else(|| anyhow::anyhow!("missing parent"))?,
        )?;
        let status = Command::new("curl")
            .arg("--location")
            .arg("--fail")
            .arg("--silent")
            .arg("--show-error")
            .arg(
                item["url"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("missing url"))?,
            )
            .arg("--output")
            .arg(&path)
            .status()?;
        if !status.success() {
            bail!("curl failed for {}", path.display());
        }
    }
    let mermaid = fs::read_to_string(vendor_dir.join("mermaid.js"))?;
    let version = mermaid
        .split("version: \"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .unwrap_or("unknown");
    fs::write(
        vendor_dir.join("mermaid-version.txt"),
        format!("{version}\n"),
    )?;
    Ok(())
}

pub fn clean() -> Result<()> {
    let dir = dir()?;
    if dir.is_dir() {
        fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

pub fn ensure() -> Result<()> {
    if missing()?.is_some() {
        sync(false)?;
    }
    Ok(())
}

fn missing() -> Result<Option<PathBuf>> {
    let vendor_dir = dir()?;
    let manifest = load_manifest(&PathBuf::from(env!("CARGO_MANIFEST_DIR")))?;
    for item in manifest["files"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("missing files"))?
    {
        let rel = item["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing path"))?;
        let path = vendor_dir.join(rel);
        if !path.is_file() {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn load_manifest(root: &Path) -> Result<serde_json::Value> {
    Ok(serde_json::from_str(&fs::read_to_string(
        root.join("assets/config.json"),
    )?)?)
}
