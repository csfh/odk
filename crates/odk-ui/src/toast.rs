//! Toasts: short messages raised from anywhere in a window with [`toast`], on the toast's
//! component token (`docs/spec/odk-component-tokens.md`, `ComponentTokens::toast`). They stack in
//! the bottom-right corner above everything else, even a modal. Each stays for the foundation's
//! `toast` (twice as long when it has an action), until it is dismissed, or for as long as the
//! application says, and a persistent one stays until it is dismissed. The clocks stop while the
//! pointer is over the stack or focus is inside it. At most [`MAX_VISIBLE`] show at a time, or
//! the limit of [`set_toast_limit`]; the rest wait their turn. They never take focus. The
//! window's [`crate::Shell`] draws them, and this module is the one manager: it hands out
//! [`ToastId`]s, runs the clocks and removes a toast once its exit has run.

use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    App, Global, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, WindowId, anchored, deferred, div,
    point, px,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::ButtonVariant;
use odk_theme::foundation::MotionDuration;
use odk_theme::semantic::Role as Semantic;

use crate::focus::focus_handle;
use crate::layout::{StyledExt, h_flex, inside_window, v_flex};
use crate::overlay::{Layer, Presence, Transition};
use crate::theme::{colour, mono};
use crate::{Button, Size, Tone, components};

/// The most toasts shown at once unless [`set_toast_limit`] says otherwise. A count, not a
/// length.
pub const MAX_VISIBLE: usize = 3;

/// The width of a toast, in logical pixels. ODK's own (ADR 0035).
const TOAST_WIDTH: f32 = 336.0;

/// How many times as long as an ordinary toast one with an action stays: time enough to reach
/// the action and press it. ODK's own.
const ACTION_TIME: u32 = 2;

type Handler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// What names a toast: every [`Toast`] has one from the moment it is made ([`Toast::id`]),
/// distinct from every other made on the thread, and stable while the toast lives. An
/// application that wants to name its own, to update or dismiss a toast it does not hold, makes
/// one with [`ToastId::named`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToastId(u64);

thread_local! {
    /// The last id handed out on this thread. Toasts are raised from the UI thread.
    static LAST_ID: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

impl ToastId {
    /// An id no other has.
    fn fresh() -> Self {
        Self(LAST_ID.with(|n| {
            n.set(n.get() + 1);
            n.get()
        }))
    }

    /// The id that `name` always gives: the same toast every time it is raised, so one that can
    /// repeat, such as "Saving…" or a failed retry, updates in place instead of stacking, and
    /// the application can dismiss it without keeping anything.
    pub fn named(name: &str) -> Self {
        // FNV-1a, with the top bit set so that a name never gives a fresh id's number.
        let hash = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        });
        Self(hash | (1 << 63))
    }
}

/// How long a toast stays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lifetime {
    /// The foundation's `toast`, doubled with an action.
    Default,
    For(Duration),
    /// Until it is dismissed.
    Persistent,
}

/// A message for [`toast`].
pub struct Toast {
    /// `None` is a neutral toast, with no mark.
    tone: Option<Tone>,
    id: ToastId,
    title: SharedString,
    description: Option<SharedString>,
    action: Option<(SharedString, Handler)>,
    lifetime: Lifetime,
}

impl Toast {
    /// An info toast; see [`Toast::variant`] and [`Toast::neutral`] for the others.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            tone: Some(Tone::default()),
            id: ToastId::fresh(),
            title: title.into(),
            description: None,
            action: None,
            lifetime: Lifetime::Default,
        }
    }

    /// Info, success, warning or danger: the mark and its colour.
    pub fn variant(mut self, tone: Tone) -> Self {
        self.tone = Some(tone);
        self
    }

    /// A toast with no status: no mark, the surface's own colours.
    pub fn neutral(mut self) -> Self {
        self.tone = None;
        self
    }

    /// Names the toast with `id`, so raising another with the same id while one is still shown
    /// (or waiting) updates that one in place and restarts its clock instead of stacking a
    /// second.
    pub fn with_id(mut self, id: ToastId) -> Self {
        self.id = id;
        self
    }

    /// What names this toast, for [`dismiss_toast`].
    pub fn id(&self) -> ToastId {
        self.id
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// A button on the toast. Pressing it runs `handler` and dismisses the toast.
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), Rc::new(handler)));
        self
    }

    /// Stays until it is dismissed, by its button, Escape while focus is in it, or
    /// [`dismiss_toast`]: for an error the user must see.
    pub fn persistent(mut self) -> Self {
        self.lifetime = Lifetime::Persistent;
        self
    }

    /// Stays this long instead of the foundation's `toast`.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.lifetime = Lifetime::For(timeout);
        self
    }

    fn role(&self) -> Semantic {
        self.tone.map_or(Semantic::Neutral, Into::into)
    }

    /// How long it stays, given the foundation's `toast`; `None` for ever.
    fn life(&self, base: Duration) -> Option<Duration> {
        match self.lifetime {
            Lifetime::Persistent => None,
            Lifetime::For(own) => Some(own),
            Lifetime::Default if self.action.is_some() => Some(base * ACTION_TIME),
            Lifetime::Default => Some(base),
        }
    }
}

impl Describe for Toast {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Status, self.title.to_string())
    }
}

/// Shows `toast` in `window`. A toast whose id one still shown (or waiting) already has replaces
/// that one's content and starts its clock again, instead of stacking a second.
pub fn toast(window: &mut Window, cx: &mut App, toast: Toast) {
    crate::app::require_shell(window, cx, "a toast");
    let id = window.window_handle().window_id();
    let now = cx.background_executor().now();
    forget_closed_windows(cx);
    let store = cx.default_global::<Toasts>();
    let stack = store.windows.entry(id).or_default();
    let paused = stack.paused;
    if let Some(entry) = stack.entries.iter_mut().find(|e| e.toast.id == toast.id) {
        entry.toast = toast;
        entry.restart(now, paused);
    } else {
        stack.entries.push(Entry {
            toast,
            presence: Presence::default(),
            remaining: None,
            running_since: None,
            shown: false,
        });
    }
    window.refresh();
}

/// Dismisses the toast `id` in `window`: its exit runs and it goes. Nothing happens if it has
/// gone already.
pub fn dismiss_toast(window: &mut Window, cx: &mut App, id: ToastId) {
    let window_id = window.window_handle().window_id();
    let now = cx.background_executor().now();
    if cx.has_global::<Toasts>()
        && let Some(stack) = cx.global_mut::<Toasts>().windows.get_mut(&window_id)
    {
        stack.dismiss(id, now, toast_timing());
    }
    window.refresh();
}

/// How many toasts the application shows at once; the rest wait. At least one. A change shows at
/// the next draw.
pub fn set_toast_limit(cx: &mut App, limit: usize) {
    cx.default_global::<Toasts>().limit = limit.max(1);
}

/// What a toast takes to come and to go: the foundation's `normal` and `exit`, none with reduced
/// motion.
fn toast_timing() -> Transition {
    Transition::of(MotionDuration::Normal, MotionDuration::Exit)
}

struct Entry {
    toast: Toast,
    presence: Presence,
    /// Time left before it dismisses itself, not counting since `running_since`; `None` if it
    /// never does.
    remaining: Option<Duration>,
    running_since: Option<Instant>,
    /// Whether it has been on screen; a waiting toast's clock has not started.
    shown: bool,
}

impl Entry {
    /// Starts it over: open again if it was going, with all its time.
    fn restart(&mut self, now: Instant, paused: bool) {
        if self.shown {
            self.presence.set(true, now, toast_timing());
            self.remaining = self.toast.life(crate::foundation().motion.toast);
            self.running_since = (!paused && self.remaining.is_some()).then_some(now);
        }
    }
}

#[derive(Default)]
struct Stack {
    entries: Vec<Entry>,
    paused: bool,
    /// Whether the pointer is over the stack, and whether focus is in it.
    hovered: bool,
    focused: bool,
    /// The deadline a wake-up is already set for, so each render does not set another.
    wake_at: Option<Instant>,
}

struct Toasts {
    windows: HashMap<WindowId, Stack>,
    limit: usize,
}

impl Default for Toasts {
    fn default() -> Self {
        Self {
            windows: HashMap::new(),
            limit: MAX_VISIBLE,
        }
    }
}

impl Global for Toasts {}

/// Drops what is kept for windows that have closed, so a long-running application that opens
/// and closes windows does not collect their toasts.
fn forget_closed_windows(cx: &mut App) {
    let open: Vec<WindowId> = cx.windows().iter().map(|w| w.window_id()).collect();
    if cx.has_global::<Toasts>() {
        cx.global_mut::<Toasts>()
            .windows
            .retain(|id, _| open.contains(id));
    }
    crate::overlay::forget_closed_windows(&open, cx);
}

impl Stack {
    /// Advances the clocks to `now`: starts the toasts that have come on screen, closes
    /// the expired ones, drops the ones whose exit has run. Returns the next deadline.
    fn tick(
        &mut self,
        now: Instant,
        base: Duration,
        limit: usize,
        transition: impl Into<Transition>,
    ) -> Option<Instant> {
        let transition = transition.into();
        self.entries
            .retain(|e| !e.shown || e.presence.amount(now, transition).is_some());
        let mut slots = limit;
        let mut next = None::<Instant>;
        let paused = self.paused;
        for e in &mut self.entries {
            if slots == 0 {
                break;
            }
            if !e.shown {
                e.shown = true;
                e.presence.set(true, now, transition);
                e.remaining = e.toast.life(base);
                e.running_since = (!paused && e.remaining.is_some()).then_some(now);
            }
            if e.presence.is_open() {
                slots -= 1;
                // A timeout too long for the clock to count is one that never ends.
                if let (Some(since), Some(left)) = (e.running_since, e.remaining)
                    && let Some(deadline) = since.checked_add(left)
                {
                    if deadline <= now {
                        e.presence.set(false, now, transition);
                    } else {
                        next = Some(next.map_or(deadline, |n| n.min(deadline)));
                    }
                }
            }
        }
        next
    }

    /// Stops or restarts every clock, without losing the time each has left.
    fn pause(&mut self, paused: bool, now: Instant) {
        if paused == self.paused {
            return;
        }
        self.paused = paused;
        for e in self.entries.iter_mut().filter(|e| e.shown) {
            if paused {
                if let (Some(since), Some(left)) = (e.running_since.take(), e.remaining) {
                    e.remaining = Some(left.saturating_sub(now - since));
                }
            } else if e.remaining.is_some() {
                e.running_since = Some(now);
            }
        }
    }

    fn dismiss(&mut self, id: ToastId, now: Instant, transition: impl Into<Transition>) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.toast.id == id) {
            // One that never came on screen simply goes.
            if e.shown {
                e.presence.set(false, now, transition);
            } else {
                e.shown = true;
                e.presence = Presence::settled(false);
            }
        }
    }
}

/// What one toast needs to be drawn this frame, copied out of the store.
struct Shown {
    id: ToastId,
    amount: f32,
    role: Semantic,
    mark: Option<&'static str>,
    title: SharedString,
    description: Option<SharedString>,
    action: Option<(SharedString, Handler)>,
}

/// Draws the window's toasts. The window's [`crate::Shell`] has one.
#[derive(IntoElement)]
pub(crate) struct ToastHost;

impl RenderOnce for ToastHost {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = window.window_handle().window_id();
        let c = components();
        let f = c.foundation().clone();
        // GPUI's clock: the system clock in an application, a clock the test controls in tests.
        let now = cx.background_executor().now();
        let timing = toast_timing();
        let Some(store) = cx.try_global::<Toasts>() else {
            return div().into_any_element();
        };
        if !store.windows.contains_key(&id) {
            return div().into_any_element();
        }
        let focus = focus_handle("odk-toasts-focus".into(), false, window, cx);
        let focused = focus.contains_focused(window, cx);
        let store = cx.global_mut::<Toasts>();
        let limit = store.limit;
        let stack = store.windows.get_mut(&id).expect("checked above");
        stack.focused = focused;
        stack.pause(stack.hovered || stack.focused, now);
        let next = stack.tick(now, f.motion.toast, limit, timing);
        let animating = stack.entries.iter().filter(|e| e.shown).any(|e| {
            e.presence.amount(now, timing).is_some_and(|a| a < 1.0) || !e.presence.is_open()
        });
        if animating {
            crate::interaction::request_frame(window);
        }
        let shown: Vec<Shown> = stack
            .entries
            .iter()
            .filter(|e| e.shown)
            .filter_map(|e| {
                let a = e.presence.amount(now, timing)?;
                let tone = e.toast.tone;
                Some(Shown {
                    id: e.toast.id,
                    amount: a,
                    role: e.toast.role(),
                    mark: tone.map(Tone::mark),
                    title: e.toast.title.clone(),
                    description: e.toast.description.clone(),
                    action: e.toast.action.clone(),
                })
            })
            .collect();
        let wake = next.filter(|d| stack.wake_at != Some(*d));
        if wake.is_some() {
            stack.wake_at = wake;
        }
        if let Some(deadline) = wake {
            window
                .spawn(cx, async move |cx| {
                    let executor = cx.background_executor().clone();
                    executor
                        .timer(deadline.saturating_duration_since(executor.now()))
                        .await;
                    let _ = cx.update(|window, _| window.refresh());
                })
                .detach();
        }
        if shown.is_empty() {
            return div().into_any_element();
        }

        let viewport = window.viewport_size();
        let margin = px(f.spacing.s4);
        let mut column = v_flex()
            .id("odk-toasts")
            .track_focus(&focus)
            .gap(px(f.spacing.s2))
            // Never wider than the window, less its margins.
            .w(px(TOAST_WIDTH).min(inside_window(viewport.width, margin)))
            .on_hover(move |over, _, cx| {
                let (over, now) = (*over, cx.background_executor().now());
                if let Some(stack) = cx.global_mut::<Toasts>().windows.get_mut(&id) {
                    stack.hovered = over;
                    stack.pause(stack.hovered || stack.focused, now);
                }
            });
        for Shown {
            id: toast_id,
            amount,
            role,
            mark,
            title,
            description,
            action,
        } in shown
        {
            let toast_id_n = toast_id.0;
            let style = c.toast(role);
            let g = style.overlay.geometry;
            let amount = f.motion.easing.apply(amount);
            let dismiss = move |window: &mut Window, cx: &mut App| {
                let now = cx.background_executor().now();
                if let Some(stack) = cx.global_mut::<Toasts>().windows.get_mut(&id) {
                    stack.dismiss(toast_id, now, toast_timing());
                }
                window.refresh();
            };
            // `min_w_0` lets the text be narrower than its longest word, so long words and
            // URLs wrap inside the box and the trailing buttons keep their place.
            let mut text = v_flex().flex_1().min_w_0().gap(px(f.spacing.s1)).child(
                div()
                    .text_role(g.title)
                    .text_color(colour(style.overlay.surface.foreground))
                    .child(title),
            );
            if let Some(d) = description {
                text = text.child(
                    div()
                        .text_role(g.body)
                        .text_color(colour(style.overlay.secondary))
                        .child(d),
                );
            }
            // The action goes under the text rather than beside it, so the text keeps the
            // toast's width in a narrow window instead of what a button leaves of it.
            text = text.children(action.map(|(label, handler)| {
                Button::new(("odk-toast-action", toast_id_n as usize))
                    .label(label)
                    .variant(ButtonVariant::Secondary)
                    .size(Size::Sm)
                    .on_click(move |_, window, cx| {
                        handler(window, cx);
                        dismiss(window, cx);
                    })
            }));
            let key_dismiss = dismiss;
            let card = h_flex()
                .debug_selector(|| format!("odk-toast/card-{toast_id_n}"))
                .items_start()
                .gap(px(g.gap + f.spacing.s1))
                .px(px(g.padding_x))
                .py(px(g.padding_y))
                .treatment(&style.overlay.surface)
                .border(px(g.border_width))
                .rounded(px(g.radius))
                .text_role(g.body)
                .opacity(amount)
                // A short slide in from the edge.
                .mr(px(-(1.0 - amount) * f.spacing.s4))
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        key_dismiss(window, cx);
                        cx.stop_propagation();
                    }
                })
                .children(mark.map(|mark| {
                    div()
                        .font_family(mono())
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(colour(style.mark))
                        .child(mark)
                }))
                .child(text)
                .child(
                    Button::icon(("odk-toast-dismiss", toast_id_n as usize), "×", "Dismiss")
                        .variant(ButtonVariant::Ghost)
                        .size(Size::Sm)
                        .on_click(move |_, window, cx| dismiss(window, cx)),
                );
            column = column.child(card.relative());
        }

        deferred(
            anchored()
                .anchor(gpui::Corner::BottomRight)
                .position(point(viewport.width - margin, viewport.height - margin))
                .child(column),
        )
        .with_priority(Layer::Toast.priority())
        .into_any_element()
    }
}

/// How many windows the toast store is keeping something for, for tests.
#[cfg(test)]
pub(crate) fn tracked_windows(cx: &mut gpui::VisualTestContext) -> usize {
    cx.update(|_, cx| cx.try_global::<Toasts>().map_or(0, |t| t.windows.len()))
}

/// Whether the toast `id` is on screen or going: it has come up and its exit has not run. GPUI
/// keeps the last bounds of a part that is no longer drawn, so a test cannot ask the bounds.
#[cfg(test)]
pub(crate) fn is_shown(cx: &mut gpui::VisualTestContext, id: ToastId) -> bool {
    cx.update(|window, cx| {
        let now = cx.background_executor().now();
        let window_id = window.window_handle().window_id();
        cx.try_global::<Toasts>()
            .and_then(|t| t.windows.get(&window_id))
            .and_then(|s| s.entries.iter().find(|e| e.toast.id == id))
            .is_some_and(|e| e.shown && e.presence.amount(now, toast_timing()).is_some())
    })
}

/// How many toasts the window has, shown or waiting, for tests.
#[cfg(test)]
pub(crate) fn entries(cx: &mut gpui::VisualTestContext) -> usize {
    cx.update(|window, cx| {
        let id = window.window_handle().window_id();
        cx.try_global::<Toasts>()
            .and_then(|t| t.windows.get(&id))
            .map_or(0, |s| s.entries.len())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(n: usize) -> Stack {
        let mut s = Stack::default();
        for id in 0..n as u64 {
            s.entries.push(Entry {
                toast: Toast::new("t").with_id(ToastId(id)),
                presence: Presence::default(),
                remaining: None,
                running_since: None,
                shown: false,
            });
        }
        s
    }

    const LIFE: Duration = Duration::from_secs(5);
    const EXIT: Duration = Duration::from_millis(100);

    #[test]
    fn shows_at_most_the_cap_and_queues_the_rest() {
        let mut s = stack(5);
        let t0 = Instant::now();
        assert_eq!(s.tick(t0, LIFE, MAX_VISIBLE, EXIT), Some(t0 + LIFE));
        assert_eq!(s.entries.iter().filter(|e| e.shown).count(), MAX_VISIBLE);
        let mut s = stack(5);
        s.tick(t0, LIFE, 1, EXIT);
        assert_eq!(
            s.entries.iter().filter(|e| e.shown).count(),
            1,
            "a lower cap"
        );
    }

    #[test]
    fn expires_then_removes_after_the_exit() {
        let mut s = stack(1);
        let t0 = Instant::now();
        s.tick(t0, LIFE, MAX_VISIBLE, EXIT);
        s.tick(t0 + LIFE, LIFE, MAX_VISIBLE, EXIT);
        assert!(!s.entries[0].presence.is_open(), "closing");
        s.tick(t0 + LIFE + EXIT, LIFE, MAX_VISIBLE, EXIT);
        assert!(s.entries.is_empty());
    }

    #[test]
    fn hovering_pauses_the_clock() {
        let mut s = stack(1);
        let t0 = Instant::now();
        s.tick(t0, LIFE, MAX_VISIBLE, EXIT);
        s.pause(true, t0 + Duration::from_secs(2));
        assert_eq!(
            s.tick(t0 + Duration::from_secs(60), LIFE, MAX_VISIBLE, EXIT),
            None
        );
        assert!(s.entries[0].presence.is_open(), "still open while paused");
        let t1 = t0 + Duration::from_secs(60);
        s.pause(false, t1);
        assert_eq!(
            s.tick(t1, LIFE, MAX_VISIBLE, EXIT),
            Some(t1 + Duration::from_secs(3))
        );
    }

    #[test]
    fn dismissing_starts_the_exit() {
        let mut s = stack(2);
        let t0 = Instant::now();
        s.tick(t0, LIFE, MAX_VISIBLE, EXIT);
        s.dismiss(ToastId(0), t0, EXIT);
        assert!(!s.entries[0].presence.is_open());
        assert!(s.entries[1].presence.is_open());
    }

    #[test]
    fn a_persistent_toast_never_expires_and_takes_no_wake_up() {
        let mut s = stack(1);
        s.entries[0].toast = Toast::new("t").persistent();
        let t0 = Instant::now();
        assert_eq!(s.tick(t0, LIFE, MAX_VISIBLE, EXIT), None, "no deadline");
        let later = t0 + Duration::from_secs(3600);
        assert_eq!(s.tick(later, LIFE, MAX_VISIBLE, EXIT), None);
        assert!(s.entries[0].presence.is_open());
    }

    #[test]
    fn a_toast_with_an_action_stays_longer() {
        let plain = Toast::new("t");
        let acting = Toast::new("t").action("Undo", |_, _| {});
        assert_eq!(plain.life(LIFE), Some(LIFE));
        assert_eq!(acting.life(LIFE), Some(LIFE * ACTION_TIME));
        assert_eq!(
            acting.timeout(Duration::from_secs(1)).life(LIFE),
            Some(Duration::from_secs(1)),
            "an explicit timeout wins"
        );
    }

    #[test]
    fn a_waiting_toast_has_no_clock_until_it_is_shown() {
        let mut s = stack(2);
        let t0 = Instant::now();
        s.tick(t0, LIFE, 1, EXIT);
        assert!(s.entries[1].running_since.is_none());
        s.dismiss(ToastId(0), t0, EXIT);
        s.tick(t0 + EXIT, LIFE, 1, EXIT);
        assert_eq!(
            s.entries[0].toast.id,
            ToastId(1),
            "the one that waited comes up when the first has gone"
        );
        assert_eq!(s.entries[0].running_since, Some(t0 + EXIT));
    }

    #[test]
    fn a_timeout_too_long_to_count_is_a_toast_that_stays() {
        let mut s = stack(1);
        s.entries[0].toast = Toast::new("t").timeout(Duration::MAX);
        let t0 = Instant::now();
        assert_eq!(s.tick(t0, LIFE, MAX_VISIBLE, EXIT), None, "no deadline");
        assert!(s.entries[0].presence.is_open());
    }
}
