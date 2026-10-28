//! The Data page: components that lay out and show an application's data. A resizable group of
//! panels, in a row and in a column, whose handles move with the pointer or the arrow keys
//! (ADR 0069), and what it reports when a resize ends.

use odk::gpui::{Div, px};
use odk::prelude::*;

use crate::buttons::caption;
use crate::content::Content;
use crate::{Page, Settings};

/// How tall each resizable group is drawn here: a group fills its parent, so the page gives it
/// a height of its own.
const GROUP_HEIGHT: f32 = 220.0;

/// The Data page: its own state, and the Content switch it follows.
pub struct DataPage {
    worst_case: bool,
    /// What the row's `on_layout` last reported, as whole per cent.
    layout: Option<Vec<u32>>,
}

impl DataPage {
    pub fn new(settings: &Entity<Settings>, cx: &mut Context<Self>) -> Self {
        cx.observe(settings, |this, settings, cx| {
            this.worst_case = settings.read(cx).worst_case;
            cx.notify();
        })
        .detach();
        Self {
            worst_case: settings.read(cx).worst_case,
            layout: None,
        }
    }
}

impl Render for DataPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = Content::get(self.worst_case);
        crate::page(Page::Data, section(self, &c, cx))
    }
}

fn group(title: &'static str) -> impl IntoElement {
    Heading::new(title).level(3)
}

/// The section.
pub fn section(g: &mut DataPage, c: &Content, cx: &mut Context<DataPage>) -> Div {
    v_flex()
        .flex_shrink_0()
        .gap(Space::Lg)
        .child(group("Resizable"))
        .child(resizable(g, c, cx))
}

/// A panel's content: a title over a paragraph, which wraps and is cut at the panel's edge.
fn panel_text(title: &'static str, body: &'static str) -> impl IntoElement {
    v_flex()
        .p(Space::Md)
        .gap(Space::Sm)
        .child(Text::new(title).single_line())
        .child(Text::new(body).muted())
}

fn framed(group: impl IntoElement) -> Div {
    v_flex().h(px(GROUP_HEIGHT)).w_full().child(group)
}

fn resizable(g: &mut DataPage, c: &Content, cx: &mut Context<DataPage>) -> Div {
    let [list, detail, log] = c.panels;
    let reported = match &g.layout {
        Some(sizes) => sizes
            .iter()
            .map(|s| format!("{s}%"))
            .collect::<Vec<_>>()
            .join(" · "),
        None => "nothing yet".into(),
    };
    v_flex()
        .gap(Space::Sm)
        .child(caption(
            "A row of three panels. Drag a handle, or Tab to it and use Left, Right, Home and End; the list keeps between 20% and 60%, the detail at least 30%.",
        ))
        .child(framed(
            Resizable::horizontal("data-row", "List, detail and log")
                .panel(
                    ResizablePanel::new()
                        .default_size(0.3)
                        .min_size(0.2)
                        .max_size(0.6)
                        .child(panel_text(list.0, list.1)),
                )
                .panel(
                    ResizablePanel::new()
                        .min_size(0.3)
                        .child(panel_text(detail.0, detail.1)),
                )
                .panel(
                    ResizablePanel::new()
                        .default_size(0.25)
                        .child(panel_text(log.0, log.1)),
                )
                .on_layout(cx.listener(|this, sizes: &[f32], _, cx| {
                    this.layout = Some(sizes.iter().map(|s| (s * 100.0).round() as u32).collect());
                    cx.notify();
                })),
        ))
        .child(caption(format!("Reported when a resize ends: {reported}")))
        .child(caption(
            "A column: Up and Down move the handle. Below it, a disabled group, whose handle neither moves nor takes focus.",
        ))
        .child(framed(
            Resizable::vertical("data-column", "Editor and terminal")
                .panel(
                    ResizablePanel::new()
                        .default_size(0.6)
                        .min_size(0.25)
                        .child(panel_text(detail.0, detail.1)),
                )
                .panel(
                    ResizablePanel::new()
                        .min_size(0.2)
                        .child(panel_text(log.0, log.1)),
                ),
        ))
        .child(framed(
            Resizable::horizontal("data-disabled", "Fixed split")
                .disabled(true)
                .panel(ResizablePanel::new().child(panel_text(list.0, list.1)))
                .panel(ResizablePanel::new().child(panel_text(detail.0, detail.1))),
        ))
}
