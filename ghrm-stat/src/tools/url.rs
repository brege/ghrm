use crate::{Context, Row, repo};
use anyhow::Result;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let url = remote_url(repo(ctx)).unwrap_or_default();
    Ok(vec![Row::new("url", url)])
}

// Reads the configured origin remote URL from git config, or None when it
// is unset.
fn remote_url(repo: &gix::Repository) -> Option<String> {
    repo.config_snapshot()
        .string("remote.origin.url")
        .map(|url| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn repo_with_config(name: &str, body: &str) -> (std::path::PathBuf, gix::Repository) {
        let dir = std::env::temp_dir().join(format!("ghrm-stat-url-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        gix::init(&dir).expect("init repository");
        let config = dir.join(".git/config");
        let mut text = fs::read_to_string(&config).expect("read repository config");
        text.push_str(body);
        fs::write(&config, text).unwrap();
        let repo = gix::open(&dir).expect("open repository");
        (dir, repo)
    }

    #[test]
    fn reads_the_origin_url() {
        let (dir, repo) = repo_with_config(
            "origin",
            "[remote \"origin\"]\n\turl = https://example.com/a/b.git\n",
        );
        assert_eq!(
            remote_url(&repo),
            Some("https://example.com/a/b.git".to_string())
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_origin_yields_none() {
        let (dir, repo) = repo_with_config("none", "");
        assert_eq!(remote_url(&repo), None);
        fs::remove_dir_all(&dir).ok();
    }
}
