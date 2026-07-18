use comrak::Anchorizer;
use comrak::adapters::{HeadingAdapter, HeadingMeta};
use comrak::nodes::Sourcepos;
use lol_html::{RewriteStrSettings, element, end_tag, rewrite_str, text};
use std::cell::{Cell, RefCell};
use std::fmt::Write;
use std::rc::Rc;
use std::sync::Mutex;

// comrak heading plugin: emits the id and a trailing "#" anchor during render,
// so the id slug comes from comrak's own Anchorizer and no HTML post-parsing is
// needed. anchorize dedups repeated headings, so the id is stashed on enter and
// reused when the closing anchor is written on exit.
#[derive(Default)]
pub(super) struct HeadingAnchors {
    state: Mutex<AnchorState>,
}

#[derive(Default)]
struct AnchorState {
    anchorizer: Anchorizer,
    pending: Option<String>,
}

impl HeadingAdapter for HeadingAnchors {
    fn enter(
        &self,
        output: &mut dyn Write,
        heading: &HeadingMeta,
        _sourcepos: Option<Sourcepos>,
    ) -> std::fmt::Result {
        let mut state = self.state.lock().unwrap();
        let id = state.anchorizer.anchorize(&heading.content);
        write!(output, r#"<h{} id="{id}">"#, heading.level)?;
        state.pending = Some(id);
        Ok(())
    }

    fn exit(&self, output: &mut dyn Write, heading: &HeadingMeta) -> std::fmt::Result {
        let id = self
            .state
            .lock()
            .unwrap()
            .pending
            .take()
            .expect("heading exit should follow heading enter");
        write!(
            output,
            r##"<a class="ghrm-anchor" aria-hidden="true" tabindex="-1" href="#{id}">#</a></h{}>"##,
            heading.level
        )
    }
}

pub(super) fn extract_title(html: &str) -> Option<String> {
    let title = Rc::new(RefCell::new(String::new()));
    let seen = Rc::new(Cell::new(false));
    let active = Rc::new(Cell::new(false));
    let in_anchor = Rc::new(Cell::new(false));
    let open_active = Rc::clone(&active);
    let close_active = Rc::clone(&active);
    let open_anchor = Rc::clone(&in_anchor);
    let seen_h1 = Rc::clone(&seen);
    let title_text = Rc::clone(&title);

    rewrite_str(
        html,
        RewriteStrSettings::new()
            .with_strict(false)
            .append_element_content_handler(element!("h1", move |el| {
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
            }))
            .append_element_content_handler(element!("h1 a.ghrm-anchor", move |el| {
                open_anchor.set(true);
                let close_anchor = Rc::clone(&open_anchor);
                el.on_end_tag(end_tag!(move |_| {
                    close_anchor.set(false);
                    Ok(())
                }))?;
                Ok(())
            }))
            .append_element_content_handler(text!("h1", move |chunk| {
                if active.get() && !in_anchor.get() {
                    title_text.borrow_mut().push_str(chunk.as_str());
                }
                Ok(())
            })),
    )
    .expect("rendered markdown title extraction should parse valid HTML");

    let title = title.borrow();
    let title = title.trim();
    (!title.is_empty()).then(|| html_escape::decode_html_entities(title).into_owned())
}
