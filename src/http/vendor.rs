use crate::render::Rendered;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::{
    fs, io,
    path::{Component, Path, PathBuf},
    sync::OnceLock,
};

const COMMON_FEATURE: &str = "common";
const MERMAID_FEATURE: &str = "mermaid";
const MATH_FEATURE: &str = "math";
const MAP_FEATURE: &str = "map";

pub fn plan(r: &Rendered) -> Assets {
    manifest().plan(&feature_names(r))
}

pub fn feature_list(r: &Rendered) -> String {
    feature_names(r).join(" ")
}

pub fn client_json() -> &'static str {
    static CLIENT_JSON: OnceLock<String> = OnceLock::new();
    CLIENT_JSON
        .get_or_init(|| manifest().client_json())
        .as_str()
}

fn feature_names(r: &Rendered) -> Vec<&'static str> {
    [
        (r.has_mermaid, MERMAID_FEATURE),
        (r.has_math, MATH_FEATURE),
        (r.has_map, MAP_FEATURE),
    ]
    .into_iter()
    .filter_map(|(enabled, name)| enabled.then_some(name))
    .collect()
}

pub fn dir() -> Result<PathBuf> {
    crate::dirs::cache()
}

pub fn path(rel: &str) -> Result<PathBuf> {
    validate_rel(rel)?;
    Ok(dir()?.join(rel))
}

pub fn sync(refresh: bool) -> Result<()> {
    let vendor_dir = dir()?;
    let manifest = manifest();
    for item in &manifest.files {
        let path = vendor_dir.join(&item.path);
        if !refresh && path.is_file() {
            continue;
        }
        fs::create_dir_all(
            path.parent()
                .ok_or_else(|| anyhow::anyhow!("missing parent"))?,
        )?;
        download(&item.url, &path)?;
    }
    let mermaid = fs::read_to_string(vendor_dir.join(&manifest.mermaid_version.source))?;
    let version = mermaid
        .split("version: \"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .unwrap_or("unknown");
    fs::write(
        vendor_dir.join(&manifest.mermaid_version.path),
        format!("{version}\n"),
    )?;
    Ok(())
}

fn download(url: &str, path: &Path) -> Result<()> {
    let response = ureq::get(url)
        .call()
        .with_context(|| format!("download failed: {url}"))?;
    let mut reader = response.into_reader();
    let mut file = fs::File::create(path)?;
    io::copy(&mut reader, &mut file)?;
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
    missing_in(&dir()?, manifest())
}

fn missing_in(vendor_dir: &Path, manifest: &Manifest) -> Result<Option<PathBuf>> {
    for item in &manifest.files {
        let path = vendor_dir.join(&item.path);
        if !path.is_file() {
            return Ok(Some(path));
        }
    }
    let generated = vendor_dir.join(&manifest.mermaid_version.path);
    if !generated.is_file() {
        return Ok(Some(generated));
    }
    Ok(None)
}

#[derive(Deserialize)]
struct Manifest {
    base_path: String,
    features: BTreeMap<String, Assets>,
    mermaid_version: Generated,
    files: Vec<FileAsset>,
}

impl Manifest {
    fn plan(&self, active: &[&str]) -> Assets {
        let mut styles = Vec::new();
        let mut scripts = Vec::new();
        let mut seen = BTreeSet::new();

        self.push_feature(COMMON_FEATURE, &mut seen, &mut styles, &mut scripts);
        for feature in active {
            self.push_feature(feature, &mut seen, &mut styles, &mut scripts);
        }

        Assets { styles, scripts }
    }

    fn push_feature<'a>(
        &'a self,
        name: &str,
        seen: &mut BTreeSet<&'a str>,
        styles: &mut Vec<String>,
        scripts: &mut Vec<String>,
    ) {
        let Some(feature) = self.features.get(name) else {
            return;
        };
        for path in &feature.styles {
            if seen.insert(path.as_str()) {
                styles.push(self.public_url(path));
            }
        }
        for path in &feature.scripts {
            if seen.insert(path.as_str()) {
                scripts.push(self.public_url(path));
            }
        }
    }

    fn public_url(&self, path: &str) -> String {
        format!("{}{}", self.base_path, path)
    }

    fn client_json(&self) -> String {
        let features = self
            .features
            .iter()
            .map(|(name, assets)| {
                (
                    name,
                    json!({
                    "styles": self.public_urls(&assets.styles),
                    "scripts": self.public_urls(&assets.scripts),
                    }),
                )
            })
            .collect::<BTreeMap<_, _>>();

        json!({
            "features": features,
            "mermaidVersion": self.public_url(&self.mermaid_version.path),
        })
        .to_string()
    }

    fn public_urls(&self, paths: &[String]) -> Vec<String> {
        paths.iter().map(|path| self.public_url(path)).collect()
    }

    fn validate(&self) -> Result<()> {
        if !self.base_path.starts_with('/') || !self.base_path.ends_with('/') {
            bail!("invalid vendor base path");
        }
        if !self.features.contains_key(COMMON_FEATURE) {
            bail!("missing common feature");
        }
        let mut paths = BTreeSet::new();
        for item in &self.files {
            validate_rel(&item.path)?;
            paths.insert(item.path.as_str());
        }
        for feature in self.features.values() {
            for path in feature.styles.iter().chain(&feature.scripts) {
                validate_rel(path)?;
                if !paths.contains(path.as_str()) {
                    bail!("unknown feature asset: {path}");
                }
            }
        }
        validate_rel(&self.mermaid_version.source)?;
        validate_rel(&self.mermaid_version.path)?;
        if !paths.contains(self.mermaid_version.source.as_str()) {
            bail!(
                "unknown generated asset source: {}",
                self.mermaid_version.source
            );
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct Generated {
    source: String,
    path: String,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Assets {
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default)]
    pub scripts: Vec<String>,
}

#[derive(Deserialize)]
struct FileAsset {
    url: String,
    path: String,
}

fn manifest() -> &'static Manifest {
    static MANIFEST: OnceLock<Manifest> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        manifest_from_str(include_str!("../../assets/config.json"))
            .expect("embedded vendor manifest is valid")
    })
}

fn manifest_from_str(raw: &str) -> Result<Manifest> {
    let manifest: Manifest = serde_json::from_str(raw)?;
    manifest.validate()?;
    Ok(manifest)
}

fn validate_rel(rel: &str) -> Result<()> {
    let rel = Path::new(rel);
    let mut saw_part = false;
    for part in rel.components() {
        match part {
            Component::Normal(_) => saw_part = true,
            _ => bail!("invalid vendor path"),
        }
    }
    if !saw_part {
        bail!("invalid vendor path");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use std::fs;

    fn rendered(has_mermaid: bool, has_math: bool, has_map: bool) -> Rendered {
        Rendered {
            html: String::new(),
            title: String::new(),
            lang: None,
            has_mermaid,
            has_math,
            has_map,
        }
    }

    #[test]
    fn plan_includes_common_assets() {
        let plan = plan(&rendered(false, false, false));
        let common = manifest().features.get(COMMON_FEATURE).unwrap();

        assert_eq!(
            plan.styles,
            common
                .styles
                .iter()
                .map(|path| manifest().public_url(path))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            plan.scripts,
            common
                .scripts
                .iter()
                .map(|path| manifest().public_url(path))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn plan_expands_feature_assets() {
        let plan = plan(&rendered(true, true, true));

        assert_eq!(
            feature_list(&rendered(true, true, true)),
            "mermaid math map"
        );
        assert!(plan.styles.len() > manifest().features[COMMON_FEATURE].styles.len());
        assert!(plan.scripts.len() > manifest().features[COMMON_FEATURE].scripts.len());
    }

    #[test]
    fn client_json_exposes_assets() {
        let value: serde_json::Value = serde_json::from_str(client_json()).unwrap();

        assert_eq!(
            value["features"]["math"]["styles"][0],
            manifest().public_url("katex/katex.min.css")
        );
        assert_eq!(
            value["mermaidVersion"],
            manifest().public_url(&manifest().mermaid_version.path)
        );
    }

    #[test]
    fn missing_uses_manifest_files_and_generated_asset() {
        let td = TempDir::new("ghrm-vendor-missing");

        let missing = missing_in(td.path(), manifest()).unwrap().unwrap();
        assert_eq!(missing, td.path().join(&manifest().files[0].path));

        for item in &manifest().files {
            let path = td.path().join(&item.path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "asset").unwrap();
        }
        let generated = td.path().join(&manifest().mermaid_version.path);
        assert_eq!(
            missing_in(td.path(), manifest()).unwrap(),
            Some(generated.clone())
        );

        fs::write(&generated, "11.0.0\n").unwrap();
        assert_eq!(missing_in(td.path(), manifest()).unwrap(), None);
    }
}
