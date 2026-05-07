pub(super) fn rewrite_heading_anchors(html: &str) -> String {
    let mut out = String::with_capacity(html.len() + 512);
    let mut rest = html;
    loop {
        let next = (1u8..=6)
            .filter_map(|n| rest.find(&format!("<h{n}")).map(|i| (i, n)))
            .min_by_key(|(i, _)| *i);
        let Some((idx, level)) = next else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..idx]);
        let at = &rest[idx..];
        let Some(open_end) = at.find('>') else {
            out.push_str(at);
            break;
        };
        let close_tag = format!("</h{level}>");
        let Some(close_pos) = at.find(&close_tag) else {
            out.push_str(at);
            break;
        };
        let mut open_tag = at[..=open_end].to_string();
        let (anchor_id, inner) = split_heading_anchor(&at[open_end + 1..close_pos]);
        let id = attr_value(&open_tag, "id").or(anchor_id);
        if let Some(id_value) = id.as_ref()
            && !open_tag.contains(" id=\"")
        {
            open_tag.insert_str(open_tag.len() - 1, &format!(r#" id="{id_value}""#));
        }
        out.push_str(&open_tag);
        out.push_str(inner);
        if let Some(id) = id {
            out.push_str("<a class=\"ghrm-anchor\" aria-hidden=\"true\" tabindex=\"-1\" href=\"#");
            out.push_str(&id);
            out.push_str("\">#</a>");
        }
        out.push_str(&close_tag);
        rest = &at[close_pos + close_tag.len()..];
    }
    out
}

fn attr_value(tag: &str, name: &str) -> Option<String> {
    let needle = format!(r#" {name}=""#);
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_string())
}

fn split_heading_anchor(inner: &str) -> (Option<String>, &str) {
    if !inner.starts_with("<a ") {
        return (None, inner);
    }
    let Some(close) = inner.find("</a>") else {
        return (None, inner);
    };
    let anchor = &inner[..close + 4];
    if !anchor.contains(r#"class="anchor""#) {
        return (None, inner);
    }
    let id = attr_value(anchor, "id").or_else(|| {
        attr_value(anchor, "href").and_then(|href| href.strip_prefix('#').map(str::to_string))
    });
    (id, &inner[close + 4..])
}

pub(super) fn extract_title(html: &str) -> Option<String> {
    let open = html.find("<h1")?;
    let gt = html[open..].find('>')? + open + 1;
    let close = html[gt..].find("</h1>")? + gt;
    let inner = &html[gt..close];
    Some(strip_tags(inner).trim().to_string())
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    html_escape::decode_html_entities(&out).into_owned()
}
