use lol_html::html_content::ContentType;
use lol_html::{RewriteStrSettings, element, rewrite_str, text};

pub(super) fn rewrite_alerts(html: &str) -> String {
    rewrite_str(
        html,
        RewriteStrSettings {
            element_content_handlers: vec![
                element!("div.markdown-alert", |el| {
                    if let Some(kind) = el
                        .get_attribute("class")
                        .as_deref()
                        .and_then(alert_kind_from_class)
                    {
                        el.set_attribute(
                            "class",
                            &format!("markdown-admonition markdown-admonition-{kind}"),
                        )?;
                    }
                    Ok(())
                }),
                element!("p.markdown-alert-title", |el| {
                    el.set_attribute("class", "markdown-admonition-title")?;
                    Ok(())
                }),
                text!("p.markdown-alert-title", |chunk| {
                    if let Some(icon) = octicon_for_title(chunk.as_str()) {
                        chunk.before(&octicon_svg(icon), ContentType::Html);
                    }
                    Ok(())
                }),
            ],
            strict: false,
            ..RewriteStrSettings::new()
        },
    )
    .expect("rendered markdown alert rewriting should produce valid HTML")
}

fn alert_kind_from_class(classes: &str) -> Option<&str> {
    classes
        .split_ascii_whitespace()
        .filter_map(|class| class.strip_prefix("markdown-alert-"))
        .find(|kind| matches!(*kind, "note" | "tip" | "important" | "warning" | "caution"))
}

fn octicon_for_title(title: &str) -> Option<&'static str> {
    match title.trim().to_ascii_lowercase().as_str() {
        "note" => Some("note"),
        "tip" => Some("tip"),
        "important" => Some("important"),
        "warning" => Some("warning"),
        "caution" => Some("caution"),
        _ => None,
    }
}

fn octicon_svg(icon: &str) -> String {
    format!(
        "<svg class=\"octicon\" width=\"16\" height=\"16\" aria-hidden=\"true\"><use href=\"/_ghrm/assets/js/icons.svg#ghrm-icon-{icon}\"></use></svg>"
    )
}
