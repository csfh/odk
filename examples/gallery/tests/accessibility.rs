//! The accessibility page's controls are the real ones, driven with the keyboard, and its log
//! says what happened: the key that acted, the value a control reported, the focus that moved.
//! These tests press keys and read the log back; the page's focus-restoration tests are in
//! `interaction_log.rs`.

mod common;

use std::sync::Arc;

use common::{open, press, serial, settle, tab_to};
use gallery::{Gallery, Page};
use odk::gpui::{Entity, TestAppContext, VisualTestContext};
use odk::theme::ThemeSnapshot;

fn log(gallery: &Entity<Gallery>, cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| gallery.read(cx).interaction_log(cx))
}

fn has(lines: &[String], text: &str) -> bool {
    lines.iter().any(|l| l == text)
}

/// Space toggles a check box and the log says which way; Space on a switch the same.
#[test]
fn space_toggles_a_check_box_and_a_switch_and_the_log_says_so() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Interaction);
    tab_to(cx, "ix-check");
    press(cx, "space");
    assert!(
        has(&log(&gallery, cx), "Checkbox changed to checked"),
        "{:?}",
        log(&gallery, cx)
    );
    press(cx, "space");
    assert!(has(&log(&gallery, cx), "Checkbox changed to unchecked"));
    tab_to(cx, "ix-switch");
    press(cx, "space");
    assert!(
        has(&log(&gallery, cx), "Switch changed to on"),
        "{:?}",
        log(&gallery, cx)
    );
}

/// A radio group is one Tab stop, and the arrow keys move inside it and choose.
#[test]
fn arrow_keys_choose_inside_a_radio_group_and_a_tab_list() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Interaction);
    tab_to(cx, "ix-radio");
    press(cx, "down");
    let lines = log(&gallery, cx);
    assert!(
        lines.iter().any(|l| l.starts_with("Radio group chose")),
        "{lines:?}"
    );
    // One stop: the next Tab leaves the group.
    press(cx, "tab");
    assert!(
        !common::focused(cx).starts_with("ix-radio"),
        "{}",
        common::focused(cx)
    );

    tab_to(cx, "ix-tabs");
    press(cx, "right");
    assert!(
        log(&gallery, cx)
            .iter()
            .any(|l| l.starts_with("Tab ") && l.ends_with("selected")),
        "{:?}",
        log(&gallery, cx)
    );
}

/// A menu opens from the keyboard, moves with the arrow keys, chooses with Enter and is
/// closed by Escape, with focus back on the trigger; a disabled item is never chosen.
#[test]
fn a_menu_is_driven_by_the_keyboard() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Interaction);
    tab_to(cx, "ix-menu");
    press(cx, "enter down enter");
    let lines = log(&gallery, cx);
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("Menu item ") && l.ends_with(" chosen")),
        "{lines:?}"
    );
    assert!(lines.iter().all(|l| !l.contains("disabled: a bug")));
    assert!(
        common::focused(cx).starts_with("ix-menu"),
        "focus returned to {}",
        common::focused(cx)
    );
    press(cx, "enter escape");
    assert!(
        common::focused(cx).starts_with("ix-menu"),
        "after Escape: {}",
        common::focused(cx)
    );
}

/// The gallery's Reduced switch makes every transition zero, and the system's request
/// stays on when the gallery's switch is off: the toolbar then says whose it is.
#[test]
fn reduced_motion_follows_the_switch_and_the_users_request() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Motion);
    common::click(cx, "motion/item-0");
    assert!(!odk::ui::foundation().motion.reduced);
    assert!(
        odk::ui::foundation()
            .motion
            .transition(odk::theme::foundation::MotionDuration::Normal)
            > std::time::Duration::ZERO
    );
    common::click(cx, "motion/item-1");
    assert!(odk::ui::foundation().motion.reduced);
    assert_eq!(
        odk::ui::foundation()
            .motion
            .transition(odk::theme::foundation::MotionDuration::Normal),
        std::time::Duration::ZERO
    );
    assert!(!gallery.read_with(cx, |g, _| g.motion_forced_by_user()));

    // The user asks, through the compositor (ADR 0048): the followed theme is published
    // carrying the request.
    let asked = Arc::new(ThemeSnapshot {
        reduced_motion: true,
        ..(*odk::ui::theme_snapshot()).clone()
    });
    odk::ui::test_support::publish_followed(asked);
    cx.update(|_, cx| cx.refresh_windows());
    settle(cx);
    assert!(gallery.read_with(cx, |g, _| g.motion_forced_by_user()));
    cx.update(|_, cx| odk::ui::set_reduced_motion(cx, false));
    assert!(
        odk::ui::foundation().motion.reduced,
        "the gallery cannot take the user's request back"
    );
    // Taking the user's request back is the user's.
    let relaxed = Arc::new(ThemeSnapshot {
        reduced_motion: false,
        ..(*odk::ui::theme_snapshot()).clone()
    });
    odk::ui::test_support::publish_followed(relaxed);
    cx.update(|_, cx| cx.refresh_windows());
    assert!(!gallery.read_with(cx, |g, _| g.motion_forced_by_user()));
    assert!(!odk::ui::foundation().motion.reduced);
}

/// The motion page's Play and Reset move the real components to their other end and back; with
/// motion reduced they get there at once, and nothing is left asking for frames.
#[test]
fn play_and_reset_move_the_real_components() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (_, cx) = open(&mut app, Page::Motion);
    let thumb =
        |cx: &mut VisualTestContext| cx.debug_bounds("motion-switch/thumb").unwrap().origin.x;
    let before = thumb(cx);
    common::click(cx, "motion-play/button");
    let played = thumb(cx);
    assert!(
        played > before,
        "the switch moved from {before:?} to {played:?}"
    );
    common::click(cx, "motion-reset/button");
    assert_eq!(thumb(cx), before);
    odk::ui::test_support::take_frame_requests();
    odk::ui::test_support::redraw(cx);
    assert_eq!(odk::ui::test_support::take_frame_requests(), 0);
}

/// Status is never colour alone: each tone has a mark and the marks differ.
#[test]
fn every_status_tone_has_its_own_mark() {
    use odk::theme::component::status_mark;
    use odk::theme::semantic::Role;
    let marks: Vec<_> = [Role::Info, Role::Success, Role::Warning, Role::Danger]
        .into_iter()
        .map(|r| status_mark(r).expect("a status role has a mark"))
        .collect();
    for (n, m) in marks.iter().enumerate() {
        assert!(!m.is_empty() && !marks[..n].contains(m), "{marks:?}");
    }
}
