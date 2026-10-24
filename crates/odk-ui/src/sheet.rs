use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce,
    SharedString, Styled, Window, div, px,
};
use odk_a11y::{Accessible, Describe, Role};

use odk_theme::foundation::MotionDuration;

use crate::components;
use crate::focus::{child_id, part_name};
use crate::layout::{StyledExt, v_flex};
use crate::overlay::{Dismiss, Transition, docked, layer_over, modal, modal_header};

type Handler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;
type Content = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement + 'static>;

/// The window edge a sheet slides in from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side {
    Left,
    #[default]
    Right,
    Bottom,
}

/// The width of a side sheet, or the height of a bottom one, unless the application sets it.
/// ODK's own length (ADR 0035), in logical pixels.
const DEFAULT_EXTENT: f32 = 400.0;

/// A modal panel that slides in from an edge over a scrim. While open, Tab stays inside it;
/// Escape, the close button or a click on the scrim asks the application to close it through
/// `on_close`. Controlled: the application owns `open`. It enters over the foundation's
/// `enter` and leaves over its `exit`.
///
/// The panel's content comes from a builder, [`Sheet::content`]. While the sheet closes it
/// keeps drawing the title, side, size and builder it last had open, so an application may
/// clear its own state as soon as it asks the sheet to close.
#[derive(IntoElement)]
pub struct Sheet {
    id: ElementId,
    open: bool,
    side: Side,
    title: SharedString,
    extent: Option<Pixels>,
    on_close: Option<Handler>,
    content: Option<Content>,
    modal: bool,
    close_on_escape: bool,
    close_on_backdrop: bool,
}

impl Sheet {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            open: false,
            side: Side::default(),
            title: title.into(),
            extent: None,
            on_close: None,
            content: None,
            modal: true,
            close_on_escape: true,
            close_on_backdrop: true,
        }
    }

    /// Whether it blocks the window behind it; true by default. A modal sheet dims the window,
    /// takes focus, keeps Tab inside and takes the pointer and keys. With `modal(false)` it is a
    /// side panel: no scrim, no focus trap and no focus taken on open, the rest of the window
    /// stays live, and Escape closes it while focus is inside it.
    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }

    /// Whether Escape asks to close; true by default.
    pub fn close_on_escape(mut self, close: bool) -> Self {
        self.close_on_escape = close;
        self
    }

    /// Whether a click on the scrim of a modal sheet asks to close; true by default.
    pub fn close_on_backdrop(mut self, close: bool) -> Self {
        self.close_on_backdrop = close;
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    /// The panel's width (left, right) or height (bottom).
    pub fn extent(mut self, extent: Pixels) -> Self {
        self.extent = Some(extent);
        self
    }

    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }

    /// Builds the panel's content below its title, each frame while the sheet is shown.
    pub fn content<E: IntoElement>(
        mut self,
        build: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.content = Some(Rc::new(move |w, cx| build(w, cx).into_any_element()));
        self
    }
}

impl Describe for Sheet {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Dialog, self.title.to_string())
    }
}

/// What the sheet draws: what it is given while open, remembered in `shown`, and the
/// remembered one while it closes.
fn remembered<T: Clone>(open: bool, wanted: T, shown: &mut T) -> T {
    if open {
        *shown = wanted;
    }
    shown.clone()
}

/// Everything a sheet needs to draw its exit.
#[derive(Clone)]
struct Shown {
    side: Side,
    extent: Pixels,
    title: SharedString,
    content: Option<Content>,
}

/// How far the panel is pushed off screen, given how open it is.
fn slide(extent: f32, amount: f32) -> f32 {
    -extent * (1.0 - amount)
}

impl RenderOnce for Sheet {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = components();
        let f = c.foundation().clone();
        let frame = layer_over(
            child_id(&self.id, "layer"),
            self.open,
            self.modal,
            Transition::of(MotionDuration::Enter, MotionDuration::Exit),
            window,
            cx,
        );
        let Some(amount) = frame.amount else {
            return div().into_any_element();
        };
        let amount = f.motion.easing.apply(amount);
        // Keep what the sheet opened with until its exit has run: an application often clears
        // what chose the side, the title or the content as soon as it asks the sheet to close.
        let wanted = Shown {
            side: self.side,
            extent: self.extent.unwrap_or(px(DEFAULT_EXTENT)),
            title: self.title,
            content: self.content,
        };
        let first = wanted.clone();
        let kept = window.use_keyed_state(child_id(&self.id, "shown"), cx, |_, _| first);
        let Shown {
            side,
            extent,
            title,
            content,
        } = kept.update(cx, |k, _| remembered(self.open, wanted, k));
        let close: Handler = self.on_close.unwrap_or_else(|| Rc::new(|_, _| {}));
        let viewport = window.viewport_size();
        // Never larger than the window: in a small window the sheet fills it.
        let extent = extent.min(match side {
            Side::Left | Side::Right => viewport.width,
            Side::Bottom => viewport.height,
        });
        let offset = px(slide(f32::from(extent), amount));

        let style = c.sheet();
        let g = style.geometry;
        let panel = v_flex()
            .id(child_id(&self.id, "panel"))
            .debug_selector(|| part_name(&self.id, "panel"))
            .absolute()
            .gap(px(g.gap))
            .px(px(g.padding_x))
            .py(px(g.padding_y))
            .treatment(&style.surface)
            .child(modal_header(&self.id, title, true, close.clone()))
            .children(content.map(|build| build(window, cx)));
        let width = px(g.border_width);
        let panel = match side {
            Side::Left => panel
                .top_0()
                .h_full()
                .w(extent)
                .left(offset)
                .border_r(width),
            Side::Right => panel
                .top_0()
                .h_full()
                .w(extent)
                .right(offset)
                .border_l(width),
            Side::Bottom => panel
                .left_0()
                .w_full()
                .h(extent)
                .bottom(offset)
                .border_t(width),
        };
        let mut dismiss = Dismiss::new(close);
        dismiss.escape = self.close_on_escape;
        dismiss.backdrop = self.close_on_backdrop;
        if self.modal {
            modal(
                &self.id,
                amount,
                &dismiss,
                &frame.content,
                panel,
                window,
                cx,
            )
        } else {
            docked(&self.id, &dismiss, &frame.content, panel, window, cx)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closes_on_the_side_it_opened_on() {
        let mut shown = (Side::Right, px(400.0));
        assert_eq!(
            remembered(true, (Side::Left, px(300.0)), &mut shown),
            (Side::Left, px(300.0))
        );
        // The application has reset its choice while the sheet closes.
        assert_eq!(
            remembered(false, (Side::Right, px(400.0)), &mut shown),
            (Side::Left, px(300.0))
        );
        assert_eq!(
            remembered(true, (Side::Bottom, px(400.0)), &mut shown),
            (Side::Bottom, px(400.0))
        );
    }

    #[test]
    fn slides_in_from_off_screen() {
        assert_eq!(slide(400.0, 0.0), -400.0);
        assert_eq!(slide(400.0, 0.5), -200.0);
        assert_eq!(slide(400.0, 1.0), 0.0);
    }
}
