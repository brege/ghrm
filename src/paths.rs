use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use std::path::{Component, Path, PathBuf};

// Encode characters that the WHATWG URL parser treats specially inside one
// path segment while retaining slash separators between filesystem entries.
const URL_SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'/')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'\\')
    .add(b'}');

pub fn safe_rel(raw: &str) -> Option<PathBuf> {
    let clean = raw.trim_matches('/');
    if clean.is_empty() {
        return None;
    }
    let path = Path::new(clean);
    if path
        .components()
        .all(|comp| matches!(comp, Component::Normal(_)))
    {
        Some(path.to_path_buf())
    } else {
        None
    }
}

pub(crate) fn url_path(raw: &str) -> String {
    let encoded = raw
        .trim_matches('/')
        .split('/')
        .map(|segment| utf8_percent_encode(segment, URL_SEGMENT).to_string())
        .collect::<Vec<_>>()
        .join("/");
    format!("/{encoded}")
}

pub fn allowed_name(name: &str, exclude_names: &[String]) -> bool {
    !exclude_names.iter().any(|entry| entry == name)
}

pub fn has_excluded_part(path: &Path, exclude_names: &[String]) -> bool {
    path.iter().any(|part| {
        let name = part.to_string_lossy();
        !allowed_name(name.as_ref(), exclude_names)
    })
}

pub fn has_hidden_part(path: &Path) -> bool {
    path.iter()
        .any(|part| part.to_string_lossy().starts_with('.'))
}

pub fn resolve_file(base: &Path, raw: &str) -> Option<PathBuf> {
    let rel = safe_rel(raw)?;
    let path = base.join(rel);
    if path.is_file() { Some(path) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_rel_rejects_parent_components() {
        assert!(safe_rel("../secret.txt").is_none());
        assert!(safe_rel("docs/../secret.txt").is_none());
        assert!(safe_rel("/absolute").is_some());
    }

    #[test]
    fn url_path_encodes_reserved_characters_by_segment() {
        assert_eq!(url_path("docs/notes #1?.md"), "/docs/notes%20%231%3F.md");
        assert_eq!(url_path("docs/100%.md"), "/docs/100%25.md");
        assert_eq!(url_path("/docs/readme.md/"), "/docs/readme.md");
    }

    #[test]
    fn excluded_part_matches_nested_paths() {
        let excludes = ["target".to_string(), "node_modules".to_string()];
        assert!(has_excluded_part(Path::new("target/debug/app"), &excludes));
        assert!(has_excluded_part(
            Path::new("src/node_modules/pkg/index.js"),
            &excludes
        ));
        assert!(!has_excluded_part(Path::new("src/main.rs"), &excludes));
    }

    #[test]
    fn hidden_part_matches_nested_paths() {
        assert!(has_hidden_part(Path::new(".git/config")));
        assert!(has_hidden_part(Path::new("docs/.draft/readme.md")));
        assert!(!has_hidden_part(Path::new("docs/readme.md")));
    }

    #[test]
    fn git_is_excluded_only_when_configured() {
        assert!(!has_excluded_part(Path::new(".git/config"), &[]));
        assert!(has_excluded_part(
            Path::new(".git/config"),
            &[".git".to_string()]
        ));
    }
}
