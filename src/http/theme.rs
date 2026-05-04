use anyhow::Result;
use include_dir::{Dir, DirEntry, include_dir};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

const THEME_VERSION: &str = include_str!(concat!(env!("OUT_DIR"), "/theme_version.txt"));

static CSS: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/css");
static IMG: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/img");
static JS: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/js");

pub fn dir() -> Result<PathBuf> {
    if let Some(path) = env::var_os("GHRM_THEME_DIR") {
        return Ok(PathBuf::from(path));
    }
    Ok(crate::dirs::data()?.join("theme"))
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
    if dest.is_dir() {
        fs::remove_dir_all(dest)?;
    }
    fs::create_dir_all(dest)?;
    install_dir(&CSS, &dest.join("css"))?;
    install_dir(&IMG, &dest.join("img"))?;
    install_dir(&JS, &dest.join("js"))?;
    fs::write(dest.join("VERSION"), THEME_VERSION)?;
    Ok(())
}

fn install_dir(dir: &Dir<'_>, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(dir) => {
                fs::create_dir_all(dest.join(dir.path()))?;
                install_dir(dir, dest)?;
            }
            DirEntry::File(file) => {
                let path = dest.join(file.path());
                fs::create_dir_all(path.parent().unwrap())?;
                fs::write(path, file.contents())?;
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn install_writes_only_runtime_theme_assets() {
        let td = TempDir::new("ghrm-theme-install");

        install(td.path()).unwrap();

        assert!(td.path().join("css/page.css").is_file());
        assert!(td.path().join("js/main.js").is_file());
        assert!(td.path().join("img/favicon.svg").is_file());
        assert!(td.path().join("VERSION").is_file());
        assert!(!td.path().join("vendor").exists());
        assert!(!td.path().join("templates").exists());
        assert!(!td.path().join("config.json").exists());
    }
}
