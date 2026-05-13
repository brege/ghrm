use crate::{Context, Row, metadata};
use anyhow::{Result, anyhow};
use askalono::{Store, TextData};
use std::{ffi::OsStr, fs, path::Path, sync::OnceLock};

const LICENSE_FILES: [&str; 3] = ["LICENSE", "LICENCE", "COPYING"];
const MIN_THRESHOLD: f32 = 0.8;

static CACHE_DATA: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/license.cache.zstd"
));
static STORE: OnceLock<Result<Store, String>> = OnceLock::new();

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let license = metadata(ctx)
        .ok()
        .and_then(|metadata| metadata.license().map(str::to_string))
        .unwrap_or_else(|| detect_license(&ctx.root).unwrap_or_default());

    Ok(vec![Row::new("license", license)])
}

struct Detector {
    store: &'static Store,
}

impl Detector {
    fn new() -> Result<Self> {
        Ok(Self { store: store()? })
    }

    fn detect(&self, dir: &Path) -> Result<String> {
        let mut output = fs::read_dir(dir)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|entry| {
                entry.is_file()
                    && entry
                        .file_name()
                        .map(OsStr::to_string_lossy)
                        .is_some_and(is_license_file)
            })
            .filter_map(|entry| {
                let contents = fs::read_to_string(entry).unwrap_or_default();
                self.analyze(&contents)
            })
            .collect::<Vec<_>>();

        output.sort();
        output.dedup();
        Ok(output.join(", "))
    }

    fn analyze(&self, text: &str) -> Option<String> {
        let matched = self.store.analyze(&TextData::from(text));
        (matched.score >= MIN_THRESHOLD).then(|| matched.name.into())
    }
}

fn detect_license(dir: &Path) -> Result<String> {
    Detector::new()?.detect(dir)
}

fn store() -> Result<&'static Store> {
    let result = STORE.get_or_init(|| Store::from_cache(CACHE_DATA).map_err(|err| err.to_string()));
    result.as_ref().map_err(|message| anyhow!(message.clone()))
}

fn is_license_file<S: AsRef<str>>(file_name: S) -> bool {
    LICENSE_FILES
        .iter()
        .any(|name| file_name.as_ref().starts_with(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn license_file_names_match_onefetch() {
        assert!(is_license_file("LICENSE"));
        assert!(is_license_file("LICENSE.md"));
        assert!(is_license_file("LICENCE"));
        assert!(is_license_file("COPYING"));
        assert!(!is_license_file("NOT_LICENSE"));
    }
}
