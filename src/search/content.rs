use crate::explorer::filter;
use crate::paths;

use grep_matcher::Matcher;
use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::{BinaryDetection, SearcherBuilder};
use ignore::WalkBuilder;
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

const MAX_LINE_LEN: usize = 500;

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub path: String,
    pub line: u64,
    pub text: String,
    pub ranges: Vec<(usize, usize)>,
    pub modified: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub truncated: bool,
    pub max_rows: usize,
}

pub struct SearchOpts<'a> {
    pub query: &'a str,
    pub root: &'a Path,
    pub use_ignore: bool,
    pub hidden: bool,
    pub exclude_names: &'a [String],
    pub filter_exts: Option<&'a [String]>,
    pub group_filter: Option<&'a filter::Matcher>,
    pub max_rows: usize,
}

pub fn search(opts: SearchOpts<'_>) -> SearchResponse {
    let matcher = match RegexMatcher::new(opts.query) {
        Ok(m) => m,
        Err(_) => {
            return SearchResponse {
                results: vec![],
                truncated: false,
                max_rows: opts.max_rows,
            };
        }
    };

    let results: Mutex<Vec<SearchResult>> = Mutex::new(Vec::new());
    let truncated = Mutex::new(false);

    let mut walk = WalkBuilder::new(opts.root);
    walk.hidden(!opts.hidden)
        .git_ignore(opts.use_ignore)
        .git_exclude(opts.use_ignore)
        .git_global(opts.use_ignore);

    let filter_exts = opts.filter_exts;
    let group_filter = opts.group_filter;
    let exclude_names = opts.exclude_names;

    walk.build_parallel().run(|| {
        let matcher = matcher.clone();
        let results = &results;
        let truncated = &truncated;

        let mut searcher = SearcherBuilder::new()
            .binary_detection(BinaryDetection::quit(0))
            .line_number(true)
            .build();

        Box::new(move |entry| {
            if *truncated.lock().unwrap() {
                return ignore::WalkState::Quit;
            }

            let entry = match entry {
                Ok(e) => e,
                Err(_) => return ignore::WalkState::Continue,
            };

            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                return ignore::WalkState::Continue;
            }

            let path = entry.path();

            let rel = match path.strip_prefix(opts.root) {
                Ok(r) => r.to_path_buf(),
                Err(_) => return ignore::WalkState::Continue,
            };

            if paths::has_excluded_part(&rel, exclude_names) {
                return ignore::WalkState::Continue;
            }

            if let Some(filter) = group_filter {
                if !filter.matches(&rel) {
                    return ignore::WalkState::Continue;
                }
            } else if let Some(exts) = filter_exts {
                let has_ext = rel
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| exts.iter().any(|x| x == e))
                    .unwrap_or(false);
                if !has_ext {
                    return ignore::WalkState::Continue;
                }
            }

            let rel = rel.to_string_lossy().into_owned();
            let modified = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs());

            let mut file_matches: Vec<SearchResult> = Vec::new();
            let search_result = searcher.search_path(
                &matcher,
                path,
                UTF8(|line_num, line| {
                    if file_matches.len() >= 5 {
                        return Ok(true);
                    }

                    let text = if line.len() > MAX_LINE_LEN {
                        format!("{}...", &line[..MAX_LINE_LEN])
                    } else {
                        line.trim_end().to_string()
                    };

                    let ranges = find_matches(&matcher, &text);

                    file_matches.push(SearchResult {
                        path: rel.clone(),
                        line: line_num,
                        text,
                        ranges,
                        modified,
                    });

                    Ok(true)
                }),
            );

            if search_result.is_ok() && !file_matches.is_empty() {
                let mut guard = results.lock().unwrap();
                for m in file_matches {
                    if guard.len() >= opts.max_rows {
                        *truncated.lock().unwrap() = true;
                        return ignore::WalkState::Quit;
                    }
                    guard.push(m);
                }
            }

            ignore::WalkState::Continue
        })
    });

    let results = results.into_inner().unwrap();
    let truncated = *truncated.lock().unwrap();

    SearchResponse {
        results,
        truncated,
        max_rows: opts.max_rows,
    }
}

fn find_matches(matcher: &RegexMatcher, text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let _ = matcher.find_iter(text.as_bytes(), |m| {
        ranges.push((m.start(), m.end()));
        true
    });
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use std::fs;

    #[test]
    fn search_skips_nested_excluded_paths() {
        let td = TempDir::new("ghrm-search-test");
        fs::create_dir_all(td.path().join("target/debug")).unwrap();
        fs::create_dir_all(td.path().join("src")).unwrap();
        fs::write(td.path().join("target/debug/app.txt"), "needle\n").unwrap();
        fs::write(td.path().join("src/app.txt"), "needle\n").unwrap();

        let resp = search(SearchOpts {
            query: "needle",
            root: td.path(),
            use_ignore: false,
            hidden: true,
            exclude_names: &["target".to_string()],
            filter_exts: None,
            group_filter: None,
            max_rows: 10,
        });

        assert!(resp.results.iter().all(|result| result.modified.is_some()));

        let paths: Vec<_> = resp.results.into_iter().map(|result| result.path).collect();
        assert_eq!(paths, vec!["src/app.txt"]);
    }
}
