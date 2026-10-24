use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{Modifiers, TestAppContext, VisualTestContext};
use odk_a11y::{Describe, Role};

use super::{Count, FIXTURES, open, press, publish_fixture, redraw};
use crate::{Button, Text, Toast, ToastId, Tone, dismiss_toast, set_toast_limit, toast};

/// A toast's number, as its card's part name has it.
fn number(id: ToastId) -> u64 {
    format!("{id:?}")
        .trim_start_matches("ToastId(")
        .trim_end_matches(')')
        .parse()
        .unwrap()
}

/// The bounds of a part by a name made on the spot; the names live for the test.
fn part(cx: &mut VisualTestContext, name: String) -> Option<gpui::Bounds<gpui::Pixels>> {
    cx.debug_bounds(Box::leak(name.into_boxed_str()))
}

fn shown(cx: &mut VisualTestContext, id: ToastId) -> bool {
    crate::toast::is_shown(cx, id)
}

fn wait(cx: &mut VisualTestContext, by: Duration) {
    cx.executor().advance_clock(by);
    redraw(cx);
}

/// Raises `t` in the window and draws it.
fn raise(cx: &mut VisualTestContext, t: Toast) -> ToastId {
    let id = t.id();
    cx.update(|window, cx| toast(window, cx, t));
    redraw(cx);
    id
}

fn page(cx: &mut TestAppContext) -> &mut VisualTestContext {
    open(cx, |_, _| Button::new("page").label("Page"))
}

/// How long toasts stay and take to go, on the foundation (full motion).
fn life() -> Duration {
    crate::foundation().motion.toast
}

fn exit() -> Duration {
    crate::foundation().motion.exit
}

/// Item 13: a toast never takes focus; the focused button still takes Enter.
#[gpui::test]
fn does_not_take_focus(cx: &mut TestAppContext) {
    let clicks = Count::default();
    let c = clicks.clone();
    let cx = open(cx, move |_, _| {
        let c = c.clone();
        Button::new("b")
            .label("Go")
            .on_click(move |_, _, _| c.bump())
    });
    press(cx, "tab");
    cx.update(|window, cx| {
        toast(
            window,
            cx,
            Toast::new("Saved")
                .variant(Tone::Success)
                .action("Undo", |_, _| {}),
        )
    });
    redraw(cx);
    press(cx, "enter");
    assert_eq!(clicks.get(), 1);
}

/// Item 14, 15.
#[test]
fn reports_a_status_named_by_its_title() {
    let a = Toast::new("Saved").accessible();
    assert_eq!((a.role, a.name.as_str()), (Role::Status, "Saved"));
}

/// A window opened with GPUI's own `open_window` has no toast host, so raising a toast there
/// says so instead of never showing it.
#[gpui::test]
#[should_panic(expected = "odk::ui::open_window")]
fn a_raw_window_cannot_show_a_toast(cx: &mut TestAppContext) {
    use gpui::{Context, IntoElement, Render, Window};
    struct Raw;
    impl Render for Raw {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            gpui::div()
        }
    }
    crate::theme::install_test_theme();
    let (_, cx) = cx.add_window_view(|_, _| Raw);
    cx.update(|window, cx| crate::toast(window, cx, crate::Toast::new("Saved")));
}

/// `open_window` is how an application gets its window, and a toast raised while its view is
/// being built, such as "restored your session", is shown: the window has its shell by then.
#[gpui::test]
fn a_toast_raised_while_the_view_is_built_is_shown(cx: &mut TestAppContext) {
    use gpui::{AppContext as _, Context, IntoElement, Render, Window, WindowOptions};
    struct Page;
    impl Render for Page {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            crate::root()
        }
    }
    crate::theme::install_test_theme();
    let id = Rc::new(Cell::new(None));
    let made = id.clone();
    let window = cx.update(|cx| {
        crate::open_window(cx, WindowOptions::default(), move |window, cx| {
            let t = crate::Toast::new("Restored your session");
            made.set(Some(t.id()));
            crate::toast(window, cx, t);
            cx.new(|_| Page)
        })
        .unwrap()
    });
    cx.run_until_parked();
    let mut cx = gpui::VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();
    assert!(shown(&mut cx, id.get().unwrap()), "the toast is on screen");
}

/// A toast stays for the foundation's `toast`, then goes after its exit; its own button
/// dismisses it at once.
#[gpui::test]
fn a_toast_dismisses_itself_after_its_time_and_by_its_button(cx: &mut TestAppContext) {
    let cx = page(cx);
    let timed = raise(cx, Toast::new("Saved"));
    wait(cx, life() - Duration::from_millis(100));
    assert!(shown(cx, timed), "still there just before its time");
    wait(cx, Duration::from_millis(100) + exit() / 2);
    assert!(shown(cx, timed), "going, not gone");
    wait(cx, exit());
    assert!(!shown(cx, timed), "gone after its exit");

    let held = raise(cx, Toast::new("Copied"));
    let at = part(cx, format!("odk-toast-dismiss-{}/button", number(held)))
        .unwrap()
        .center();
    cx.simulate_click(at, Modifiers::none());
    redraw(cx);
    wait(cx, exit() * 2);
    assert!(!shown(cx, held), "its dismiss button removed it");
}

/// A toast with an action stays twice as long, time enough to reach the action.
#[gpui::test]
fn a_toast_with_an_action_stays_longer(cx: &mut TestAppContext) {
    let cx = page(cx);
    let plain = raise(cx, Toast::new("Saved"));
    let acting = raise(cx, Toast::new("Deleted").action("Undo", |_, _| {}));
    wait(cx, life() + exit() * 2);
    assert!(!shown(cx, plain));
    assert!(shown(cx, acting), "the one with an action is still there");
    wait(cx, life());
    wait(cx, exit() * 2);
    assert!(!shown(cx, acting));
}

/// A persistent toast stays until it is dismissed; an explicit timeout is honoured.
#[gpui::test]
fn a_persistent_toast_stays_and_a_timeout_is_honoured(cx: &mut TestAppContext) {
    let cx = page(cx);
    let error = raise(
        cx,
        Toast::new("Could not save")
            .variant(Tone::Danger)
            .persistent(),
    );
    let brief = raise(cx, Toast::new("Hi").timeout(Duration::from_secs(1)));
    wait(cx, Duration::from_secs(2) + exit() * 2);
    assert!(!shown(cx, brief), "the short timeout ran");
    wait(cx, Duration::from_secs(3600));
    assert!(shown(cx, error), "an hour later the error is still there");
    cx.update(|window, cx| dismiss_toast(window, cx, error));
    redraw(cx);
    wait(cx, exit() * 2);
    assert!(!shown(cx, error), "until the application dismissed it");
}

/// At most three show at once, the rest wait their turn and come up as room is made; the limit
/// can be changed.
#[gpui::test]
fn only_the_limit_shows_and_the_rest_wait(cx: &mut TestAppContext) {
    let cx = page(cx);
    let ids: Vec<ToastId> = (0..5)
        .map(|n| raise(cx, Toast::new(format!("Toast {n}")).persistent()))
        .collect();
    let visible = |cx: &mut VisualTestContext| ids.iter().filter(|id| shown(cx, **id)).count();
    assert_eq!(visible(cx), crate::MAX_VISIBLE_TOASTS);
    assert!(
        shown(cx, ids[0]) && !shown(cx, ids[3]),
        "first come, first shown"
    );
    cx.update(|window, cx| dismiss_toast(window, cx, ids[0]));
    redraw(cx);
    wait(cx, exit() * 2);
    assert!(shown(cx, ids[3]), "the next waiting one came up");
    assert_eq!(visible(cx), crate::MAX_VISIBLE_TOASTS);
    cx.update(|_, cx| set_toast_limit(cx, 5));
    redraw(cx);
    assert_eq!(visible(cx), 4, "all that are left now show");
}

/// Stacking: the first toast is above the later ones, all against the same edge.
#[gpui::test]
fn toasts_stack_in_the_order_they_came(cx: &mut TestAppContext) {
    let cx = page(cx);
    let a = raise(cx, Toast::new("First").persistent());
    let b = raise(cx, Toast::new("Second").persistent());
    wait(cx, exit());
    let (ba, bb) = (
        part(cx, format!("odk-toast/card-{}", number(a))).unwrap(),
        part(cx, format!("odk-toast/card-{}", number(b))).unwrap(),
    );
    assert!(ba.bottom() <= bb.top(), "the first is above the second");
    assert_eq!(ba.right(), bb.right(), "against the same edge");
}

/// Duplicate ids: raising a toast with an id already shown updates that one in place and starts
/// its clock again; it does not stack a second.
#[gpui::test]
fn the_same_id_updates_a_toast_in_place(cx: &mut TestAppContext) {
    let cx = page(cx);
    let id = ToastId::named("saving");
    raise(cx, Toast::new("Saving…").with_id(id));
    wait(cx, life() - Duration::from_millis(500));
    raise(cx, Toast::new("Saved").with_id(id).variant(Tone::Success));
    wait(cx, Duration::from_secs(1));
    assert!(
        shown(cx, id),
        "still there after the first clock would have run out"
    );
    assert_eq!(crate::toast::entries(cx), 1, "and it is the only one");
}

/// The toast's button can be reached with Tab and pressed with Enter, and runs its handler once
/// and dismisses the toast.
#[gpui::test]
fn its_action_is_reached_by_keyboard_and_runs_once(cx: &mut TestAppContext) {
    let ran = Count::default();
    let cx = page(cx);
    let r = ran.clone();
    let id = raise(
        cx,
        Toast::new("Deleted")
            .action("Undo", move |_, _| r.bump())
            .persistent(),
    );
    // The page's button, then the toast's action.
    press(cx, "tab tab enter");
    assert_eq!(ran.get(), 1, "the action ran once");
    wait(cx, exit() * 2);
    assert!(!shown(cx, id), "and dismissed the toast");
}

/// Escape while focus is in a toast dismisses it.
#[gpui::test]
fn escape_dismisses_the_toast_that_has_focus(cx: &mut TestAppContext) {
    let cx = page(cx);
    let id = raise(
        cx,
        Toast::new("Deleted").action("Undo", |_, _| {}).persistent(),
    );
    press(cx, "tab tab escape");
    wait(cx, exit() * 2);
    assert!(!shown(cx, id));
}

/// The clocks stop while focus is inside the stack, and run again when it leaves.
#[gpui::test]
fn focus_in_a_toast_pauses_its_clock(cx: &mut TestAppContext) {
    let cx = page(cx);
    let id = raise(cx, Toast::new("Deleted").action("Undo", |_, _| {}));
    press(cx, "tab tab");
    wait(cx, life() * 10);
    assert!(shown(cx, id), "focus held it");
    press(cx, "shift-tab");
    wait(cx, life() * 3 + exit() * 2);
    assert!(!shown(cx, id), "released, it ran out");
}

/// The pointer over the stack pauses the clocks too.
#[gpui::test]
fn the_pointer_over_a_toast_pauses_its_clock(cx: &mut TestAppContext) {
    let cx = page(cx);
    let id = raise(cx, Toast::new("Saved"));
    let at = part(cx, format!("odk-toast/card-{}", number(id)))
        .unwrap()
        .center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    wait(cx, life() * 4);
    assert!(shown(cx, id), "held while the pointer is over it");
    cx.simulate_mouse_move(
        gpui::point(gpui::px(2.0), gpui::px(2.0)),
        None,
        Modifiers::none(),
    );
    wait(cx, life() + exit() * 2);
    assert!(!shown(cx, id));
}

/// A theme change colours open toasts without restarting their clocks, closing them or taking
/// focus; the deadline stays the one they started with.
#[gpui::test]
fn a_theme_change_keeps_a_toast_and_its_deadline(cx: &mut TestAppContext) {
    let cx = page(cx);
    let id = raise(cx, Toast::new("Saved"));
    wait(cx, life() - Duration::from_millis(300));
    for (_, colours) in FIXTURES {
        publish_fixture(colours);
        redraw(cx);
        assert!(shown(cx, id), "still shown after a theme change");
    }
    wait(cx, Duration::from_millis(300) + exit() * 2);
    assert!(
        !shown(cx, id),
        "the theme changes did not give it a new clock"
    );
}

/// What is kept for a window goes with it: a window that closes with a toast open leaves
/// nothing behind.
#[gpui::test]
fn a_closed_window_leaves_no_toasts_behind(cx: &mut TestAppContext) {
    let first = page(cx);
    raise(first, Toast::new("Hello").persistent());
    assert_eq!(crate::toast::tracked_windows(first), 1);
    first.update(|window, _| window.remove_window());
    first.run_until_parked();
    let second = open(cx, |_, _| Text::new("another"));
    raise(second, Toast::new("Again").persistent());
    assert_eq!(
        crate::toast::tracked_windows(second),
        1,
        "only the open window is tracked"
    );
}
