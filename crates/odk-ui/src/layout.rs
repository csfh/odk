use gpui::{
    App, Bounds, DefiniteLength, Div, ElementId, InteractiveElement, IntoElement, Length,
    ParentElement, Pixels, SharedString, Stateful, StatefulInteractiveElement, Styled, Window,
    canvas, div, px,
};

use odk_theme::component::ControlGeometry;
use odk_theme::foundation::TypeRole;
use odk_theme::semantic::Treatment;

use crate::separator::Separator;
use crate::theme::{colour, family, foundation, theme_snapshot, weight};

/// A step on the spacing ladder. The pixel size comes from the foundation's spacing
/// (`docs/spec/odk-foundation-tokens.md`) and follows the user's text size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Space {
    Xs,
    Sm,
    Md,
    Lg,
    Xl,
}

impl Space {
    pub fn px(self) -> f32 {
        let space = &foundation().spacing;
        match self {
            Self::Xs => space.s1,
            Self::Sm => space.s2,
            Self::Md => space.s3,
            Self::Lg => space.s4,
            Self::Xl => space.s6,
        }
    }
}

/// A `Space` goes wherever GPUI takes a length, so `.gap(Space::Md)` and `.p(Space::Lg)` are
/// GPUI's own methods; the pixel size is read from the foundation when the element is built.
impl From<Space> for DefiniteLength {
    fn from(space: Space) -> Self {
        DefiniteLength::from(px(space.px()))
    }
}

/// Margins take a `Length`, which may also be `auto`.
impl From<Space> for Length {
    fn from(space: Space) -> Self {
        Length::Definite(space.into())
    }
}

/// Spacing and type helpers GPUI lacks.
pub trait StyledExt: Styled + Sized {
    /// Text in one of the foundation's type roles (`docs/spec/odk-foundation-tokens.md`): the
    /// role's size, line height and weight, and its family: the application's, or Omarchy's
    /// font (ADR 0043). The role's letter spacing is not applied, since GPUI has none.
    fn text_role(self, role: TypeRole) -> Self {
        let f = crate::foundation();
        let style = f.typography.style(role);
        self.text_size(px(style.size))
            .line_height(px(style.line_height))
            .font_weight(weight(style.weight))
            .font_family(family(&f, style.family))
    }

    /// A component token's colours (`docs/spec/odk-component-tokens.md`): its background, its
    /// foreground as the text colour, and its border's colour. The border's width is the
    /// geometry's ([`StyledExt::control_geometry`]). Inside `.hover()` and `.active()` it sets
    /// only the colour of the text, and GPUI 0.2.2 then replaces the element's whole text style
    /// with that, dropping its size and weight: there, set the background and border colours
    /// and keep the text style, as ODK's components do (`docs/checks.md`).
    fn treatment(self, t: &Treatment) -> Self {
        self.bg(colour(t.background))
            .text_color(colour(t.foreground))
            .border_color(colour(t.border))
    }

    /// A control's box from its component token: height, padding, corner radius, border width,
    /// the gap between its parts and its type role.
    fn control_geometry(self, g: &ControlGeometry) -> Self {
        self.h(px(g.height))
            .px(px(g.padding_x))
            .py(px(g.padding_y))
            .rounded(px(g.radius))
            .border(px(g.border_width))
            .gap(px(g.gap))
            .text_role(g.type_role)
    }

    /// Padding with one step across and another down.
    fn p_xy(mut self, x: Space, y: Space) -> Self {
        let (x, y) = (Some(DefiniteLength::from(x)), Some(DefiniteLength::from(y)));
        let padding = &mut self.style().padding;
        padding.left = x;
        padding.right = x;
        padding.top = y;
        padding.bottom = y;
        self
    }
}

impl<T: Styled + Sized> StyledExt for T {}

/// The room an overlay has in a window `viewport` wide (or high), kept `margin` from each
/// edge.
pub(crate) fn inside_window(viewport: Pixels, margin: Pixels) -> Pixels {
    viewport - margin - margin
}

/// How wide `text` is on one line at `size`, in the window's current font.
pub(crate) fn text_width(window: &Window, text: &SharedString, size: Pixels) -> Pixels {
    let run = window.text_style().to_run(text.len());
    window
        .text_system()
        .shape_line(text.clone(), size, &[run], None)
        .width
}

/// Records the bounds of the element it is placed in, after layout, for placing an overlay
/// against it. It covers the element's border box when `border` is the element's border width,
/// its padding box when it is zero: an absolute element is placed inside its parent's border,
/// and without offsets it would sit where it would have been in the flow.
pub(crate) fn bounds_probe(
    border: Pixels,
    record: impl FnOnce(Bounds<Pixels>, &mut App) + 'static,
) -> impl IntoElement {
    canvas(move |bounds, _, cx| record(bounds, cx), |_, _, _, _| {})
        .absolute()
        .top(-border)
        .left(-border)
        .right(-border)
        .bottom(-border)
}

/// Text that stays on one line and ends in `…` when it does not fit. Use this instead of
/// GPUI's `truncate`: in GPUI 0.2.2 a text element that may not wrap keeps the size it was
/// first measured at, so inside a shrinking flex item it is clipped instead of truncated.
/// Text that may wrap is measured again at its final width, so this lets it wrap, keeps one
/// line and truncates that (`docs/checks.md`).
pub(crate) fn single_line<E: Styled>(el: E) -> E {
    el.overflow_hidden().line_clamp(1).text_ellipsis()
}

/// The top of a window's view: fills the window, paints the theme's base surface and sets the
/// default text colour. Start every window's view with this. The window's toasts and overlay
/// host come from the window itself ([`crate::open_window`]).
pub fn root() -> Div {
    let s = theme_snapshot();
    v_flex()
        .size_full()
        .bg(colour(s.semantic.surface.canvas))
        .text_color(colour(s.semantic.text.primary))
}

/// A column of children.
pub fn v_flex() -> Div {
    div().flex().flex_col()
}

/// A row of children.
pub fn h_flex() -> Div {
    div().flex().flex_row().items_center()
}

/// A region that takes the room its parent leaves and scrolls when its content is taller. It is
/// a block, not a flex column: children of a flex column shrink to fit instead of scrolling, so
/// put one `v_flex()` or `content_column()` inside it. The `id` is GPUI's key for the scroll
/// position, so it must be unique in the window.
pub fn scroll_y(id: impl Into<ElementId>) -> Stateful<Div> {
    div().id(id).flex_1().min_h_0().overflow_y_scroll()
}

/// The frame of an application with a sidebar: the sidebar at the foundation's width, a line,
/// and the content taking the rest. It fills its parent, so put it where the page goes, inside
/// [`root`].
pub fn sidebar_frame(sidebar: impl IntoElement, content: impl IntoElement) -> Div {
    let width = foundation().layout.sidebar_width;
    // A row, not `h_flex`: that centres its children, and these two stretch to the full height.
    div()
        .flex()
        .flex_row()
        .size_full()
        .child(v_flex().w(px(width)).flex_none().h_full().child(sidebar))
        .child(Separator::vertical())
        .child(v_flex().flex_1().min_w_0().h_full().child(content))
}

/// A page's column of content: as wide as its parent up to the foundation's content width,
/// centred when the parent is wider, with the panel padding and the gap between sections.
pub fn content_column() -> Div {
    let layout = &foundation().layout;
    v_flex()
        .w_full()
        .max_w(px(layout.content_width))
        .mx_auto()
        .p(px(layout.panel_padding))
        .gap(px(layout.section_gap))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_goes_wherever_gpui_takes_a_length() {
        crate::theme::install_test_theme();
        assert_eq!(Space::Md.px(), 12.0);
        assert_eq!(
            DefiniteLength::from(Space::Lg),
            DefiniteLength::from(px(16.0))
        );
    }
}
