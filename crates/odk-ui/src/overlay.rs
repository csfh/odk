//! The overlay layer of ADR 0037: what every popover, menu, sheet and toast shares. An overlay
//! is drawn where it is declared, lifted above the window with GPUI's `deferred`, and placed
//! with `anchored`. Inside a modal it is lifted to the window's host instead ([`lift`], ADR
//! 0039), since GPUI cannot nest `deferred` elements. This module adds what GPUI lacks: exit transitions ([`Presence`]), moving
//! focus in on open and back on close ([`layer`]), keeping Tab inside a modal ([`trap_focus`]),
//! and the layer order.

use std::cell::Cell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, Bounds, Div, Element, ElementId, FocusHandle, Global, GlobalElementId,
    InspectorElementId, InteractiveElement, IntoElement, KeyDownEvent, LayoutId, MouseButton,
    ParentElement, Pixels, Point, Position, SharedString, Stateful, Style, Styled, Window,
    WindowId, anchored, deferred, div, point, px,
};

use crate::focus::{FocusNext, FocusPrevious, child_id, focus_handle, move_focus, part_name};
use crate::layout::h_flex;
use crate::theme::colour;
use crate::{Button, Heading, Size, components};
use odk_theme::component::ButtonVariant;
use odk_theme::foundation::MotionDuration;

/// Which layer an overlay is drawn in; a higher one covers a lower one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Layer {
    /// Menus, hover cards: anchored to something in the window.
    Popover = 1,
    /// Sheets: cover the window with a scrim.
    Modal = 2,
    /// What opens inside a modal, drawn by the window's [`RaisedHost`] (see [`lift`]).
    Raised = 3,
    /// Toasts: above everything, even a modal.
    Toast = 4,
}

impl Layer {
    pub(crate) fn priority(self) -> usize {
        self as usize
    }
}

thread_local! {
    /// How many modal overlays the element being laid out is inside of.
    static INSIDE: Cell<usize> = const { Cell::new(0) };
}

/// Puts `INSIDE` back when a scope ends, even by a panic.
struct Entered(usize);

impl Entered {
    fn to(level: usize) -> Self {
        Self(INSIDE.replace(level))
    }
}

impl Drop for Entered {
    fn drop(&mut self) {
        INSIDE.set(self.0);
    }
}

/// An element that counts as inside a modal overlay while its child is laid out, so every
/// component below it that opens an overlay hands it to [`lift`] instead of deferring it.
struct Inside(AnyElement);

impl Element for Inside {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let _entered = Entered::to(INSIDE.get() + 1);
        (self.0.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0.paint(window, cx);
    }
}

impl IntoElement for Inside {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

/// Lifts `overlay` above the window in `layer`.
///
/// GPUI 0.2.2 cannot draw a `deferred` element inside another one, and a modal overlay is
/// deferred. So an overlay declared inside a modal's content is not deferred: it is kept for
/// the window's [`RaisedHost`], which draws it above every deferred overlay. Overlays elsewhere
/// are deferred in place, as ADR 0037 describes. `id` names the overlay, so one that is lifted
/// again is kept once.
pub(crate) fn lift(
    id: &ElementId,
    layer: Layer,
    overlay: impl IntoElement,
    window: &Window,
    cx: &mut App,
) -> AnyElement {
    let inside = INSIDE.get();
    if inside == 0 {
        return deferred(overlay)
            .with_priority(layer.priority())
            .into_any_element();
    }
    crate::app::require_shell(window, cx, "an overlay opened inside a modal");
    let key = format!("{id:?}");
    let kept = Kept {
        key: key.clone(),
        inside,
        layer,
        element: overlay.into_any_element(),
    };
    let all = cx
        .default_global::<Raised>()
        .windows
        .entry(window.window_handle().window_id())
        .or_default();
    // In the order they were lifted, so overlays that tie in the host's sort keep their order
    // from one frame to the next, as GPUI's own stable sort does for `deferred`.
    match all.iter_mut().find(|k| k.key == key) {
        Some(existing) => *existing = kept,
        None => all.push(kept),
    }
    div().into_any_element()
}

/// Drops what was lifted for windows that are not in `open`, so a window that closed with an
/// overlay open does not leave it in the store.
pub(crate) fn forget_closed_windows(open: &[WindowId], cx: &mut App) {
    if cx.has_global::<Raised>() {
        cx.global_mut::<Raised>()
            .windows
            .retain(|id, _| open.contains(id));
    }
}

/// An overlay waiting for the [`RaisedHost`].
struct Kept {
    /// The overlay's id, so one lifted again replaces itself.
    key: String,
    /// How many modals it is inside of, which orders it: an overlay inside a modal is above
    /// that modal, and below one opened from inside it.
    inside: usize,
    layer: Layer,
    element: AnyElement,
}

#[derive(Default)]
struct Raised {
    windows: HashMap<WindowId, Vec<Kept>>,
}

impl Global for Raised {}

/// Draws what [`lift`] kept, above every deferred overlay. The window's [`crate::Shell`] has one.
#[derive(IntoElement)]
pub(crate) struct RaisedHost;

impl gpui::RenderOnce for RaisedHost {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        deferred(RaisedDraw { drawn: Vec::new() }).with_priority(Layer::Raised.priority())
    }
}

/// The content of the [`RaisedHost`]'s deferred draw. It lays out what was kept while it is
/// prepainted, so everything kept this frame is there however the tree was ordered, and an
/// overlay kept while another is laid out (a dialog inside a sheet) is drawn too, above the
/// batch that held it.
struct RaisedDraw {
    drawn: Vec<Kept>,
}

impl Element for RaisedDraw {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        // Out of flow: a flex parent would otherwise leave a gap beside a node of no size.
        let style = Style {
            position: Position::Absolute,
            ..Style::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let id = window.window_handle().window_id();
        let viewport = window.viewport_size();
        self.drawn.clear();
        loop {
            let mut batch: Vec<Kept> = cx
                .default_global::<Raised>()
                .windows
                .remove(&id)
                .unwrap_or_default();
            if batch.is_empty() {
                break;
            }
            // Prepaint in the order they are painted, since the pointer finds what was
            // prepainted last on top. A stable sort, so overlays that tie keep the order they
            // were lifted in.
            batch.sort_by_key(|k| (k.inside, k.layer));
            for mut kept in batch {
                let _entered = Entered::to(kept.inside);
                kept.element
                    .prepaint_as_root(Point::default(), viewport.into(), window, cx);
                self.drawn.push(kept);
            }
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        for kept in &mut self.drawn {
            kept.element.paint(window, cx);
        }
    }
}

impl IntoElement for RaisedDraw {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

/// How long an overlay takes to come in and to go out. An overlay may run a different length
/// each way: a dialog enters over the foundation's `enter` and leaves over its `exit`. A plain
/// [`Duration`] is both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Transition {
    pub enter: Duration,
    pub exit: Duration,
}

impl Transition {
    /// The foundation's durations `enter` and `exit`: zero when motion is reduced.
    pub(crate) fn of(enter: MotionDuration, exit: MotionDuration) -> Self {
        let foundation = crate::foundation();
        let motion = &foundation.motion;
        Self {
            enter: motion.transition(enter),
            exit: motion.transition(exit),
        }
    }

    /// The length of a full transition toward `open`.
    fn toward(self, open: bool) -> Duration {
        if open { self.enter } else { self.exit }
    }
}

impl From<Duration> for Transition {
    fn from(both: Duration) -> Self {
        Self {
            enter: both,
            exit: both,
        }
    }
}

/// Whether an overlay is shown, and how far through its transition it is. GPUI animates only
/// an element that is being added, so a closing overlay keeps being drawn here until its exit
/// has run.
#[derive(Debug, Clone, Default)]
pub(crate) struct Presence {
    open: bool,
    /// How open it was when the current transition started.
    from: f32,
    since: Option<Instant>,
}

impl Presence {
    /// Already open or closed, with no transition to run.
    pub(crate) fn settled(open: bool) -> Self {
        Self {
            open,
            from: if open { 1.0 } else { 0.0 },
            since: None,
        }
    }

    /// Records the wanted state; a change starts a transition at `now` from wherever the
    /// previous one had got to, so reversing partway does not jump (checklist item 22).
    /// `duration` is the length of a full transition, as passed to [`Presence::amount`]; the
    /// previous transition's progress is read at the speed it was running.
    pub(crate) fn set(
        &mut self,
        open: bool,
        now: Instant,
        duration: impl Into<Transition>,
    ) -> bool {
        let duration = duration.into();
        let changed = open != self.open;
        if changed {
            self.from = self.value(now, duration);
            self.open = open;
            self.since = Some(now);
        }
        changed
    }

    /// How open the overlay is, from 0 (gone) to 1 (fully shown), or `None` once it has
    /// finished closing and need not be drawn. A full transition takes `duration`; a partial
    /// one takes its share of it.
    pub(crate) fn amount(&self, now: Instant, duration: impl Into<Transition>) -> Option<f32> {
        let value = self.value(now, duration.into());
        (self.open || value > 0.0).then_some(value)
    }

    fn value(&self, now: Instant, duration: Transition) -> f32 {
        let target = if self.open { 1.0 } else { 0.0 };
        let duration = duration.toward(self.open);
        let Some(since) = self.since.filter(|_| !duration.is_zero()) else {
            return target;
        };
        let step = now.saturating_duration_since(since).as_secs_f32() / duration.as_secs_f32();
        if self.open {
            (self.from + step).min(1.0)
        } else {
            (self.from - step).max(0.0)
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
    }
}

/// What a component needs to draw one overlay this frame.
pub(crate) struct Frame {
    /// How open it is (see [`Presence::amount`]); `None` means draw nothing.
    pub amount: Option<f32>,
    /// The handle the overlay's content tracks. Focus moves here on open.
    pub content: FocusHandle,
    /// True on the frame the overlay opened, for components that focus a part of it.
    pub opened: bool,
}

struct State {
    presence: Presence,
    content: Option<FocusHandle>,
    /// What had focus before the overlay opened, given focus back on close.
    restore: Option<FocusHandle>,
}

/// Tracks one overlay with id `id` that should be `open`, over `duration`: the foundation's
/// (`Transition::of`, zero with reduced motion). When it `takes_focus`, opening it remembers what
/// had focus and moves focus to the content, and closing it gives focus back if focus was still
/// inside. Keeps frames coming while a transition runs. The amount it gives is linear; ease it
/// with the foundation's curve to draw it.
pub(crate) fn layer_over(
    id: ElementId,
    open: bool,
    takes_focus: bool,
    duration: impl Into<Transition>,
    window: &mut Window,
    cx: &mut App,
) -> Frame {
    let duration = duration.into();
    let state = window.use_keyed_state(id.clone(), cx, |_, _| State {
        presence: Presence::default(),
        content: None,
        restore: None,
    });
    let content = focus_handle(crate::focus::child_id(&id, "content"), false, window, cx);
    // GPUI's clock: the system clock in an application, a clock the test controls in tests.
    let now = cx.background_executor().now();
    let focused = window.focused(cx);
    let content_has_focus = content.contains_focused(window, cx);
    let (amount, opened, restore) = state.update(cx, |s, _| {
        s.content = Some(content.clone());
        let changed = s.presence.set(open, now, duration);
        let mut restore = None;
        if takes_focus && changed && open {
            s.restore = focused.filter(|f| *f != content);
        } else if takes_focus && changed && content_has_focus {
            restore = s.restore.take();
        }
        (s.presence.amount(now, duration), changed && open, restore)
    });
    if opened && takes_focus {
        let content = content.clone();
        window.defer(cx, move |window, _| move_focus(&content, window));
    }
    if let Some(previous) = restore {
        window.defer(cx, move |window, _| move_focus(&previous, window));
    }
    if amount.is_some_and(|a| a < 1.0) || (amount.is_some() && !open) {
        crate::interaction::request_frame(window);
    }
    #[cfg(test)]
    probe::record(&id, amount);
    Frame {
        amount,
        content,
        opened,
    }
}

/// What had focus when the overlay `id` opened (and still stands for where it came from), for an
/// overlay that is not modal and lets Tab leave: focus goes on from there, so the keys carry on
/// from the trigger instead of from wherever the overlay is in the window's tab order.
pub(crate) fn opener(id: &ElementId, window: &mut Window, cx: &mut App) -> Option<FocusHandle> {
    let state = window.use_keyed_state(id.clone(), cx, |_, _| State {
        presence: Presence::default(),
        content: None,
        restore: None,
    });
    state.read(cx).restore.clone()
}

/// Keeps Tab and Shift+Tab inside `content`, for modal overlays. Put on the element that
/// tracks `content`; the actions reach it because focus is inside.
pub(crate) fn trap_focus<E: InteractiveElement>(el: E, content: &FocusHandle) -> E {
    let (next, previous) = (content.clone(), content.clone());
    el.on_action(move |_: &FocusNext, window, cx| {
        cycle_within(&next, window, cx, Window::focus_next)
    })
    .on_action(move |_: &FocusPrevious, window, cx| {
        cycle_within(&previous, window, cx, Window::focus_prev)
    })
}

/// Moves focus with `step` until it lands inside `content`, giving up after a full lap.
fn cycle_within(content: &FocusHandle, window: &mut Window, cx: &mut App, step: fn(&mut Window)) {
    for _ in 0..256 {
        step(window);
        if content.contains_focused(window, cx) {
            return;
        }
    }
}

pub(crate) type Action = std::rc::Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// An invisible layer over the whole window, under the overlay `id` is the name of, that takes
/// the press which dismisses the overlay: it asks to close, and nothing under it is pressed, as
/// in a native menu. Lift it before the overlay itself, so the overlay is over it. A press of
/// either main button counts; the wheel passes through nothing, since a menu is open.
pub(crate) fn dismiss_catcher(
    id: &ElementId,
    close: Action,
    window: &Window,
    cx: &mut App,
) -> AnyElement {
    let viewport = window.viewport_size();
    let (left, right) = (close.clone(), close);
    let catcher = anchored().position(point(px(0.0), px(0.0))).child(
        div()
            .w(viewport.width)
            .h(viewport.height)
            .occlude()
            .on_mouse_down(MouseButton::Left, move |_, window, cx| left(window, cx))
            .on_mouse_down(MouseButton::Right, move |_, window, cx| right(window, cx)),
    );
    lift(
        &child_id(id, "catcher"),
        Layer::Popover,
        catcher,
        window,
        cx,
    )
}

/// How a modal overlay is asked to close: through `action`, by Escape and by a press on the
/// backdrop as far as the policy allows. A confirmation that destroys something turns the
/// backdrop off, so a stray click does not dismiss it; a dialog that is busy turns both off.
#[derive(Clone)]
pub(crate) struct Dismiss {
    pub action: Action,
    /// Escape asks to close.
    pub escape: bool,
    /// A press on the backdrop asks to close.
    pub backdrop: bool,
}

impl Dismiss {
    /// Escape and the backdrop both close, as for a dialog that has not said otherwise.
    pub(crate) fn new(action: Action) -> Self {
        Self {
            action,
            escape: true,
            backdrop: true,
        }
    }
}

/// The header of a modal overlay: its title, wrapping when long, beside a close button that
/// keeps its place when `closable`.
pub(crate) fn modal_header(
    id: &ElementId,
    title: SharedString,
    closable: bool,
    close: Action,
) -> Div {
    let c = components();
    let gap = c.foundation().spacing.s3;
    h_flex()
        .items_start()
        .gap(px(gap))
        .child(
            div()
                .debug_selector(|| part_name(id, "title"))
                .flex_1()
                .min_w_0()
                .child(Heading::new(title).level(2)),
        )
        .children(closable.then(|| {
            Button::icon(child_id(id, "close"), "×", "Close")
                .variant(ButtonVariant::Ghost)
                .size(Size::Sm)
                .on_click(move |_, window, cx| close(window, cx))
        }))
}

/// A modal overlay: `panel` over a scrim that fills the window, `amount` open. The panel takes
/// the overlay's focus and keeps Tab inside; it asks to close on Escape, and a press on the
/// scrim asks too, as far as `dismiss` allows. The panel is centred unless it places itself
/// (`absolute`), as a sheet does. The scrim is the dialog's backdrop on component tokens.
pub(crate) fn modal(
    id: &ElementId,
    amount: f32,
    dismiss: &Dismiss,
    content: &FocusHandle,
    panel: Stateful<Div>,
    window: &Window,
    cx: &mut App,
) -> AnyElement {
    let backdrop = components().dialog().backdrop.map(colour);
    let viewport = window.viewport_size();
    let (key_close, escape, backdrop_close, on_backdrop) = (
        dismiss.action.clone(),
        dismiss.escape,
        dismiss.action.clone(),
        dismiss.backdrop,
    );
    let panel = panel.track_focus(content).occlude().on_key_down(
        move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.key == "escape" {
                if escape {
                    key_close(window, cx);
                }
                // The topmost overlay has heard it: an outer one does not close as well.
                cx.stop_propagation();
            }
        },
    );
    let panel = trap_focus(panel, content);
    let overlay = anchored().position(point(px(0.0), px(0.0))).child(
        div()
            .id(child_id(id, "scrim"))
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .w(viewport.width)
            .h(viewport.height)
            .overflow_hidden()
            .child(
                div()
                    .id(child_id(id, "dim"))
                    .debug_selector(|| part_name(id, "dim"))
                    .absolute()
                    .inset_0()
                    .when_some(backdrop, |d, c| d.bg(c))
                    .opacity(amount)
                    .occlude()
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        if on_backdrop {
                            backdrop_close(window, cx);
                        }
                    }),
            )
            // What opens inside the panel is lifted, not deferred inside this deferred draw.
            .child(Inside(panel.into_any_element())),
    );
    lift(id, Layer::Modal, overlay, window, cx)
}

/// A panel attached to the window, with no scrim and no focus trap: the rest of the window stays
/// live, and only the panel takes the pointer. Escape, with focus in the panel, asks to close as
/// far as `dismiss` allows (its `backdrop` has nothing to apply to). The panel places itself
/// (`absolute`), as a sheet's does. Drawn in the popover layer: over the page, under every modal.
pub(crate) fn docked(
    id: &ElementId,
    dismiss: &Dismiss,
    content: &FocusHandle,
    panel: Stateful<Div>,
    window: &Window,
    cx: &mut App,
) -> AnyElement {
    let viewport = window.viewport_size();
    let (key_close, escape) = (dismiss.action.clone(), dismiss.escape);
    let panel = panel.track_focus(content).occlude().on_key_down(
        move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.key == "escape" {
                if escape {
                    key_close(window, cx);
                }
                cx.stop_propagation();
            }
        },
    );
    // A layer the size of the window, to place the panel in and clip its slide. It has no
    // listeners, so it takes nothing the panel does not.
    let overlay = anchored().position(point(px(0.0), px(0.0))).child(
        div()
            .relative()
            .w(viewport.width)
            .h(viewport.height)
            .overflow_hidden()
            .child(Inside(panel.into_any_element())),
    );
    lift(id, Layer::Popover, overlay, window, cx)
}

/// What each overlay drew on each frame, for tests that follow a transition. Kept per thread,
/// like the test theme.
#[cfg(test)]
pub(crate) mod probe {
    use std::cell::RefCell;

    use gpui::ElementId;

    thread_local! {
        static FRAMES: RefCell<Vec<(String, Option<f32>)>> = const { RefCell::new(Vec::new()) };
    }

    pub(crate) fn record(id: &ElementId, amount: Option<f32>) {
        FRAMES.with(|f| f.borrow_mut().push((format!("{id:?}"), amount)));
    }

    /// How open the overlay whose id mentions `name` was on the last frame it was drawn.
    pub(crate) fn last(name: &str) -> Option<f32> {
        FRAMES.with(|f| {
            f.borrow()
                .iter()
                .rev()
                .find(|(id, _)| id.contains(name))
                .unwrap_or_else(|| panic!("no overlay named {name} has been drawn"))
                .1
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_and_closes_through_a_transition() {
        let d = Duration::from_millis(100);
        let t0 = Instant::now();
        let mut p = Presence::default();
        assert_eq!(
            p.amount(t0, d),
            None,
            "closed from the start: nothing to draw"
        );
        assert!(p.set(true, t0, d));
        assert!(!p.set(true, t0, d), "no change, no new transition");
        assert_eq!(p.amount(t0, d), Some(0.0));
        assert_eq!(p.amount(t0 + d / 2, d), Some(0.5));
        assert_eq!(p.amount(t0 + d * 2, d), Some(1.0));
        p.set(false, t0 + d, d);
        assert_eq!(p.amount(t0 + d + d / 4, d), Some(0.75));
        assert_eq!(p.amount(t0 + d * 2, d), None, "the exit has run");
    }

    #[test]
    fn reduced_motion_skips_the_transition() {
        let t0 = Instant::now();
        let mut p = Presence::default();
        p.set(true, t0, Duration::ZERO);
        assert_eq!(p.amount(t0, Duration::ZERO), Some(1.0));
        p.set(false, t0, Duration::ZERO);
        assert_eq!(p.amount(t0, Duration::ZERO), None);
    }
}
