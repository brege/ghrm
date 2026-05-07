use lol_html::{RewriteStrSettings, element, rewrite_str};
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy)]
pub struct RenderPath<'a> {
    pub root: &'a Path,
    pub src: &'a Path,
}

pub(super) fn rewrite_local_urls(html: &str, path: RenderPath<'_>) -> String {
    rewrite_str(
        html,
        RewriteStrSettings {
            element_content_handlers: vec![
                element!("[href]", move |el| {
                    if let Some(value) = el.get_attribute("href") {
                        el.set_attribute("href", &local_url(path, &value))?;
                    }
                    Ok(())
                }),
                element!("[src]", move |el| {
                    if let Some(value) = el.get_attribute("src") {
                        el.set_attribute("src", &local_url(path, &value))?;
                    }
                    Ok(())
                }),
            ],
            strict: false,
            ..RewriteStrSettings::new()
        },
    )
    .expect("rendered markdown URL rewriting should produce valid HTML")
}

fn local_url(path: RenderPath<'_>, dest: &str) -> String {
    if is_external_url(dest) || dest.starts_with('#') {
        return dest.to_string();
    }

    let (target, suffix) = split_suffix(dest);
    if target.is_empty() || target.starts_with('/') {
        return dest.to_string();
    }

    let Some(rel) = resolve_target(path.root, path.src, target) else {
        return dest.to_string();
    };

    let mut out = String::from("/");
    out.push_str(&rel.to_string_lossy().replace('\\', "/"));
    out.push_str(suffix);
    out
}

fn is_external_url(dest: &str) -> bool {
    if dest.starts_with("//") {
        return true;
    }
    let end = dest.find(['/', '?', '#']).unwrap_or(dest.len());
    dest[..end].contains(':')
}

fn split_suffix(dest: &str) -> (&str, &str) {
    let idx = dest.find(['?', '#']).unwrap_or(dest.len());
    (&dest[..idx], &dest[idx..])
}

fn resolve_target(root: &Path, src: &Path, target: &str) -> Option<PathBuf> {
    let src_dir = src.parent()?;
    let mut rel = src_dir.strip_prefix(root).ok()?.to_path_buf();

    for comp in Path::new(target).components() {
        match comp {
            Component::CurDir => {}
            Component::Normal(part) => rel.push(part),
            Component::ParentDir => {
                if !rel.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    Some(rel)
}
