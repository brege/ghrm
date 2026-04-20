use anyhow::{Result, bail};
use include_dir::{Dir, include_dir};
use std::{env, fs, path::PathBuf};

const THEME_VERSION: &str = include_str!(concat!(env!("OUT_DIR"), "/theme_version.txt"));

static EMBEDDED: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets");

pub fn dir() -> Result<PathBuf> {
    if let Some(path) = env::var_os("GHRM_THEME_DIR") {
        return Ok(PathBuf::from(path));
    }
    if let Some(path) = env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(path).join("ghrm/theme"));
    }
    if let Some(home) = env::var_os("HOME") {
        return Ok(PathBuf::from(home).join(".local/share/ghrm/theme"));
    }
    bail!("missing HOME and XDG_DATA_HOME");
}

pub fn ensure() -> Result<()> {
    if env::var_os("GHRM_THEME_DIR").is_some() {
        return Ok(());
    }
    let d = dir()?;
    if fs::read_to_string(d.join("VERSION")).ok().as_deref() == Some(THEME_VERSION) {
        return Ok(());
    }
    install(&d)?;
    Ok(())
}

fn install(dest: &std::path::Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    EMBEDDED.extract(dest)?;
    // Vendor files are managed separately by vendor.rs; remove from theme dir
    let vendor_dir = dest.join("vendor");
    if vendor_dir.is_dir() {
        fs::remove_dir_all(&vendor_dir)?;
    }
    fs::write(dest.join("VERSION"), THEME_VERSION)?;
    Ok(())
}

pub fn clean() -> Result<()> {
    if env::var_os("GHRM_THEME_DIR").is_some() {
        return Ok(());
    }
    let d = dir()?;
    if d.is_dir() {
        fs::remove_dir_all(&d)?;
    }
    Ok(())
}
