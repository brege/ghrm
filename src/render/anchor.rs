use lol_html::{RewriteStrSettings, element, end_tag, rewrite_str, text};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

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
    let title = Rc::new(RefCell::new(String::new()));
    let seen = Rc::new(Cell::new(false));
    let active = Rc::new(Cell::new(false));
    let open_active = Rc::clone(&active);
    let close_active = Rc::clone(&active);
    let seen_h1 = Rc::clone(&seen);
    let title_text = Rc::clone(&title);

    rewrite_str(
        html,
        RewriteStrSettings {
            element_content_handlers: vec![
                element!("h1", move |el| {
                    if !seen_h1.get() {
                        seen_h1.set(true);
                        open_active.set(true);
                        let close_active = Rc::clone(&close_active);
                        el.on_end_tag(end_tag!(move |_| {
                            close_active.set(false);
                            Ok(())
                        }))?;
                    }
                    Ok(())
                }),
                text!("h1", move |chunk| {
                    if active.get() {
                        title_text.borrow_mut().push_str(chunk.as_str());
                    }
                    Ok(())
                }),
            ],
            strict: false,
            ..RewriteStrSettings::new()
        },
    )
    .expect("rendered markdown title extraction should parse valid HTML");

    let title = title.borrow();
    let title = title.trim();
    (!title.is_empty()).then(|| html_escape::decode_html_entities(title).into_owned())
}
