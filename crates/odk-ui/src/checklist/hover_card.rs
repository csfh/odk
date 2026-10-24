use gpui::{ParentElement, TestAppContext};
use odk_a11y::{Describe, Role};

use super::{Count, open, press, redraw, reduce_motion};
use crate::{Button, HoverCard, Text, v_flex};

/// Items 8, 10, 13: focusing the trigger opens the card at once, Escape closes it, and the
/// card does not take focus: the next Tab goes past it.
#[gpui::test]
fn opens_on_focus_and_closes_on_escape(cx: &mut TestAppContext) {
    let (built, after) = (Count::default(), Count::default());
    let (b, a) = (built.clone(), after.clone());
    let cx = open(cx, move |_, _| {
        let (b, a) = (b.clone(), a.clone());
        v_flex()
            .child(
                HoverCard::new("h", "Details", Text::new("@omarchy")).content(move |_, _| {
                    b.bump();
                    Text::new("An Arch and Hyprland setup.")
                }),
            )
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| a.bump()),
            )
    });
    reduce_motion();
    assert_eq!(built.get(), 0, "closed until the trigger has focus");
    press(cx, "tab");
    redraw(cx);
    assert!(built.get() > 0, "focus opened it");
    press(cx, "escape");
    redraw(cx);
    let shown = built.get();
    redraw(cx);
    assert_eq!(built.get(), shown, "Escape closed it");
    press(cx, "tab enter");
    assert_eq!(after.get(), 1, "the card did not take focus");
}

/// Item 14.
#[test]
fn reports_a_tooltip() {
    let a = HoverCard::new("h", "About Omarchy", Text::new("t")).accessible();
    assert_eq!((a.role, a.name.as_str()), (Role::Tooltip, "About Omarchy"));
}

/// Item 15: the card is named by its constructor, so it cannot be left without one.
#[test]
fn is_named_by_its_constructor() {
    let a = HoverCard::new("h", "About Omarchy", Text::new("t")).accessible();
    assert_eq!(a.name, "About Omarchy");
}

/// The card sits right below its trigger, with the gap of its margin, and only the trigger's
/// own width opens it.
#[gpui::test]
fn the_card_sits_right_below_its_trigger(cx: &mut TestAppContext) {
    use gpui::px;
    let cx = open(cx, move |_, _| {
        v_flex().child(
            HoverCard::new("h", "About Omarchy", Text::new("@omarchy"))
                .content(|_, _| Text::new("An Arch and Hyprland setup.")),
        )
    });
    reduce_motion();
    press(cx, "tab");
    redraw(cx);
    let (trigger, card) = (
        cx.debug_bounds("h/trigger").unwrap(),
        cx.debug_bounds("h/card").unwrap(),
    );
    // GPUI rounds an anchored element to whole pixels; the trigger may end on a half.
    let below = trigger.bottom() + px(crate::foundation().spacing.s1);
    assert!(
        (card.top() - below).abs() <= px(0.5),
        "{card:?} below {trigger:?}"
    );
    assert_eq!(card.left(), trigger.left());
    assert!(
        trigger.size.width < px(400.0),
        "as wide as its text: {:?}",
        trigger.size
    );
}

/// Item 33: a hover card whose trigger has focus inside a dialog is drawn above the dialog.
#[gpui::test]
fn a_hover_card_in_a_dialog(cx: &mut TestAppContext) {
    use super::dialog::dialog_around;
    let cx = dialog_around(cx, Count::default(), || {
        HoverCard::new("h", "Details", Text::new("@omarchy"))
            .content(|_, _| Text::new("An Arch and Hyprland setup."))
    });
    let at = cx.debug_bounds("h/trigger").unwrap().center();
    cx.simulate_click(at, gpui::Modifiers::none());
    redraw(cx);
    assert!(cx.debug_bounds("h/card").is_some(), "the card is drawn");
}
