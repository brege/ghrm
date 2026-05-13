use anyhow::Result;
use onefetch_manifest::{Manifest, get_manifests};
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct Metadata {
    manifest: Option<Manifest>,
}

impl Metadata {
    pub fn description(&self) -> Option<&str> {
        string(self.manifest.as_ref()?.description.as_deref())
    }

    pub fn version(&self) -> Option<&str> {
        string(self.manifest.as_ref()?.version.as_deref())
    }

    pub fn license(&self) -> Option<&str> {
        string(self.manifest.as_ref()?.license.as_deref())
    }
}

pub fn load(root: &Path) -> Result<Metadata> {
    Ok(Metadata {
        manifest: get_manifests(root)?.into_iter().next(),
    })
}

fn string(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn load_reads_package_json_metadata() {
        let td = TempDir::new("ghrm-stat-metadata-package");
        fs::write(
            td.path().join("package.json"),
            r#"{"name":"app","description":"test app","version":"1.2.3","license":"MIT"}"#,
        )
        .unwrap();

        let metadata = load(td.path()).unwrap();

        assert_eq!(metadata.description(), Some("test app"));
        assert_eq!(metadata.version(), Some("1.2.3"));
        assert_eq!(metadata.license(), Some("MIT"));
    }

    #[test]
    fn load_ignores_unparsed_manifests() {
        let td = TempDir::new("ghrm-stat-metadata-invalid");
        fs::write(td.path().join("Cargo.toml"), "[package]\n").unwrap();

        let metadata = load(td.path()).unwrap();

        assert_eq!(metadata.description(), None);
        assert_eq!(metadata.version(), None);
        assert_eq!(metadata.license(), None);
    }
}
