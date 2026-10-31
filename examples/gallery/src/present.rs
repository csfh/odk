//! What every page is made of: the frame that gives it a title, a description and its notes, and
//! the small pieces that show tokens, colours and code. They are the gallery's own, built on
//! ODK's components and the theme's tokens; none of them is for applications.

use odk::gpui::{ClipboardItem, Div, SharedString, rgba};
use odk::prelude::*;
use odk::theme::Color;

use crate::layout::{CONTENT_MAX_WIDTH, LABEL_WIDTH, SCROLL_HEIGHT, SWATCH};
use crate::registry::Page;

thread_local! {
    static PAGE_RENDERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times a page has been rendered on this thread, for the tests that check a page is
/// kept and not drawn again when only the header changes. Per thread because the tests run side
/// by side and each has a thread of its own.
pub fn page_renders() -> usize {
    PAGE_RENDERS.with(|n| n.get())
}

/// What every page is wrapped in: it fills the window beside the sidebar and scrolls inside it,
/// so its size is known without drawing it and GPUI can keep what it drew. The frame gives it
/// its title and description above, and below it what to know about the keyboard and assistive
/// technology and a usage example, when the page has them.
pub(crate) fn page(page: Page, body: impl IntoElement) -> impl IntoElement {
    PAGE_RENDERS.with(|n| n.set(n.get() + 1));
    let meta = page.meta();
    let mut column = Stack::new()
        .w_full()
        .max_w(odk::gpui::px(CONTENT_MAX_WIDTH))
        .gap(Space::Lg)
        .child(
            Stack::new()
                .gap(Space::Xs)
                .child(Heading::new(meta.title))
                .child(Text::new(meta.description).muted()),
        )
        .child(body);
    if let Some(code) = crate::snippets::usage(page) {
        column = column
            .child(Heading::new("Usage").level(2))
            .child(Text::new("Rust, as an application would write it. It is compiled with the gallery, so it builds against the current API.").muted())
            .child(code_panel(page.id(), code));
    }
    if !meta.accessibility.is_empty() {
        column = column
            .child(Heading::new("Keyboard and accessibility").level(2))
            .child(Text::new(meta.accessibility));
    }
    div()
        .id(SharedString::from(format!("{}-page", page.id())))
        .debug_selector(|| format!("{}-page", page.id()))
        .size_full()
        .overflow_y_scroll()
        .p(Space::Lg)
        .child(column)
}

pub(crate) fn section(title: &'static str) -> impl IntoElement {
    Heading::new(title).level(2)
}

pub(crate) fn group(title: &'static str) -> impl IntoElement {
    Heading::new(title).level(3)
}

/// The theme's border colour for the gallery's own edges.
pub(crate) fn edge() -> odk::gpui::Rgba {
    rgba(
        odk::ui::theme_snapshot()
            .semantic
            .border
            .subtle
            .to_rgba_u32(),
    )
}

/// A colour as a small square, edged like a field so a colour close to the surface still shows.
pub(crate) fn swatch(c: Color) -> Div {
    swatch_sized(c, SWATCH, SWATCH)
}

pub(crate) fn swatch_sized(c: Color, width: f32, height: f32) -> Div {
    let strong = odk::ui::theme_snapshot().semantic.border.strong;
    div()
        .w(odk::gpui::px(width))
        .h(odk::gpui::px(height))
        .flex_shrink_0()
        .border_1()
        .border_color(rgba(strong.to_rgba_u32()))
        .bg(rgba(c.to_rgba_u32()))
}

/// One labelled line of a table of values.
pub(crate) fn kv(label: impl Into<SharedString>, value: impl Into<SharedString>) -> Row {
    Row::new()
        .items_start()
        .gap(Space::Md)
        .child(
            div()
                .w(odk::gpui::px(LABEL_WIDTH))
                .flex_shrink_0()
                .child(Text::new(label).muted()),
        )
        .child(div().min_w_0().child(Text::new(value)))
}

/// A labelled line whose value is a colour: its swatch, its hex and where it came from.
pub(crate) fn colour_kv(
    label: impl Into<SharedString>,
    c: Color,
    note: impl Into<SharedString>,
) -> Row {
    Row::new()
        .gap(Space::Md)
        .child(
            div()
                .w(odk::gpui::px(LABEL_WIDTH))
                .flex_shrink_0()
                .child(Text::new(label).muted()),
        )
        .child(swatch(c))
        .child(Text::new(format!("{} {}", c.to_hex(), note.into())))
}

/// A wrapping row of things.
pub(crate) fn wrap() -> Row {
    Row::new().flex_wrap().gap(Space::Sm)
}

/// A block of code in the monospace type role, with a button that copies it. GPUI's text cannot
/// be selected, so the button is how code is taken from here.
pub(crate) fn code_panel(id: &'static str, code: &'static str) -> impl IntoElement {
    code_panel_owned(id, SharedString::from(code))
}

/// [`code_panel`] for code made at run time.
pub(crate) fn code_panel_owned(id: &'static str, code: SharedString) -> impl IntoElement {
    let snapshot = odk::ui::theme_snapshot();
    let surface = snapshot.semantic.surface.recessed;
    let edge = snapshot.semantic.border.subtle;
    let text = code;
    let copy = text.clone();
    Stack::new()
        .gap(Space::Sm)
        .p(Space::Md)
        .bg(rgba(surface.to_rgba_u32()))
        .border_1()
        .border_color(rgba(edge.to_rgba_u32()))
        .rounded(odk::gpui::px(odk::ui::foundation().geometry.radius.default))
        .child(
            Row::new().justify_end().child(
                Button::new(SharedString::from(format!("copy-{id}")))
                    .label("Copy")
                    .variant(ButtonVariant::Outline)
                    .density(Density::Compact)
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(copy.to_string()))
                    }),
            ),
        )
        .child(
            div()
                .id(SharedString::from(format!("code-{id}")))
                .w_full()
                .min_w_0()
                .max_h(odk::gpui::px(SCROLL_HEIGHT))
                .overflow_y_scroll()
                .child(code_lines(text)),
        )
}

/// One `Text` per line. GPUI refuses a text run that contains a newline, and a scroll on one
/// axis clips the other, so a long line wraps instead of scrolling sideways (ADR 0058).
fn code_lines(text: SharedString) -> impl IntoElement {
    let mut lines = Stack::new().w_full().min_w_0();
    for line in text.lines() {
        let line = if line.is_empty() {
            "\u{00a0}".to_owned()
        } else {
            line.to_owned()
        };
        lines = lines.child(Text::new(line).role(odk::theme::foundation::TypeRole::Mono));
    }
    lines
}

/// A handler for a control that reports a `bool` by value (a check box, a switch, a popover),
/// which runs `f` on the view `cx` belongs to; `cx.listener` cannot, since it passes a reference.
pub(crate) fn flag<V: 'static>(
    cx: &Context<V>,
    f: impl Fn(&mut V, bool, &mut Context<V>) + 'static,
) -> impl Fn(bool, &mut Window, &mut App) + 'static {
    let entity = cx.entity();
    move |on, _, cx| entity.update(cx, |this, cx| f(this, on, cx))
}
