use super::root;
use super::{Forge, SourceState};

use std::fs;
use std::path::Path;

pub(super) fn source_for_repo(root: &Path) -> SourceState {
    let Some(config_path) = root::git_config_path(&root.join(".git")) else {
        return SourceState::NoRepo;
    };
    let remotes = parse_remotes(&config_path);
    let selected = remotes
        .iter()
        .find(|(name, _)| name == "origin")
        .or_else(|| remotes.first());

    match selected {
        Some((_, raw)) => classify_remote(raw),
        None => SourceState::NoRemote,
    }
}

fn parse_remotes(config_path: &Path) -> Vec<(String, String)> {
    let text = match fs::read_to_string(config_path) {
        Ok(text) => text,
        Err(_) => return Vec::new(),
    };

    let mut remotes = Vec::new();
    let mut current = None::<String>;
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current = parse_remote_section(line);
            continue;
        }
        let Some(name) = current.as_ref() else {
            continue;
        };
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "url" {
            continue;
        }
        remotes.push((name.clone(), value.trim().to_string()));
    }
    remotes
}

fn parse_remote_section(section: &str) -> Option<String> {
    let body = section.strip_prefix('[')?.strip_suffix(']')?.trim();
    let rest = body.strip_prefix("remote ")?;
    let name = rest.strip_prefix('"')?.strip_suffix('"')?;
    Some(name.to_string())
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

    #[test]
    fn parse_remote_name_section() {
        assert_eq!(
            parse_remote_section(r#"[remote "origin"]"#),
            Some("origin".to_string())
        );
    }
}
