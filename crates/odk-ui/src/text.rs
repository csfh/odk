//! Body text and headings, on component tokens: each is a type role in a text colour
//! (`docs/spec/odk-component-tokens.md`).

use gpui::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, Rgba, SharedString,
    Styled, Window, canvas, div, px,
};

use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::ProseStyle;
use odk_theme::foundation::TypeRole;

use crate::components;
use crate::decoration::{self, Applied, FlowKind, TextDecoration};
use crate::layout::{StyledExt, middle_ellipsis, text_width};
use crate::theme::colour;

/// Body text in the primary text colour.
#[derive(IntoElement)]
pub struct Text {
    content: SharedString,
    muted: bool,
    role: Option<TypeRole>,
    applied: Applied,
}

impl Text {
    pub fn new(content: impl Into<SharedString>) -> Self {
        Self {
            content: content.into(),
            muted: false,
            role: None,
            applied: Applied::default(),
        }
    }

    /// Secondary text, for what says less than the text around it.
    pub fn muted(mut self) -> Self {
        self.muted = true;
        self
    }

    /// Draws in `role` instead of body. The colour stays the text colour.
    pub fn role(mut self, role: TypeRole) -> Self {
        self.role = Some(role);
        self
    }

    /// Attaches a text decoration. Body text wraps; [`crate::EndEllipsis`] keeps one line and
    /// [`crate::MiddleEllipsis`] shortens the middle. A truncated string shows in full in a
    /// tooltip on hover. Text is not a tab stop, and [`Describe`] keeps the full string.
    pub fn decorate(mut self, decoration: impl TextDecoration) -> Self {
        decoration.apply_text(&mut self.applied);
        self
    }
}

impl Describe for Text {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Text, self.content.to_string())
    }
}

impl RenderOnce for Text {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let style = components().text(self.muted);
        let role = self.role.unwrap_or(style.role);
        prose(
            style,
            role,
            self.content,
            self.applied.flow(FlowKind::Wrap),
            window,
            cx,
        )
    }
}

/// A heading. Level 1 is the largest.
#[derive(IntoElement)]
pub struct Heading {
    content: SharedString,
    level: u8,
    applied: Applied,
}

impl Heading {
    pub fn new(content: impl Into<SharedString>) -> Self {
        Self {
            content: content.into(),
            level: 1,
            applied: Applied::default(),
        }
    }

    /// 1 is a title, 2 a heading, and 3 and higher a subtitle.
    pub fn level(mut self, level: u8) -> Self {
        self.level = level;
        self
    }

    /// Attaches a text decoration. A heading wraps. A dialog title passes [`crate::Clamp`] to
    /// cap it, and [`crate::Wrap`] lifts that cap (ADR 0058).
    pub fn decorate(mut self, decoration: impl TextDecoration) -> Self {
        decoration.apply_text(&mut self.applied);
        self
    }
}

impl Describe for Heading {
    /// Reports the level it was given, between 1 and 6, even where levels 3 and below draw
    /// alike, so the outline a screen reader sees keeps the application's structure.
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Heading, self.content.to_string()).level(self.level.clamp(1, 6))
    }
}

impl RenderOnce for Heading {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let style = components().heading(self.level);
        prose(
            style,
            style.role,
            self.content,
            self.applied.flow(FlowKind::Wrap),
            window,
            cx,
        )
    }
}

/// `content` in its style's colour and `role`, flowing as `flow`.
fn prose(
    style: ProseStyle,
    role: TypeRole,
    content: SharedString,
    flow: FlowKind,
    window: &mut Window,
    cx: &mut App,
) -> gpui::AnyElement {
    #[cfg(test)]
    drawn::record(&content, style);
    let id = ElementId::Name(SharedString::from(format!("odk-text-{content}")));
    let type_style = components().foundation().typography.style(role);
    let size = px(type_style.size);
    if flow == FlowKind::Middle {
        return middle_line(
            &id,
            content,
            size,
            px(type_style.line_height),
            colour(style.colour),
            window,
            cx,
        );
    }
    // Wrap never overflows, so it takes no probe and no tooltip. A probe on every line of a
    // page would ask for a frame the first time it measured (checklist item 35).
    if flow == FlowKind::Wrap {
        return div()
            .debug_selector(|| "odk-prose".into())
            .text_color(colour(style.colour))
            .text_role(role)
            .min_w_0()
            .child(content)
            .into_any_element();
    }
    let lines = decoration::flow_lines(flow).unwrap_or(1);
    let (was, probe) = decoration::truncation_probe(&id, content.clone(), size, lines, window, cx);
    let block = decoration::apply_flow(
        div()
            .debug_selector(|| "odk-prose".into())
            .text_color(colour(style.colour))
            .text_role(role)
            .min_w_0(),
        flow,
    )
    .child(content.clone())
    .child(probe);
    decoration::overflow_tooltip(&id, content, was && flow != FlowKind::Wrap, true, block)
}

/// One line of `text` shortened in the middle to the box it is given, painted from that box.
fn middle_line(
    id: &ElementId,
    text: SharedString,
    size: gpui::Pixels,
    line_height: gpui::Pixels,
    ink: Rgba,
    window: &mut Window,
    cx: &mut App,
) -> gpui::AnyElement {
    let full = text.clone();
    let measured = text.clone();
    let flag = window.use_keyed_state(crate::focus::child_id(id, "truncated"), cx, |_, _| false);
    let stored = *flag.read(cx);
    let flag = flag.clone();
    let body = canvas(
        move |bounds, window, cx| {
            let width = bounds.size.width;
            let shown = middle_ellipsis(&measured, |sample| {
                text_width(window, &SharedString::from(sample.to_owned()), size) <= width
            });
            let over = shown != measured.as_str();
            let changed = flag.update(cx, |current, cx| {
                if *current == over {
                    false
                } else {
                    *current = over;
                    cx.notify();
                    true
                }
            });
            if changed {
                window.refresh();
            }
            let mut run = window.text_style().to_run(shown.len());
            run.color = ink.into();
            window
                .text_system()
                .shape_line(SharedString::from(shown), size, &[run], None)
        },
        move |bounds, shaped, window, cx| {
            shaped.paint(bounds.origin, line_height, window, cx).ok();
        },
    )
    .h(line_height)
    .w_full();
    decoration::overflow_tooltip(id, full, stored, true, body)
}

/// What each text drew last, by its content, for the checklist's tests.
#[cfg(test)]
pub(crate) mod drawn {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use odk_theme::component::ProseStyle;

    thread_local! {
        static DRAWN: RefCell<HashMap<String, ProseStyle>> = RefCell::new(HashMap::new());
    }

    pub(crate) fn record(content: &str, style: ProseStyle) {
        DRAWN.with(|d| d.borrow_mut().insert(content.to_owned(), style));
    }

    pub(crate) fn last(content: &str) -> ProseStyle {
        DRAWN.with(|d| {
            *d.borrow()
                .get(content)
                .unwrap_or_else(|| panic!("no text {content:?} has been drawn"))
        })
    }
}
