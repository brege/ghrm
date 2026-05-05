use super::CommitInfo;

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct LogRequest {
    abs: PathBuf,
    rel: String,
    is_dir: bool,
}

pub(super) fn commit_info(root: &Path, paths: &[PathBuf]) -> BTreeMap<PathBuf, CommitInfo> {
    let requests = paths
        .iter()
        .filter_map(|path| {
            let rel = path.strip_prefix(root).ok()?;
            if rel.as_os_str().is_empty() {
                return None;
            }
            Some(LogRequest {
                abs: path.clone(),
                rel: path_key(rel),
                is_dir: path.is_dir(),
            })
        })
        .collect::<Vec<_>>();
    commit_info_for_requests(root, &requests)
}

fn commit_info_for_requests(root: &Path, requests: &[LogRequest]) -> BTreeMap<PathBuf, CommitInfo> {
    if requests.is_empty() {
        return BTreeMap::new();
    }

    let mut cmd = Command::new("git");
    cmd.arg("--no-pager")
        .arg("-C")
        .arg(root)
        .arg("log")
        .arg("--format=format:%x1f%ct%x1f%s")
        .arg("--name-only")
        .arg("--")
        .args(requests.iter().map(|request| request.rel.as_str()))
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let Ok(mut child) = cmd.spawn() else {
        return BTreeMap::new();
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = child.wait();
        return BTreeMap::new();
    };

    let mut out = BTreeMap::new();
    let mut commit = None::<CommitInfo>;
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if let Some(raw) = line.strip_prefix('\x1f') {
            commit = raw.split_once('\x1f').and_then(|(timestamp, subject)| {
                Some(CommitInfo {
                    subject: subject.to_string(),
                    timestamp: timestamp.parse().ok()?,
                })
            });
            continue;
        }
        let Some(commit) = &commit else {
            continue;
        };
        if line.is_empty() {
            continue;
        }
        for request in requests {
            if out.contains_key(&request.abs) || !log_path_matches(request, &line) {
                continue;
            }
            out.insert(request.abs.clone(), commit.clone());
        }
        if out.len() == requests.len() {
            let _ = child.kill();
            break;
        }
    }
    let _ = child.wait();
    out
}

fn log_path_matches(request: &LogRequest, changed: &str) -> bool {
    if changed == request.rel {
        return true;
    }
    let Some(rest) = changed.strip_prefix(&request.rel) else {
        return false;
    };
    request.is_dir && rest.starts_with('/')
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_path_matches_files_exactly() {
        let request = LogRequest {
            abs: PathBuf::from("src/view.rs"),
            rel: "src/view.rs".to_string(),
            is_dir: false,
        };

        assert!(log_path_matches(&request, "src/view.rs"));
        assert!(!log_path_matches(&request, "src/view.rs.bak"));
    }

    #[test]
    fn log_path_matches_directory_children() {
        let request = LogRequest {
            abs: PathBuf::from("src"),
            rel: "src".to_string(),
            is_dir: true,
        };

        assert!(log_path_matches(&request, "src/view.rs"));
        assert!(!log_path_matches(&request, "src-old/view.rs"));
    }
}
