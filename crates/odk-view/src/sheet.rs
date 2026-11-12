//! A panel attached to a window edge, on the Rune host (`docs/spec/odk-overlays.md`, `Sheet`):
//! modal, it dims the window behind, keeps Tab inside and blocks the page; not modal, it is a side
//! panel that leaves the page live and takes no focus when it opens.

use std::rc::Rc;

use crate::describe::{apply, pointer_target};
use crate::dialog::{OnClose, modal_header};
use crate::host::Hosted;
use crate::ink::ink;
use crate::style;
use odk_a11y::{Accessible, Describe, Role};
use rune::{El, Key, KeyEvent, ScrollSpec, Tx, col, el, row};

/// The window edge a sheet is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side {
    Left,
    #[default]
    Right,
    Bottom,
}

/// The width of a side sheet, or the height of a bottom one, unless the application sets it. ODK's
/// own length (ADR 0035), in points at the base text size.
const DEFAULT_EXTENT: f32 = 400.0;

/// A panel attached to a window edge. Modal (the default), it dims the window behind it, keeps Tab
/// inside and blocks the page; Escape, the close button or a press on the scrim asks the
/// application to close it through `on_close`. With `modal(false)` it is a side panel: no scrim,
/// the page stays live, it takes no focus on opening, and Escape closes it while focus is inside
/// it. Controlled: the application owns `open`. It has a border on the side that faces the window
/// and no corner of its own, and is never larger than the window.
///
/// Rune cannot remember what had focus before a modal opened, so the application names the
/// element to give focus back to with [`Sheet::returns_focus_to`].
pub struct Sheet<T> {
    id: String,
    title: String,
    open: bool,
    side: Side,
    extent: Option<f32>,
    modal: bool,
    content: Vec<El>,
    on_close: Option<OnClose<T>>,
    close_on_escape: bool,
    close_on_backdrop: bool,
    returns_focus_to: Option<String>,
}

impl<T: 'static> Sheet<T> {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            open: false,
            side: Side::Right,
            extent: None,
            modal: true,
            content: Vec::new(),
            on_close: None,
            close_on_escape: true,
            close_on_backdrop: true,
            returns_focus_to: None,
        }
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    /// The width of a side sheet, or the height of a bottom one, in points at the base text size.
    pub fn extent(mut self, extent: f32) -> Self {
        self.extent = Some(extent);
        self
    }

    /// True (the default) dims, traps and blocks; false is a side panel.
    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }

    /// Adds to the body, which scrolls when it is too tall.
    pub fn content(mut self, element: El) -> Self {
        self.content.push(element);
        self
    }

    /// Hears every request to close.
    pub fn on_close(mut self, handler: impl Fn(&mut T) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }

    /// Whether Escape asks to close; true by default.
    pub fn close_on_escape(mut self, close: bool) -> Self {
        self.close_on_escape = close;
        self
    }

    /// Whether a press on the scrim of a modal sheet asks to close; true by default.
    pub fn close_on_backdrop(mut self, close: bool) -> Self {
        self.close_on_backdrop = close;
        self
    }

    /// The key of the element that gets focus back when a modal sheet closes.
    pub fn returns_focus_to(mut self, key: impl Into<String>) -> Self {
        self.returns_focus_to = Some(key.into());
        self
    }

    /// Draws the sheet into `layers` when it is open.
    pub fn show(self, hosted: &Hosted<T>, layers: &mut Vec<El>) {
        if !self.open {
            return;
        }
        let accessible = self.accessible();
        let paint = style::paint(hosted);
        let s = paint.tokens.sheet();
        let g = s.geometry;
        let side = self.side;
        let extent = paint.px(self.extent.unwrap_or(DEFAULT_EXTENT));
        let close_app: OnClose<T> = self
            .on_close
            .clone()
            .unwrap_or_else(|| Rc::new(|_: &mut T| {}));
        let escape = self.close_on_escape;
        let ask = {
            let close_app = close_app.clone();
            Rc::new(move |hosted: &mut Hosted<T>| close_app(&mut hosted.app))
        };

        let header = modal_header(
            hosted,
            &paint,
            &self.id,
            &self.title,
            (g, s.surface.foreground),
            true,
            &close_app,
        );
        let mut body = col().gap(g.gap).min_h(0.0);
        for element in self.content {
            body = body.child(element);
        }
        // Not `grow`: a viewport that grows from nothing is 0 pt tall (see `Dialog`).
        let body = body
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .shrink()
            .min_h(0.0);

        let edge = ink(s.surface.border);
        // The border is on the side that faces the window: a line along that edge of the panel.
        let line = |element: El| element.fill(edge);
        let content = col()
            .gap(g.gap)
            .px(g.padding_x)
            .py(g.padding_y)
            .grow(1.0)
            .min_w(0.0)
            .min_h(0.0)
            .child(header)
            .child(body);
        let mut panel = apply(
            match side {
                Side::Left => row()
                    .child(content)
                    .child(line(el().w(g.border_width).h_full())),
                Side::Right => row()
                    .child(line(el().w(g.border_width).h_full()))
                    .child(content),
                Side::Bottom => col()
                    .child(line(el().h(g.border_width).w_full()))
                    .child(content),
            }
            .key(self.id.clone()),
            &accessible,
        )
        .fill(ink(s.surface.background))
        .shrink()
        .min_w(0.0)
        .min_h(0.0);
        panel = match side {
            Side::Left | Side::Right => panel.w(extent).h_full(),
            Side::Bottom => panel.h(extent).w_full(),
        };

        if !self.modal {
            // A side panel: out of the page's flow, against its edge, and it leaves the page live.
            let ask = ask.clone();
            let panel = panel
                .pin(
                    match side {
                        Side::Right => None,
                        _ => Some(0.0),
                    },
                    match side {
                        Side::Bottom => None,
                        _ => Some(0.0),
                    },
                    match side {
                        Side::Left => None,
                        _ => Some(0.0),
                    },
                    Some(0.0),
                )
                .on_key(move |hosted: &mut Hosted<T>, ev: &KeyEvent, _: &mut Tx| {
                    // Escape closes it while focus is inside it.
                    if escape && ev.key == Key::Escape {
                        ask(hosted);
                        return true;
                    }
                    false
                });
            layers.push(panel);
            return;
        }

        // A press on the panel is the panel's: it is not the scrim's.
        let panel = panel.on_press(|_: &mut Hosted<T>| {});
        let close = {
            let ask = ask.clone();
            move |hosted: &mut Hosted<T>| {
                if escape {
                    ask(hosted);
                }
            }
        };
        let mut scrim = match side {
            Side::Left => row().justify_start(),
            Side::Right => row().justify_end(),
            Side::Bottom => col().justify_end(),
        }
        .key(format!("{}-scrim", self.id))
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .child(panel)
        // The scrim is the modal element, so what lies behind is inert and the scrim's own press
        // still reaches it (see `Dialog`).
        .modal(false, close);
        if let Some(key) = self.returns_focus_to {
            scrim = scrim.focus_return(key);
        }
        if let Some(backdrop) = s.backdrop {
            scrim = scrim.fill(ink(backdrop));
        }
        if self.close_on_backdrop {
            scrim = scrim.on_press(move |hosted: &mut Hosted<T>| ask(hosted));
        }
        // A press outside dismisses it: a target for the pointer, not a control with a name.
        layers.push(pointer_target(scrim));
    }
}

impl<T> Describe for Sheet<T> {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Dialog, self.title.clone())
    }
}
