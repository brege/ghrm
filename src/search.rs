use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

const MAX_RESULTS: usize = 100;

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub path: String,
    pub line: u64,
    pub text: String,
    pub ranges: Vec<(usize, usize)>,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub truncated: bool,
}

pub struct SearchOpts<'a> {
    pub query: &'a str,
    pub root: &'a Path,
    pub hidden: bool,
    pub exclude_names: &'a [String],
    pub filter_exts: Option<&'a [String]>,
}

pub fn search(opts: SearchOpts<'_>) -> SearchResponse {
    let mut cmd = Command::new("rg");
    cmd.arg("--json")
        .arg("--max-count=5")
        .arg("--max-columns=500")
        .arg("--max-columns-preview");

    if opts.hidden {
        cmd.arg("--hidden");
    }

    for name in opts.exclude_names {
        cmd.arg("--glob").arg(format!("!{name}"));
        cmd.arg("--glob").arg(format!("!**/{name}"));
    }

    if let Some(exts) = opts.filter_exts {
        for ext in exts {
            cmd.arg("--glob").arg(format!("*.{ext}"));
        }
    }

    cmd.arg("--").arg(opts.query).arg(opts.root);

    let output = match cmd.output() {
        Ok(o) => o,
        Err(_) => {
            return SearchResponse {
                results: vec![],
                truncated: false,
            };
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();
    let mut truncated = false;

    for line in stdout.lines() {
        if results.len() >= MAX_RESULTS {
            truncated = true;
            break;
        }
        if let Ok(msg) = serde_json::from_str::<RgMessage>(line) {
            if msg.typ == "match" {
                if let Some(r) = parse_match(&msg, opts.root) {
                    results.push(r);
                }
            }
        }
    }

    SearchResponse { results, truncated }
}

#[derive(Deserialize)]
struct RgMessage {
    #[serde(rename = "type")]
    typ: String,
    data: Option<RgData>,
}

#[derive(Deserialize)]
struct RgData {
    path: Option<RgPath>,
    line_number: Option<u64>,
    lines: Option<RgLines>,
    submatches: Option<Vec<RgSubmatch>>,
}

#[derive(Deserialize)]
struct RgPath {
    text: String,
}

#[derive(Deserialize)]
struct RgLines {
    text: String,
}

#[derive(Deserialize)]
struct RgSubmatch {
    start: usize,
    end: usize,
}

fn parse_match(msg: &RgMessage, root: &Path) -> Option<SearchResult> {
    let data = msg.data.as_ref()?;
    let abs_path = &data.path.as_ref()?.text;
    let path = Path::new(abs_path)
        .strip_prefix(root)
        .ok()?
        .to_string_lossy()
        .into_owned();
    let line = data.line_number?;
    let text = data.lines.as_ref()?.text.trim_end().to_string();
    let ranges = data
        .submatches
        .as_ref()
        .map(|subs| subs.iter().map(|s| (s.start, s.end)).collect())
        .unwrap_or_default();

    Some(SearchResult {
        path,
        line,
        text,
        ranges,
    })
}
