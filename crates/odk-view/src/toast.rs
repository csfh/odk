//! Toasts: short messages raised from anywhere, on the Rune host (`docs/spec/odk-overlays.md`,
//! `Toast`). They stack in the bottom-right corner above the page, each stays for the foundation's
//! `toast` (twice as long when it has an action), until it is dismissed, or for as long as the
//! application says, and a persistent one stays until dismissed. The clocks stop while the
//! pointer is over the stack. A toast never takes focus when it appears, and the stack is a tab
//! stop: Escape dismisses the newest toast. At most three show at a time; the rest wait their
//! turn.
//!
//! An application's handlers see only its own state, so it keeps a [`Toaster`] in that state, raises
//! toasts with it from any handler, and draws it in its view with [`Toaster::show`]. The window's
//! host runs the clocks (checklist item 35: nothing is asked of Rune while no toast is ticking).

use std::cell::{Cell, RefCell};
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::time::Duration;

use crate::button::Button;
use crate::describe::apply;
use crate::host::Hosted;
use crate::icon::Icon;
use crate::ink::ink;
use crate::style;
use crate::text::{Flow, words};
use crate::tone::Tone;
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::ButtonVariant;
use odk_theme::semantic::Role as Semantic;
use rune::{El, Key, KeyEvent, Tx, col, row};

/// The most toasts shown at once unless [`Toaster::set_limit`] says otherwise. A count.
pub const MAX_VISIBLE: usize = 3;
/// How long a toast stays until the first draw, which reads the foundation's `motion.toast` (also
/// 5 s) into the stack. A toast raised before any draw has a clock only once it shows, so this
/// stands in and is never what a toast waits for.
const BEFORE_THE_FIRST_DRAW: Duration = Duration::from_secs(5);

/// The width of a toast, in points at the base text size. ODK's own (ADR 0035).
const TOAST_WIDTH: f32 = 336.0;
/// How many times as long as an ordinary toast one with an action stays: time enough to reach the
/// action and press it. ODK's own.
const ACTION_TIME: u32 = 2;

/// What names a toast: every [`Toast`] has one from the moment it is made, distinct from every
/// other made on the thread and stable while it lives. An application that wants to name its own,
/// to update or dismiss a toast it does not hold, makes one with [`ToastId::named`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToastId(u64);

thread_local! {
    static NEXT: Cell<u64> = const { Cell::new(1) };
}

impl ToastId {
    fn fresh() -> Self {
        NEXT.with(|next| {
            let id = next.get();
            next.set(id + 1);
            Self(id)
        })
    }

    /// An id the application names: the same name is the same id, and never a fresh one's.
    pub fn named(name: &str) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        name.hash(&mut hasher);
        Self(hasher.finish() | (1 << 63))
    }
}

#[derive(Clone, Copy)]
enum Lifetime {
    Default,
    For(Duration),
    Persistent,
}

type Action<T> = Rc<dyn Fn(&mut T)>;

/// A short message. `T` is the application's state, which an action's handler sees.
pub struct Toast<T> {
    tone: Option<Tone>,
    id: ToastId,
    title: String,
    description: Option<String>,
    action: Option<(String, Action<T>)>,
    lifetime: Lifetime,
}

impl<T> Toast<T> {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            tone: Some(Tone::default()),
            id: ToastId::fresh(),
            title: title.into(),
            description: None,
            action: None,
            lifetime: Lifetime::Default,
        }
    }

    pub fn variant(mut self, tone: Tone) -> Self {
        self.tone = Some(tone);
        self
    }

    /// No status colour and no mark.
    pub fn neutral(mut self) -> Self {
        self.tone = None;
        self
    }

    /// Raising a toast whose id is on screen or waiting updates it in place and starts its clock
    /// again.
    pub fn with_id(mut self, id: ToastId) -> Self {
        self.id = id;
        self
    }

    pub fn id(&self) -> ToastId {
        self.id
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// A button that runs `handler` and dismisses the toast.
    pub fn action(mut self, label: impl Into<String>, handler: impl Fn(&mut T) + 'static) -> Self {
        self.action = Some((label.into(), Rc::new(handler)));
        self
    }

    /// Stays until it is dismissed.
    pub fn persistent(mut self) -> Self {
        self.lifetime = Lifetime::Persistent;
        self
    }

    /// Stays this long, whatever the foundation says.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.lifetime = Lifetime::For(timeout);
        self
    }

    fn life(&self, base: Duration) -> Option<Duration> {
        match self.lifetime {
            Lifetime::Persistent => None,
            Lifetime::For(own) => Some(own),
            Lifetime::Default if self.action.is_some() => Some(base * ACTION_TIME),
            Lifetime::Default => Some(base),
        }
    }
}

/// A toast is a status named by its title, and by its description when it has one.
impl<T> Describe for Toast<T> {
    fn accessible(&self) -> Accessible {
        let name = match &self.description {
            Some(description) => format!("{}: {description}", self.title),
            None => self.title.clone(),
        };
        Accessible::new(Role::Status, name)
    }
}

struct Entry<T> {
    toast: Toast<T>,
    /// Time left, once it is on screen: `None` while it waits or when it is persistent.
    remaining: Option<Duration>,
    shown: bool,
}

struct State<T> {
    entries: Vec<Entry<T>>,
    limit: usize,
    paused: bool,
    /// The foundation's `toast`, as the last `show` read it.
    base: Duration,
    /// The host's clock at the last step.
    last: Option<f64>,
}

/// What the host's poll steps, with `T` erased.
pub(crate) trait Clock {
    /// Steps the clocks to `now`; says whether a toast went, and asks for the next wake.
    fn advance(&self, now: f64, tx: &mut Tx) -> bool;
}

/// The toasts of a window, kept in the application's state. Cheap to clone: a clone is the same
/// stack.
pub struct Toaster<T> {
    state: Rc<RefCell<State<T>>>,
}

impl<T> Clone for Toaster<T> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

impl<T> Default for Toaster<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Toaster<T> {
    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(State {
                entries: Vec::new(),
                limit: MAX_VISIBLE,
                paused: false,
                base: BEFORE_THE_FIRST_DRAW,
                last: None,
            })),
        }
    }

    /// Raises a toast: it shows when there is room for it, and an id already here is updated in
    /// place.
    pub fn raise(&self, toast: Toast<T>) {
        let mut state = self.state.borrow_mut();
        if let Some(entry) = state.entries.iter_mut().find(|e| e.toast.id == toast.id) {
            entry.toast = toast;
            // Its clock starts again, from the moment it next shows.
            entry.remaining = None;
            entry.shown = false;
        } else {
            state.entries.push(Entry {
                toast,
                remaining: None,
                shown: false,
            });
        }
    }

    pub fn dismiss(&self, id: ToastId) {
        self.state.borrow_mut().entries.retain(|e| e.toast.id != id);
    }

    /// How many toasts show at once; at least one.
    pub fn set_limit(&self, limit: usize) {
        self.state.borrow_mut().limit = limit.max(1);
    }

    /// How many toasts there are, on screen and waiting.
    pub fn len(&self) -> usize {
        self.state.borrow().entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn set_paused(&self, paused: bool) {
        self.state.borrow_mut().paused = paused;
    }

    /// Puts a clock on every toast that has just come on screen.
    fn start_visible(state: &mut State<T>) -> bool {
        let (base, limit) = (state.base, state.limit);
        let mut started = false;
        for entry in state.entries.iter_mut().take(limit) {
            if !entry.shown {
                entry.shown = true;
                entry.remaining = entry.toast.life(base);
                started = true;
            }
        }
        started
    }
}

impl<T: 'static> Clock for Toaster<T> {
    fn advance(&self, now: f64, tx: &mut Tx) -> bool {
        let mut state = self.state.borrow_mut();
        let elapsed = state.last.map_or(Duration::ZERO, |last| {
            Duration::from_secs_f64((now - last).max(0.0))
        });
        state.last = Some(now);
        let mut changed = Self::start_visible(&mut state);
        if !state.paused {
            let limit = state.limit;
            for entry in state.entries.iter_mut().take(limit) {
                if let Some(remaining) = &mut entry.remaining {
                    *remaining = remaining.saturating_sub(elapsed);
                }
            }
        }
        let before = state.entries.len();
        state
            .entries
            .retain(|e| !(e.shown && e.remaining.is_some_and(|r| r.is_zero())));
        changed |= state.entries.len() != before;
        // Whoever came on screen as a toast left starts its clock now.
        changed |= Self::start_visible(&mut state);
        if !state.paused {
            let next = state
                .entries
                .iter()
                .take(state.limit)
                .filter_map(|e| e.remaining)
                .min();
            if let Some(next) = next {
                tx.wake_after(next);
            }
        }
        changed
    }
}

impl<T: 'static> Toaster<T> {
    /// Draws the toasts that show into `layers`, last, so they lie over the page. A window with no
    /// toast draws nothing and asks for nothing.
    pub fn show(&self, hosted: &Hosted<T>, layers: &mut Vec<El>) {
        let paint = style::paint(hosted);
        {
            let mut state = self.state.borrow_mut();
            state.base = paint.tokens.foundation().motion.toast;
            // A toast that is on screen has its clock; the host's poll runs it.
            Self::start_visible(&mut state);
        }
        // The host steps this stack's clocks, now and while it lives.
        hosted.ui.attach_clock(Rc::new(self.clone()));
        let state = self.state.borrow();
        if state.entries.is_empty() {
            return;
        }
        let margin = paint.tokens.foundation().spacing.s4;
        let width = paint.px(TOAST_WIDTH);
        let mut stack = col()
            .key("odk-toasts")
            .pin(None, None, Some(margin), Some(margin))
            .w(width)
            .gap(paint.tokens.foundation().spacing.s2)
            .focusable(true);
        for (index, entry) in state.entries.iter().take(state.limit).enumerate() {
            stack = stack.child(self.card(hosted, &paint, &entry.toast, index));
        }
        let (hover, keys) = (self.clone(), self.clone());
        let newest = state
            .entries
            .iter()
            .take(state.limit)
            .next_back()
            .map(|e| e.toast.id);
        drop(state);
        layers.push(
            paint
                .present(stack)
                .on_hover_change(move |_: &mut Hosted<T>, over: bool, tx: &mut Tx| {
                    hover.set_paused(over);
                    // The clocks run again from the next frame.
                    tx.wake_after(Duration::ZERO);
                })
                .on_key(move |_: &mut Hosted<T>, ev: &KeyEvent, _: &mut Tx| {
                    // Escape dismisses the newest toast, with or without an action button.
                    if ev.key == Key::Escape
                        && let Some(id) = newest
                    {
                        keys.dismiss(id);
                        return true;
                    }
                    false
                }),
        );
    }

    fn card(&self, hosted: &Hosted<T>, paint: &style::Paint, toast: &Toast<T>, index: usize) -> El {
        let role = toast.tone.map_or(Semantic::Neutral, Into::into);
        let s = paint.tokens.toast(role);
        let g = s.overlay.geometry;
        let key = format!("toast-{index}");
        let id = toast.id;
        let mut text = col().gap(g.gap / 2.0).grow(1.0).min_w(0.0).child(words(
            paint,
            toast.title.clone(),
            g.title,
            s.overlay.surface.foreground,
            Flow::Wrap,
        ));
        if let Some(description) = &toast.description {
            text = text.child(words(
                paint,
                description.clone(),
                g.body,
                s.overlay.secondary,
                Flow::Wrap,
            ));
        }
        let mut card = row()
            .items_start()
            .gap(g.gap)
            .px(g.padding_x)
            .py(g.padding_y)
            .fill(ink(s.overlay.surface.background))
            .outline(g.border_width, 0.0, ink(s.overlay.surface.border))
            .radius(g.radius);
        if toast.tone.is_some()
            && let Some(mark) = Icon::status(role)
        {
            card = card.child(mark.mark(g.icon_size, ink(s.mark)));
        }
        card = card.child(text);
        if let Some((label, handler)) = &toast.action {
            let (toaster, handler) = (self.clone(), handler.clone());
            card = card.child(
                Button::new(
                    format!("{key}-action"),
                    label.clone(),
                    move |app: &mut T| {
                        handler(app);
                        toaster.dismiss(id);
                    },
                )
                .variant(ButtonVariant::Ghost)
                .show(hosted),
            );
        }
        let toaster = self.clone();
        card = card.child(
            Button::icon(
                format!("{key}-close"),
                Icon::Close,
                "Dismiss",
                move |_: &mut T| toaster.dismiss(id),
            )
            .variant(ButtonVariant::Ghost)
            .show(hosted),
        );
        apply(card.key(key), &toast.accessible())
    }
}
