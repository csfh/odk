//! A modal panel centred over a scrim, on the Rune host (`docs/spec/odk-overlays.md`, `Dialog`):
//! a title and a close button, an optional description, a body that scrolls, and actions laid out
//! in a row aligned to the end that wraps, so no action is ever pushed out of reach.
//!
//! While it is open Tab stays inside it and the window behind it takes no pointer or key. The
//! application owns `open` and hears `on_close` when Escape, the close button or the scrim asks.

use std::rc::Rc;

use crate::button::Button;
use crate::describe::apply;
use crate::host::Hosted;
use crate::icon::Icon;
use crate::ink::ink;
use crate::style::{self};
use crate::text::{Flow, words};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::ButtonVariant;
use rune::{El, ScrollSpec, col, row};

/// The widest a dialog is, in logical pixels. ODK's own (ADR 0035).
const DIALOG_WIDTH: f32 = 480.0;

pub(crate) type OnClose<T> = Rc<dyn Fn(&mut T)>;

/// A modal panel centred over a scrim. Escape, the close button or a press on the scrim asks the
/// application to close it through `on_close`. [`Dialog::close_on_escape`],
/// [`Dialog::close_on_backdrop`] and [`Dialog::close_button`] take those away one by one, as a
/// confirmation of something destructive should. Turning all three off is refused: a dialog with
/// no way out is a keyboard trap, so Escape stays. A body too tall scrolls while the header and
/// the actions stay, and the dialog is never taller than its window. Controlled: the application
/// owns `open`.
///
/// Rune cannot remember what had focus before a modal opened, so the application names the
/// element to give focus back to with [`Dialog::returns_focus_to`].
pub struct Dialog<T> {
    id: String,
    title: String,
    description: Option<String>,
    open: bool,
    content: Vec<El>,
    actions: Vec<El>,
    on_close: Option<OnClose<T>>,
    close_on_escape: bool,
    close_on_backdrop: bool,
    close_button: bool,
    returns_focus_to: Option<String>,
}

impl<T: 'static> Dialog<T> {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            open: false,
            content: Vec::new(),
            actions: Vec::new(),
            on_close: None,
            close_on_escape: true,
            close_on_backdrop: true,
            close_button: true,
            returns_focus_to: None,
        }
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// A line or two under the title that says what the dialog is about.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Adds to the body, which scrolls when it is too tall.
    pub fn content(mut self, element: El) -> Self {
        self.content.push(element);
        self
    }

    /// Adds an action, which is the application's own `Button`, so a destructive one is a
    /// `ButtonVariant::Destructive`.
    pub fn action(mut self, element: El) -> Self {
        self.actions.push(element);
        self
    }

    /// Hears every request to close.
    pub fn on_close(mut self, handler: impl Fn(&mut T) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }

    pub fn close_on_escape(mut self, close: bool) -> Self {
        self.close_on_escape = close;
        self
    }

    pub fn close_on_backdrop(mut self, close: bool) -> Self {
        self.close_on_backdrop = close;
        self
    }

    pub fn close_button(mut self, show: bool) -> Self {
        self.close_button = show;
        self
    }

    /// The key of the element that gets focus back when the dialog closes, such as the button that
    /// opened it.
    pub fn returns_focus_to(mut self, key: impl Into<String>) -> Self {
        self.returns_focus_to = Some(key.into());
        self
    }

    /// Whether Escape closes it: as asked, and always when the other two ways out are off.
    fn escape_closes(&self) -> bool {
        self.close_on_escape || !(self.close_button || self.close_on_backdrop)
    }

    /// Draws the dialog into `layers` when it is open.
    pub fn show(self, hosted: &Hosted<T>, layers: &mut Vec<El>) {
        if !self.open {
            return;
        }
        let accessible = self.accessible();
        let paint = style::paint(hosted.theme());
        let s = paint.tokens.dialog();
        let g = s.geometry;
        let escape = self.escape_closes();
        // What a request to close does: the application hears it, and decides.
        let close_app: OnClose<T> = self
            .on_close
            .clone()
            .unwrap_or_else(|| Rc::new(|_: &mut T| {}));
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
            self.close_button,
            &close_app,
        );

        let mut body = col().gap(g.gap).min_h(0.0);
        if let Some(description) = &self.description {
            body = body.child(words(
                &paint,
                description.clone(),
                g.body,
                s.secondary,
                Flow::Wrap,
            ));
        }
        for element in self.content {
            body = body.child(element);
        }
        let body = body
            // No `grow`: a viewport that grows from nothing is 0 pt tall in a panel that has no
            // height of its own. It is as tall as its content and shrinks when the window is
            // shorter.
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .shrink()
            .min_h(0.0);

        let mut actions = row().flex_wrap().justify_end().gap(g.gap);
        for action in self.actions {
            actions = actions.child(action);
        }

        let key = self.id.clone();
        let panel = apply(col().key(key), &accessible)
            .px(g.padding_x)
            .py(g.padding_y)
            .gap(g.gap)
            .w_full()
            .max_w(paint.px(DIALOG_WIDTH))
            // Never taller than its window: it shrinks, and its body scrolls.
            .shrink()
            .min_h(0.0)
            .fill(ink(s.surface.background))
            .outline(g.border_width, 0.0, ink(s.surface.border))
            .radius(g.radius)
            // A panel with nothing to focus is itself a stop, so focus has somewhere to go.
            .focusable(true)
            // A press on the panel is the panel's: it is not the scrim's.
            .on_press(|_: &mut Hosted<T>| {})
            .child(header)
            .child(body)
            .child(actions);
        let close = {
            let ask = ask.clone();
            move |hosted: &mut Hosted<T>| {
                if escape {
                    ask(hosted);
                }
            }
        };

        let mut scrim = col()
            .key(format!("{}-scrim", self.id))
            // Over the whole window, out of the page's flow.
            .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .center()
            .padding(g.padding_x)
            .child(panel)
            // The scrim is the modal element, so the panel and the scrim's own press are both
            // inside it and what lies behind is inert. Escape asks to close, if the policy lets it.
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
        layers.push(scrim);
    }
}

impl<T> Describe for Dialog<T> {
    fn accessible(&self) -> Accessible {
        let accessible = Accessible::new(Role::Dialog, self.title.clone());
        match &self.description {
            Some(description) => accessible.description(description.clone()),
            None => accessible,
        }
    }
}

/// The header of a modal panel: its title, which wraps and takes the room, and a ghost icon
/// button that closes it. A dialog and a sheet share it.
pub(crate) fn modal_header<T: 'static>(
    hosted: &Hosted<T>,
    paint: &crate::style::Paint,
    id: &str,
    title: &str,
    (g, colour): (odk_theme::component::SurfaceGeometry, odk_theme::Color),
    close_button: bool,
    close_app: &OnClose<T>,
) -> El {
    let mut header = row().items_start().gap(g.gap).child(
        words(paint, title.to_owned(), g.title, colour, Flow::Wrap)
            .grow(1.0)
            .shrink(),
    );
    if close_button {
        let close_app = close_app.clone();
        header = header.child(
            Button::icon(
                format!("{id}-close"),
                Icon::Close,
                "Close",
                move |app: &mut T| close_app(app),
            )
            .variant(ButtonVariant::Ghost)
            .show(hosted),
        );
    }
    header
}
