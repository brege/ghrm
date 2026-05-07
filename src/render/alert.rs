pub(super) fn rewrite_alerts(html: &str) -> String {
    let mut out = html.to_string();
    let kinds = ["note", "tip", "important", "warning", "caution"];
    for k in kinds {
        let needle = format!(r#"<div class="markdown-alert markdown-alert-{k}">"#);
        let replacement = format!(r#"<div class="markdown-admonition markdown-admonition-{k}">"#);
        out = out.replace(&needle, &replacement);
    }
    for k in kinds {
        let needle = format!(r#"<p class="markdown-alert-title">{}</p>"#, titlecase(k));
        let icon = octicon_for(k);
        let svg = if icon.is_empty() {
            String::new()
        } else {
            format!(
                "<svg class=\"octicon\" width=\"16\" height=\"16\" aria-hidden=\"true\"><use href=\"#ghrm-icon-{icon}\"></use></svg>"
            )
        };
        let replacement = format!(
            r#"<p class="markdown-admonition-title">{svg}{title}</p>"#,
            title = titlecase(k),
        );
        out = out.replace(&needle, &replacement);
    }
    out
}

fn titlecase(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn octicon_for(kind: &str) -> &'static str {
    match kind {
        "note" => "note",
        "tip" => "tip",
        "important" => "important",
        "warning" => "warning",
        "caution" => "caution",
        _ => "",
    }
}
