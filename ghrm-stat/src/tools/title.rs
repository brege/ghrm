use crate::{Context, Row, repo};
use anyhow::Result;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let user = user_name(repo(ctx)).unwrap_or_default();
    Ok(vec![Row::new("user", user)])
}

// Reads the configured user name from the git config cascade, or None when
// it is unset.
fn user_name(repo: &gix::Repository) -> Option<String> {
    repo.config_snapshot()
        .string("user.name")
        .map(|name| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // A repository with user.name set only in local config, so the read is
    // independent of the machine's global git configuration.
    fn repo_with_user(name: &str, body: &str) -> (std::path::PathBuf, gix::Repository) {
        let dir = std::env::temp_dir().join(format!("ghrm-stat-title-{name}"));
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
    fn reads_the_local_user_name() {
        let (dir, repo) = repo_with_user("user", "[user]\n\tname = Ada Lovelace\n");
        assert_eq!(user_name(&repo), Some("Ada Lovelace".to_string()));
        fs::remove_dir_all(&dir).ok();
    }
}
