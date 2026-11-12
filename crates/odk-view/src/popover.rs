//! A panel of content opened from a trigger, on the Rune host
//! (`docs/spec/odk-overlays.md`, `Popover`).

use std::rc::Rc;

use crate::describe::apply;
use crate::host::Hosted;
use crate::ink::ink;
use crate::placement::{Align, Placement};
use crate::style::{self, focus_ring};
use crate::text::{Flow, words};
use crate::toggle::{Toggle, pressable};
use odk_a11y::{Accessible, Describe, Role};
use rune::{El, col, row};

/// The widest a popover grows, in points at the base text size. ODK's own.
const MAX_WIDTH: f32 = 360.0;

/// What the application hears: whether the user asked for the popover to be open.
type OpenChange<T> = Rc<dyn Fn(&mut T, bool)>;

/// Builds the panel's content, when the popover is open.
type Content<T> = Box<dyn FnOnce(&Hosted<T>) -> El>;

/// A panel of content opened from a trigger: a title and what the application puts in it. The
/// trigger is any content; it is wrapped in a focusable button that opens the panel on a press,
/// and on Enter or Space released on it, and says whether the panel is open.
///
/// The panel is placed beside the trigger, below it by default. It takes focus on open and gives
/// it back to the trigger on close. Escape closes it, and so does a press outside, which is the
/// overlay's: nothing beneath is pressed. In Rune a panel like this is modal, so Tab stays
/// inside it, where the GPUI one let Tab leave to the control after its trigger.
///
/// Uncontrolled by default, starting from `default_open` and keeping its state in the host's
/// store; pass `open` to control it and follow `on_open_change`, which hears each request once.
pub struct Popover<T> {
    id: String,
    name: String,
    title: Option<String>,
    content: Option<Content<T>>,
    placement: Placement,
    align: Align,
    open: Option<bool>,
    default_open: bool,
    on_open_change: Option<OpenChange<T>>,
    close_on_outside_click: bool,
}

impl<T: 'static> Popover<T> {
    /// A popover named `name`, for assistive technology to say of the trigger and the panel,
    /// such as "Filters".
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            title: None,
            content: None,
            placement: Placement::Bottom,
            align: Align::Start,
            open: None,
            default_open: false,
            on_open_change: None,
            close_on_outside_click: true,
        }
    }

    /// A heading at the top of the panel.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Builds the panel's content; it runs only while the popover is shown.
    pub fn content(mut self, build: impl FnOnce(&Hosted<T>) -> El + 'static) -> Self {
        self.content = Some(Box::new(build));
        self
    }

    /// The side of the trigger it prefers; below by default.
    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// Which edge of the trigger it lines up with; the start by default.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Controls it: shown while `open`. The application follows `on_open_change`.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Whether an uncontrolled popover starts open; closed by default.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Called with the state the user asks for, once per request: on a press on the trigger, on
    /// Enter or Space, on Escape and on a press outside.
    pub fn on_open_change(mut self, handler: impl Fn(&mut T, bool) + 'static) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }

    /// Whether a press outside closes it; true by default. Off, it stays until the trigger,
    /// Escape or the application closes it.
    pub fn close_on_outside_click(mut self, close: bool) -> Self {
        self.close_on_outside_click = close;
        self
    }

    /// `trigger` with its popover: the open panel is added to `layers`.
    pub fn wrap(self, hosted: &Hosted<T>, trigger: El, layers: &mut Vec<El>) -> El {
        let accessible = self.accessible();
        let paint = style::paint(hosted);
        let s = paint.tokens.popover();
        let g = s.geometry;
        let open = self
            .open
            .unwrap_or_else(|| hosted.ui.is_open(&self.id, self.default_open));
        let gap = paint.tokens.foundation().spacing.s1;
        let ring = paint.tokens.trigger_focus();

        // Asking for the state the panel is not in: the store follows when it is uncontrolled, and
        // the application hears it.
        let (id, controlled, default) = (self.id.clone(), self.open, self.default_open);
        let change = self.on_open_change.clone();
        let ask = move |hosted: &mut Hosted<T>, want: Option<bool>| {
            let current = controlled.unwrap_or_else(|| hosted.ui.is_open(&id, default));
            let want = want.unwrap_or(!current);
            if want == current {
                return;
            }
            if controlled.is_none() {
                hosted.ui.set_open(&id, want);
            }
            if let Some(change) = &change {
                change(&mut hosted.app, want);
            }
        };
        let ask = Rc::new(ask);

        let toggle: Toggle<T> = {
            let ask = ask.clone();
            Rc::new(move |hosted: &mut Hosted<T>| ask(hosted, None))
        };
        let trigger_accessible = Accessible::new(Role::Button, self.name.clone());
        let wrapper = focus_ring(
            apply(row().key(self.id.clone()), &trigger_accessible)
                .expanded(open)
                .focusable(true)
                .no_focus_ring()
                .self_start()
                .child(trigger),
            &ring,
        );
        let wrapper = pressable(wrapper, &self.id, true, toggle);

        if open {
            let mut panel = apply(col().key(format!("{}-panel", self.id)), &accessible)
                .px(g.padding_x)
                .py(g.padding_y)
                .gap(g.gap)
                .max_w(paint.px(MAX_WIDTH))
                .fill(ink(s.surface.background))
                .outline(g.border_width, 0.0, ink(s.surface.border))
                .radius(g.radius)
                // A panel with nothing to focus is itself a stop, so focus has somewhere to go.
                .focusable(true);
            if let Some(title) = self.title {
                panel = panel.child(words(
                    &paint,
                    title,
                    g.title,
                    s.surface.foreground,
                    Flow::End,
                ));
            }
            if let Some(build) = self.content {
                panel = panel.child(build(hosted));
            }
            let close = move |hosted: &mut Hosted<T>| ask(hosted, Some(false));
            layers.push(
                paint
                    .present(panel)
                    .modal(self.close_on_outside_click, close)
                    .focus_return(self.id.clone())
                    .anchored(
                        self.id.clone(),
                        self.placement.side(),
                        self.align.rune(),
                        gap,
                    ),
            );
        }
        wrapper
    }
}

impl<T> Describe for Popover<T> {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Dialog, self.name.clone())
    }
}
