use anyhow::{Result, anyhow};
use ignore::{WalkBuilder, WalkState, types::TypesBuilder};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use sysinfo::Disks;

#[derive(Clone, Debug, Default)]
pub struct FsConfig {
    pub hidden: bool,
    pub use_ignore: bool,
    pub show_excludes: bool,
    pub exclude_names: Vec<String>,
    pub same_file_system: bool,
    pub filter_groups: Vec<FsFilterGroup>,
}

#[derive(Clone, Debug)]
pub struct FsFilterGroup {
    pub name: String,
    pub label: String,
    pub globs: Vec<String>,
    pub default_enabled: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReport {
    pub root: PathBuf,
    pub file_system: Option<String>,
    pub totals: FsTotals,
    pub max_depth: usize,
    pub filters: Vec<FsFilterTotal>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsTotals {
    pub files: usize,
    pub dirs: usize,
    pub symlinks: usize,
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsFilterTotal {
    pub name: String,
    pub label: String,
    pub default_enabled: bool,
    pub totals: FsTotals,
}

struct FilterGroup {
    name: String,
    label: String,
    default_enabled: bool,
    matcher: ignore::types::Types,
}

#[derive(Clone)]
struct FileEntry {
    rel: PathBuf,
    bytes: u64,
}

#[derive(Default)]
struct ScanState {
    totals: FsTotals,
    dirs: BTreeSet<PathBuf>,
    files: Vec<FileEntry>,
    max_depth: usize,
}

pub fn scan(input: &Path, config: &FsConfig) -> Result<FsReport> {
    let root = input.canonicalize()?;
    let filter_groups = build_filter_groups(&config.filter_groups)?;
    let state = Arc::new(Mutex::new(ScanState::default()));
    let exclude_names = config
        .exclude_names
        .iter()
        .map(|name| name.as_str())
        .collect::<Vec<_>>();

    let mut builder = WalkBuilder::new(&root);
    builder
        .hidden(false)
        .follow_links(true)
        .same_file_system(config.same_file_system)
        .require_git(false)
        .git_ignore(config.use_ignore)
        .git_exclude(config.use_ignore)
        .git_global(config.use_ignore);

    builder.build_parallel().run(|| {
        let root = root.clone();
        let hidden = config.hidden;
        let show_excludes = config.show_excludes;
        let exclude_names = exclude_names.clone();
        let state = state.clone();
        Box::new(move |res| {
            let entry = match res {
                Ok(entry) => entry,
                Err(_) => return WalkState::Continue,
            };
            let path = entry.path();
            if path == root {
                return WalkState::Continue;
            }
            let rel = match path.strip_prefix(&root) {
                Ok(rel) => rel.to_path_buf(),
                Err(_) => return WalkState::Continue,
            };
            if !hidden && has_hidden_part(&rel) {
                return if entry
                    .file_type()
                    .is_some_and(|file_type| file_type.is_dir())
                {
                    WalkState::Skip
                } else {
                    WalkState::Continue
                };
            }
            let excluded = has_excluded_part(&rel, &exclude_names);
            if excluded && !show_excludes {
                return if entry
                    .file_type()
                    .is_some_and(|file_type| file_type.is_dir())
                {
                    WalkState::Skip
                } else {
                    WalkState::Continue
                };
            }

            let Some(file_type) = entry.file_type() else {
                return WalkState::Continue;
            };
            let mut state = state.lock().unwrap();
            state.max_depth = state.max_depth.max(depth(&rel));

            if entry.path_is_symlink() {
                state.totals.symlinks += 1;
                return WalkState::Continue;
            }
            if file_type.is_dir() {
                state.dirs.insert(rel);
                if excluded {
                    return WalkState::Skip;
                }
                return WalkState::Continue;
            }
            if file_type.is_file() {
                let bytes = entry
                    .metadata()
                    .ok()
                    .map(|metadata| metadata.len())
                    .unwrap_or(0);
                state.totals.files += 1;
                state.totals.bytes += bytes;
                state.files.push(FileEntry { rel, bytes });
            }
            WalkState::Continue
        })
    });

    let mut state = state.lock().unwrap();
    state.totals.dirs = state.dirs.len();
    Ok(FsReport {
        root,
        file_system: file_system(input),
        filters: filter_totals(&state.files, &filter_groups),
        totals: state.totals.clone(),
        max_depth: state.max_depth,
    })
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

fn build_filter_groups(groups: &[FsFilterGroup]) -> Result<Vec<FilterGroup>> {
    groups
        .iter()
        .map(|group| {
            let mut builder = TypesBuilder::new();
            for glob in &group.globs {
                builder
                    .add("filter", glob)
                    .map_err(|err| anyhow!("invalid filter glob `{glob}`: {err}"))?;
            }
            builder.select("filter");
            Ok(FilterGroup {
                name: group.name.clone(),
                label: group.label.clone(),
                default_enabled: group.default_enabled,
                matcher: builder
                    .build()
                    .map_err(|err| anyhow!("invalid filter matcher: {err}"))?,
            })
        })
        .collect()
}

fn filter_totals(files: &[FileEntry], groups: &[FilterGroup]) -> Vec<FsFilterTotal> {
    let mut out = Vec::with_capacity(groups.len());
    for group in groups {
        let mut totals = FsTotals::default();
        let mut dirs = BTreeSet::<PathBuf>::new();
        for file in files
            .iter()
            .filter(|file| group.matcher.matched(&file.rel, false).is_whitelist())
        {
            totals.files += 1;
            totals.bytes += file.bytes;
            for dir in ancestors(&file.rel) {
                dirs.insert(dir);
            }
        }
        totals.dirs = dirs.len();
        out.push(FsFilterTotal {
            name: group.name.clone(),
            label: group.label.clone(),
            default_enabled: group.default_enabled,
            totals,
        });
    }
    out
}

fn ancestors(path: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut current = path.parent().unwrap_or(Path::new("")).to_path_buf();
    while !current.as_os_str().is_empty() {
        out.push(current.clone());
        current = current.parent().unwrap_or(Path::new("")).to_path_buf();
    }
    out
}

fn file_system(path: &Path) -> Option<String> {
    let path = path.canonicalize().ok()?;
    Disks::new_with_refreshed_list()
        .list()
        .iter()
        .filter(|disk| path.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().components().count())
        .map(|disk| disk.file_system().to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
}

fn has_hidden_part(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|name| name.starts_with('.') && name != "." && name != "..")
    })
}

fn has_excluded_part(path: &Path, exclude_names: &[&str]) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|name| exclude_names.contains(&name))
    })
}

fn depth(path: &Path) -> usize {
    path.components().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("{prefix}-{}-{}", std::process::id(), rand()));
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn filesystem_scan_counts_visible_files_and_filters() {
        let td = TempDir::new("ghrm-stat-fs");
        std::fs::create_dir_all(td.path.join("docs")).unwrap();
        std::fs::create_dir_all(td.path.join("target")).unwrap();
        std::fs::write(td.path.join("docs/guide.md"), "guide").unwrap();
        std::fs::write(td.path.join("src.rs"), "fn main() {}\n").unwrap();
        std::fs::write(td.path.join("target/app"), "binary").unwrap();

        let report = scan(
            &td.path,
            &FsConfig {
                use_ignore: true,
                exclude_names: vec!["target".to_string()],
                filter_groups: vec![FsFilterGroup {
                    name: "docs".to_string(),
                    label: "Docs".to_string(),
                    globs: vec!["*.md".to_string()],
                    default_enabled: true,
                }],
                same_file_system: true,
                ..FsConfig::default()
            },
        )
        .unwrap();

        assert_eq!(report.totals.files, 2);
        assert_eq!(report.totals.dirs, 1);
        assert_eq!(report.filters[0].totals.files, 1);
        assert_eq!(report.filters[0].totals.dirs, 1);
        assert!(report.filters[0].default_enabled);
    }

    fn rand() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }
}
