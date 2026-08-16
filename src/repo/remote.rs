use super::{Forge, SourceState};

use gix::bstr::ByteSlice;
use std::path::Path;

pub(super) fn source_for_repo(root: &Path) -> SourceState {
    let Ok(repo) = gix::open(root) else {
        return SourceState::NoRepo;
    };
    match remote_url(&repo) {
        Some(raw) => classify_remote(&raw),
        None => SourceState::NoRemote,
    }
}

// Selects origin, then the first remote with a configured url, skipping
// remotes without one. gix sorts remote_names, so the fallback tie-break
// is name order rather than config-file order. The value is gix's
// effective config value; classify_remote receives it without passing
// through gix's URL parser, so it sees the configured string rather than
// a re-serialized URL.
fn remote_url(repo: &gix::Repository) -> Option<String> {
    let config = repo.config_snapshot();
    let read = |name: &str| {
        config
            .string(format!("remote.{name}.url").as_str())
            .map(|url| url.to_string())
    };
    if let Some(url) = read("origin") {
        return Some(url);
    }
    for name in repo.remote_names().iter() {
        if let Ok(name) = name.to_str()
            && let Some(url) = read(name)
        {
            return Some(url);
        }
    }
    None
}

fn classify_remote(raw: &str) -> SourceState {
    if let Some((scheme, host, path)) = parse_url_remote(raw) {
        return match scheme {
            "http" | "https" => SourceState::Web {
                url: canonical_http_url(scheme, host, path),
                raw: raw.to_string(),
                forge: forge_for_host(host),
            },
            "ssh" => match ssh_web_url(host, raw, path) {
                Some(url) => SourceState::Web {
                    url,
                    raw: raw.to_string(),
                    forge: forge_for_host(host),
                },
                None => SourceState::Transport {
                    raw: raw.to_string(),
                },
            },
            _ => SourceState::Transport {
                raw: raw.to_string(),
            },
        };
    }

    if let Some((host, path)) = parse_scp_remote(raw)
        && let Some(url) = scp_web_url(host, path)
    {
        return SourceState::Web {
            url,
            raw: raw.to_string(),
            forge: forge_for_host(host),
        };
    }

    SourceState::Transport {
        raw: raw.to_string(),
    }
}

fn parse_url_remote(raw: &str) -> Option<(&str, &str, &str)> {
    let (scheme, rest) = raw.split_once("://")?;
    let slash = rest.find('/').unwrap_or(rest.len());
    let authority = &rest[..slash];
    let path = &rest[slash..];
    let hostport = authority.rsplit_once('@').map_or(authority, |(_, rhs)| rhs);
    let host = hostport.split(':').next().unwrap_or(hostport);
    Some((scheme, host, path))
}

fn parse_scp_remote(raw: &str) -> Option<(&str, &str)> {
    if raw.contains("://") {
        return None;
    }
    let (lhs, path) = raw.split_once(':')?;
    let host = lhs.rsplit_once('@').map_or(lhs, |(_, rhs)| rhs);
    Some((host, path))
}

fn canonical_http_url(scheme: &str, host: &str, path: &str) -> String {
    format!("{scheme}://{host}/{}", strip_git_suffix(path))
}

fn ssh_web_url(host: &str, raw: &str, path: &str) -> Option<String> {
    let rel = strip_git_suffix(path);
    if rel.is_empty() {
        return None;
    }
    if host == "git.sr.ht" {
        return sourcehut_url(host, &rel);
    }
    if standard_web_host(host) && path_depth(&rel) == 2 {
        return Some(format!("https://{host}/{rel}"));
    }
    if host == "gitlab.com" && path_depth(&rel) >= 2 {
        return Some(format!("https://{host}/{rel}"));
    }
    if looks_like_generic_forge_path(&rel) {
        return Some(format!("https://{host}/{rel}"));
    }
    if raw.contains("@gitlab.") && path_depth(&rel) >= 2 {
        return Some(format!("https://{host}/{rel}"));
    }
    None
}

fn scp_web_url(host: &str, path: &str) -> Option<String> {
    let rel = strip_git_suffix(path);
    if rel.is_empty() {
        return None;
    }
    if host == "git.sr.ht" {
        return sourcehut_url(host, &rel);
    }
    if standard_web_host(host) && path_depth(&rel) == 2 {
        return Some(format!("https://{host}/{rel}"));
    }
    if host == "gitlab.com" && path_depth(&rel) >= 2 {
        return Some(format!("https://{host}/{rel}"));
    }
    if looks_like_generic_forge_path(&rel) {
        return Some(format!("https://{host}/{rel}"));
    }
    None
}

fn sourcehut_url(host: &str, rel: &str) -> Option<String> {
    if path_depth(rel) == 2 && rel.split('/').next()?.starts_with('~') {
        return Some(format!("https://{host}/{rel}"));
    }
    None
}

fn standard_web_host(host: &str) -> bool {
    matches!(host, "github.com" | "codeberg.org" | "bitbucket.org")
}

fn looks_like_generic_forge_path(rel: &str) -> bool {
    if path_depth(rel) != 2 {
        return false;
    }
    let first = rel.split('/').next().unwrap_or_default();
    !matches!(
        first,
        "home" | "srv" | "var" | "opt" | "usr" | "mnt" | "tmp"
    )
}

fn forge_for_host(host: &str) -> Forge {
    match host {
        "github.com" => Forge::GitHub,
        "bitbucket.org" => Forge::Bitbucket,
        "gitlab.com" => Forge::GitLab,
        "codeberg.org" => Forge::Codeberg,
        "git.sr.ht" => Forge::SourceHut,
        _ => Forge::Generic,
    }
}

fn path_depth(rel: &str) -> usize {
    rel.split('/').filter(|part| !part.is_empty()).count()
}

fn strip_git_suffix(path: &str) -> String {
    path.trim_matches('/')
        .strip_suffix(".git")
        .unwrap_or(path.trim_matches('/'))
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::testutil::TempDir;
    use std::fs;

    // Builds a real repository through gix and appends the config body so
    // source_for_repo reads remotes back through gix, not a parser.
    fn repo_with_config(prefix: &str, body: &str) -> TempDir {
        let td = TempDir::new(prefix);
        gix::init(td.path()).expect("init repository");
        let config = td.path().join(".git/config");
        let mut text = fs::read_to_string(&config).expect("read repository config");
        text.push_str(body);
        fs::write(&config, text).unwrap();
        td
    }

    fn selected_raw(state: SourceState) -> Option<String> {
        match state {
            SourceState::Web { raw, .. } | SourceState::Transport { raw } => Some(raw),
            SourceState::NoRemote | SourceState::NoRepo => None,
        }
    }

    #[test]
    fn source_prefers_origin_over_other_remotes() {
        let td = repo_with_config(
            "ghrm-remote-origin",
            "[remote \"origin\"]\n\turl = https://example.com/a/origin.git\n\
             [remote \"upstream\"]\n\turl = https://example.com/a/upstream.git\n",
        );
        assert_eq!(
            selected_raw(source_for_repo(td.path())),
            Some("https://example.com/a/origin.git".to_string())
        );
    }

    #[test]
    fn source_falls_back_in_sorted_name_order() {
        let td = repo_with_config(
            "ghrm-remote-fallback",
            "[remote \"zeta\"]\n\turl = https://example.com/a/zeta.git\n\
             [remote \"alpha\"]\n\turl = https://example.com/a/alpha.git\n",
        );
        assert_eq!(
            selected_raw(source_for_repo(td.path())),
            Some("https://example.com/a/alpha.git".to_string())
        );
    }

    #[test]
    fn source_skips_remotes_without_a_url() {
        let td = repo_with_config(
            "ghrm-remote-skip",
            "[remote \"alpha\"]\n\tfetch = +refs/heads/*:refs/remotes/alpha/*\n\
             [remote \"zeta\"]\n\turl = https://example.com/a/zeta.git\n",
        );
        assert_eq!(
            selected_raw(source_for_repo(td.path())),
            Some("https://example.com/a/zeta.git".to_string())
        );
    }

    #[test]
    fn source_without_remotes_is_no_remote() {
        let td = repo_with_config("ghrm-remote-none", "");
        assert_eq!(source_for_repo(td.path()), SourceState::NoRemote);
    }

    #[test]
    fn source_for_non_repo_is_no_repo() {
        let td = TempDir::new("ghrm-remote-norepo");
        assert_eq!(source_for_repo(td.path()), SourceState::NoRepo);
    }

    #[test]
    fn source_reads_linked_worktree_remote() {
        let td = TempDir::new("ghrm-remote-worktree");
        let main = td.path().join("main");
        let wt = td.path().join("wt");
        gix::init(&main).expect("init repository");
        let config = main.join(".git/config");
        let mut text = fs::read_to_string(&config).expect("read repository config");
        text.push_str("[remote \"origin\"]\n\turl = https://example.com/a/origin.git\n");
        fs::write(&config, text).unwrap();

        let wt_gitdir = main.join(".git/worktrees/w1");
        fs::create_dir_all(&wt_gitdir).unwrap();
        fs::create_dir_all(&wt).unwrap();
        let head = fs::read_to_string(main.join(".git/HEAD")).unwrap();
        fs::write(
            wt.join(".git"),
            format!("gitdir: {}\n", wt_gitdir.display()),
        )
        .unwrap();
        fs::write(wt_gitdir.join("commondir"), "../..\n").unwrap();
        fs::write(
            wt_gitdir.join("gitdir"),
            format!("{}\n", wt.join(".git").display()),
        )
        .unwrap();
        fs::write(wt_gitdir.join("HEAD"), head).unwrap();

        assert_eq!(
            selected_raw(source_for_repo(&wt)),
            Some("https://example.com/a/origin.git".to_string())
        );
    }

    #[test]
    fn github_scp_maps_to_web() {
        assert_eq!(
            classify_remote("git@github.com:brege/oshea.git"),
            SourceState::Web {
                url: "https://github.com/brege/oshea".to_string(),
                raw: "git@github.com:brege/oshea.git".to_string(),
                forge: Forge::GitHub,
            }
        );
    }

    #[test]
    fn gitlab_subgroup_ssh_maps_to_web() {
        assert_eq!(
            classify_remote("git@gitlab.com:group/subgroup/repo.git"),
            SourceState::Web {
                url: "https://gitlab.com/group/subgroup/repo".to_string(),
                raw: "git@gitlab.com:group/subgroup/repo.git".to_string(),
                forge: Forge::GitLab,
            }
        );
    }

    #[test]
    fn https_clone_stays_web() {
        assert_eq!(
            classify_remote("https://example.com/org/project.git"),
            SourceState::Web {
                url: "https://example.com/org/project".to_string(),
                raw: "https://example.com/org/project.git".to_string(),
                forge: Forge::Generic,
            }
        );
    }

    #[test]
    fn local_ssh_path_stays_transport() {
        assert_eq!(
            classify_remote("ssh://host/home/user/git/code/project.git"),
            SourceState::Transport {
                raw: "ssh://host/home/user/git/code/project.git".to_string(),
            }
        );
    }

    #[test]
    fn sourcehut_keeps_tilde_owner() {
        assert_eq!(
            classify_remote("git@git.sr.ht:~sircmpwn/core.sr.ht"),
            SourceState::Web {
                url: "https://git.sr.ht/~sircmpwn/core.sr.ht".to_string(),
                raw: "git@git.sr.ht:~sircmpwn/core.sr.ht".to_string(),
                forge: Forge::SourceHut,
            }
        );
    }
}
