//! Interaction rules every control that activates shares (items 6, 10 and 38,
//! `docs/spec/odk-interaction.md`), run over the controls together so that a new rule is
//! checked against all of them and not only the one it was found in: a press released elsewhere does nothing, a disabled control
//! does nothing from the pointer or the keyboard, and a key held down acts once.
//!
//! Each control's own keys, and what Enter does to it, are in its own module; this one asks only
//! what must be true of all of them.

use gpui::{
    AnyElement, IntoElement, KeyUpEvent, Keystroke, Modifiers, MouseButton, ParentElement as _,
    TestAppContext, VisualTestContext, point, px,
};

use super::{Count, open, press, redraw};
use crate::{Checkbox, NavList, RadioGroup, Switch, Tabs, Text, ToggleGroup};

/// A control that activates by a press and release on it, with the part to press and how it
/// is built.
struct Case {
    name: &'static str,
    /// The part a click is aimed at.
    part: &'static str,
    /// Builds the control, counting every change it reports into the count, and disabled or not.
    build: fn(Count, bool) -> AnyElement,
    /// Whether Space, on the control once focused, makes one change (Enter or Space on a toggle, a
    /// radio, a list row); false for tabs, which are selected as focus reaches them.
    space_acts: bool,
}

const CASES: [Case; 6] = [
    Case {
        name: "Checkbox",
        part: "c/box",
        build: |n, off| {
            Checkbox::new("c", "Wrap lines")
                .disabled(off)
                .on_change(move |_, _, _| n.bump())
                .into_any_element()
        },
        space_acts: true,
    },
    Case {
        name: "Switch",
        part: "s/track",
        build: |n, off| {
            Switch::new("s", "Wi-Fi")
                .disabled(off)
                .on_change(move |_, _, _| n.bump())
                .into_any_element()
        },
        space_acts: true,
    },
    Case {
        name: "RadioGroup",
        part: "r/radio-1",
        build: |n, off| {
            RadioGroup::new("r", "Fruit")
                .item("a", "Apple")
                .item("b", "Banana")
                .disabled(off)
                .on_change(move |_, _, _| n.bump())
                .into_any_element()
        },
        space_acts: true,
    },
    Case {
        name: "Tabs",
        part: "t/tab-1",
        build: |n, off| {
            Tabs::new("t", "Sections")
                .tab("a", "General", |_, _| Text::new("a"))
                .tab("b", "Display", |_, _| Text::new("b"))
                .disabled(off)
                .on_change(move |_, _, _| n.bump())
                .into_any_element()
        },
        // Selected as focus reaches it; Space is the key of `manual_activation` only.
        space_acts: false,
    },
    Case {
        name: "ToggleGroup",
        part: "g/item-1",
        build: |n, off| {
            ToggleGroup::new("g", "Align")
                .item("l", "Left")
                .item("r", "Right")
                .disabled(off)
                .on_change(move |_, _, _| n.bump())
                .into_any_element()
        },
        space_acts: true,
    },
    Case {
        name: "NavList",
        part: "n/item-1",
        build: |n, off| {
            NavList::new("n", "Settings")
                .item("a", "General")
                .item("b", "Display")
                .disabled(off)
                .on_change(move |_, _, _| n.bump())
                .into_any_element()
        },
        space_acts: true,
    },
];

fn centre(cx: &mut VisualTestContext, part: &'static str) -> gpui::Point<gpui::Pixels> {
    cx.debug_bounds(part)
        .unwrap_or_else(|| panic!("{part} was not drawn"))
        .center()
}

/// A place in the window with nothing to activate on it.
fn elsewhere(cx: &mut VisualTestContext) -> gpui::Point<gpui::Pixels> {
    let size = cx.update(|window, _| window.viewport_size());
    point(size.width - px(4.0), size.height - px(4.0))
}

/// Items 6, 10, 38: a click activates once; a press released somewhere else does not, and
/// leaves the control working; a disabled control ignores every click.
#[gpui::test]
fn a_press_activates_on_release_over_the_control_and_only_there(cx: &mut TestAppContext) {
    for case in &CASES {
        let n = Count::default();
        let build = case.build;
        let counter = n.clone();
        let cx = open(cx, move |_, _| build(counter.clone(), false));
        let (at, away) = (centre(cx, case.part), elsewhere(cx));

        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_up(away, MouseButton::Left, Modifiers::default());
        redraw(cx);
        assert_eq!(n.get(), 0, "{}: released elsewhere", case.name);

        cx.simulate_mouse_down(away, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::default());
        redraw(cx);
        assert_eq!(n.get(), 0, "{}: pressed elsewhere", case.name);

        cx.simulate_click(at, Modifiers::default());
        redraw(cx);
        assert_eq!(n.get(), 1, "{}: one click, one change", case.name);
    }
}

/// Item 6: a disabled control reports nothing to the pointer, and Tab goes past it, so the keys
/// that follow reach nothing.
#[gpui::test]
fn a_disabled_control_ignores_the_pointer_and_the_keyboard(cx: &mut TestAppContext) {
    for case in &CASES {
        let n = Count::default();
        let build = case.build;
        let counter = n.clone();
        let cx = open(cx, move |_, _| build(counter.clone(), true));
        let at = centre(cx, case.part);
        cx.simulate_click(at, Modifiers::default());
        redraw(cx);
        press(cx, "tab space enter right down end");
        assert_eq!(n.get(), 0, "{}", case.name);
    }
}

/// Items 10, 38: Space on a control that toggles acts once, when the key is released, and a key held
/// down (repeating) acts no more times. Tabs are left out: they are selected as focus reaches
/// them, and Space only selects with `manual_activation`.
#[gpui::test]
fn a_held_key_acts_once(cx: &mut TestAppContext) {
    for case in CASES.iter().filter(|c| c.space_acts) {
        let n = Count::default();
        let build = case.build;
        let counter = n.clone();
        let cx = open(cx, move |_, _| build(counter.clone(), false));
        press(cx, "tab");
        let space = Keystroke::parse("space").unwrap();
        for _ in 0..4 {
            let space = space.clone();
            cx.update(move |window, cx| {
                window.dispatch_keystroke(space, cx);
            });
        }
        cx.run_until_parked();
        assert_eq!(n.get(), 0, "{}: a key repeating does not act", case.name);
        cx.simulate_event(KeyUpEvent { keystroke: space });
        cx.run_until_parked();
        assert_eq!(n.get(), 1, "{}: it acts on release", case.name);
    }
}

/// Diagnostics can say which control has focus, by its id, as focus moves with Tab and as an
/// overlay takes it and gives it back; a control that has gone is forgotten.
#[cfg(debug_assertions)]
#[gpui::test]
fn the_focused_control_can_be_named(cx: &mut TestAppContext) {
    use std::cell::Cell;
    use std::rc::Rc;

    use crate::{Button, Dialog, focused_component};

    let shown = Rc::new(Cell::new(false));
    let s = shown.clone();
    let cx = open(cx, move |_, _| {
        crate::v_flex()
            .child(Button::new("first").label("First"))
            .child(Checkbox::new("second", "Second"))
            .child(
                Dialog::new("dlg", "Sure?")
                    .open(s.get())
                    .action(|_, _| Button::new("sure").label("Yes")),
            )
    });
    let focused = |cx: &mut VisualTestContext| {
        cx.update(|w, cx| focused_component(w, cx))
            .map(|n| n.to_string())
    };
    assert_eq!(focused(cx), None, "nothing has focus yet");
    press(cx, "tab");
    assert_eq!(focused(cx).as_deref(), Some("first/focus"));
    press(cx, "tab");
    assert_eq!(focused(cx).as_deref(), Some("second/focus"));
    shown.set(true);
    redraw(cx);
    cx.run_until_parked();
    let inside = focused(cx);
    assert!(
        inside
            .as_deref()
            .is_some_and(|n| n.starts_with("sure") || n.starts_with("dlg")),
        "the dialog took focus: {inside:?}"
    );
    shown.set(false);
    redraw(cx);
    cx.run_until_parked();
    assert_eq!(
        focused(cx).as_deref(),
        Some("second/focus"),
        "and gave it back"
    );
}

/// Item 13: an overlay whose trigger has gone while it was open closes without a panic, leaves
/// focus where GPUI puts it, and Tab still lands on a control that is there.
#[cfg(debug_assertions)]
#[gpui::test]
fn a_dialog_whose_opener_has_gone_closes_safely(cx: &mut TestAppContext) {
    use std::cell::Cell;
    use std::rc::Rc;

    use crate::{Button, Dialog, focused_component};

    let (shown, opener) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(true)));
    let (s, o) = (shown.clone(), opener.clone());
    let cx = open(cx, move |_, _| {
        let mut page = crate::v_flex();
        if o.get() {
            page = page.child(Button::new("opener").label("Open"));
        }
        page.child(Button::new("other").label("Other")).child(
            Dialog::new("dlg", "Sure?")
                .open(s.get())
                .action(|_, _| Button::new("sure").label("Yes")),
        )
    });
    let focused = |cx: &mut VisualTestContext| {
        cx.update(|w, cx| focused_component(w, cx))
            .map(|n| n.to_string())
    };
    press(cx, "tab");
    assert_eq!(focused(cx).as_deref(), Some("opener/focus"));
    shown.set(true);
    redraw(cx);
    cx.run_until_parked();
    // The button that opened it is taken away while the dialog is up, then the dialog closes.
    opener.set(false);
    shown.set(false);
    redraw(cx);
    cx.run_until_parked();
    cx.executor()
        .advance_clock(crate::foundation().motion.exit * 2);
    redraw(cx);
    cx.run_until_parked();
    press(cx, "tab");
    let now = focused(cx);
    assert!(
        now.as_deref().is_some_and(|n| n.starts_with("other")),
        "Tab reached a control that is there: {now:?}"
    );
}
