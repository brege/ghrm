use crate::runtime;
use crate::tmpl::{self, AboutPeek};

use tracing::warn;

const PROJECT_URL: &str = "https://github.com/brege/ghrm";

pub(crate) fn html(runtime_paths: &runtime::Paths, oob: bool) -> String {
    let project_version = env!("CARGO_PKG_VERSION");
    let project_release_href = format!("{PROJECT_URL}/releases/tag/v{project_version}");
    let about = AboutPeek {
        oob,
        runtime_paths: runtime_paths.rows(),
        project_href: PROJECT_URL,
        project_release_href: &project_release_href,
        project_version,
    };
    match tmpl::about(about) {
        Ok(html) => html,
        Err(e) => {
            warn!("about template error: {}", e);
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn test_runtime_paths() -> runtime::Paths {
        let td = TempDir::new("ghrm-about-runtime-paths");
        runtime::Paths::new(td.path(), None).unwrap()
    }

    #[test]
    fn about_html_renders_runtime_and_app_links() {
        let runtime_paths = test_runtime_paths();
        let html = html(&runtime_paths, false);

        assert!(html.contains("Runtime Paths"));
        assert!(html.contains("href=\"https://github.com/brege/ghrm\""));
        assert!(html.contains(">brege/ghrm</span>"));
    }

    #[test]
    fn about_html_omits_current_source() {
        let runtime_paths = test_runtime_paths();
        let html = html(&runtime_paths, false);

        assert!(!html.contains("Current Source"));
    }

    #[test]
    fn about_oob_includes_swap_attribute() {
        let runtime_paths = test_runtime_paths();
        let html = html(&runtime_paths, true);

        assert!(html.contains("id=\"ghrm-about-peek\""));
        assert!(html.contains("hx-swap-oob=\"true\""));
    }
}
