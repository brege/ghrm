use anyhow::Result;
use std::path::Path;

#[derive(Clone)]
pub(crate) struct Paths {
    rows: Vec<PathRow>,
}

#[derive(Clone)]
pub(crate) struct PathRow {
    pub(crate) label: &'static str,
    pub(crate) value: String,
}

impl Paths {
    pub(crate) fn new(target: &Path, config: Option<&Path>) -> Result<Self> {
        let mut rows = vec![row("root", target)];
        if let Some(config) = config {
            rows.push(row("config", config));
        }
        rows.push(row("theme", crate::http::theme::dir()?));
        rows.push(row("vendor", crate::http::vendor::dir()?));
        Ok(Self { rows })
    }

    pub(crate) fn rows(&self) -> &[PathRow] {
        &self.rows
    }
}

fn row(label: &'static str, path: impl AsRef<Path>) -> PathRow {
    PathRow {
        label,
        value: path.as_ref().to_string_lossy().into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn rows_include_target_config_theme_and_vendor() {
        let td = TempDir::new("ghrm-runtime-paths");
        let config = td.path().join("config.toml");

        let paths = Paths::new(td.path(), Some(&config)).unwrap();
        let labels: Vec<&str> = paths.rows().iter().map(|row| row.label).collect();

        assert_eq!(labels, vec!["root", "config", "theme", "vendor"]);
    }
}
