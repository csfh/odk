//! The Interaction page's event log says what happened and nothing else: the key that made a
//! click, the focus that moved, the change a control reported, and not a line for a control that
//! was disabled or busy. Each test drives the page with real keys and clicks on GPUI's test
//! platform and reads the log back.

use std::time::Duration;

use gallery::{Gallery, Page};
use odk::gpui::{
    AppContext as _, Entity, KeyUpEvent, Keystroke, Modifiers, TestAppContext, VisualTestContext,
};
use odk::theme::foundation::FoundationConfig;
use odk::ui::test_support::open_view;

static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn open(app: &mut TestAppContext) -> (Entity<Gallery>, &mut VisualTestContext) {
    let (gallery, cx) = open_view(app, |_, cx| cx.new(Gallery::new));
    cx.update(|_, cx| {
        odk::ui::configure(cx, |c| {
            *c = FoundationConfig {
                reduced_motion: true,
                ..Default::default()
            }
        })
    });
    gallery.update(cx, |g, cx| g.show(Page::Interaction, cx));
    cx.run_until_parked();
    (gallery, cx)
}

/// A key pressed and released, as a keyboard does: a key click is made on the release.
fn press(cx: &mut VisualTestContext, keys: &str) {
    for key in keys.split(' ') {
        let keystroke = Keystroke::parse(key).unwrap();
        cx.update(|window, cx| {
            window.dispatch_keystroke(keystroke.clone(), cx);
        });
        cx.simulate_event(KeyUpEvent { keystroke });
        cx.run_until_parked();
    }
}

fn log(gallery: &Entity<Gallery>, cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| gallery.read(cx).interaction_log(cx))
}

/// Tab until focus is on `name`, which is some stops in: the header's controls come first.
fn tab_to(gallery: &Entity<Gallery>, cx: &mut VisualTestContext, name: &str) {
    for _ in 0..12 {
        press(cx, "tab");
        if log(gallery, cx)
            .last()
            .is_some_and(|l| l.starts_with("focus moved") && l.ends_with(&format!("to {name}")))
        {
            return;
        }
    }
    panic!("Tab never reached {name}: {:?}", log(gallery, cx));
}

fn has(lines: &[String], text: &str) -> bool {
    lines.iter().any(|l| l == text)
}

/// Enter and Space on the Save button are logged by the key that made them; Tab is logged as the move
/// of focus it made; a click is logged as the pointer. The Disabled button
/// is skipped by Tab and never logs.
#[test]
fn the_log_names_the_key_or_pointer_that_acted() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app);
    tab_to(&gallery, cx, "ix-save");
    let lines = log(&gallery, cx);
    // Tab is a bound action, which GPUI does not deliver as a key; its move of focus is logged.
    assert!(
        lines.iter().any(|l| l.starts_with("focus moved from")),
        "{lines:?}"
    );
    assert!(lines.iter().all(|l| l != "key tab"), "{lines:?}");
    press(cx, "enter space");
    let lines = log(&gallery, cx);
    assert!(has(&lines, "key enter"), "{lines:?}");
    assert!(has(&lines, "Save activated using Enter"), "{lines:?}");
    assert!(has(&lines, "Save activated using Space"), "{lines:?}");

    let save = cx.debug_bounds("ix-save/button").unwrap();
    cx.simulate_click(save.center(), Modifiers::default());
    assert!(has(&log(&gallery, cx), "Save activated using the pointer"));

    // Past Start work, the disabled button is not a stop: the next stop is the close button.
    press(cx, "tab tab");
    let lines = log(&gallery, cx);
    assert!(
        lines.iter().any(|l| l.ends_with("to ix-close")),
        "Tab went past the disabled button: {lines:?}"
    );
    assert!(
        lines.iter().all(|l| !l.ends_with("to ix-disabled")),
        "{lines:?}"
    );
    press(cx, "enter space");
    assert!(
        log(&gallery, cx)
            .iter()
            .all(|l| !l.contains("disabled button"))
    );
}

/// While the work button is busy a press is not an activation: nothing is logged and the count
/// does not move. When the work is done the button works again.
#[test]
fn a_busy_button_logs_nothing_until_it_is_done() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app);
    tab_to(&gallery, cx, "ix-work");
    press(cx, "enter");
    let started = |g: &Entity<Gallery>, cx: &mut VisualTestContext| {
        log(g, cx)
            .iter()
            .filter(|l| l.starts_with("Work started"))
            .count()
    };
    assert_eq!(started(&gallery, cx), 1);
    press(cx, "enter space enter");
    let work = cx.debug_bounds("ix-work/button").unwrap();
    cx.simulate_click(work.center(), Modifiers::default());
    assert_eq!(
        started(&gallery, cx),
        1,
        "a busy button ignores every press"
    );

    cx.executor().advance_clock(Duration::from_secs(3));
    cx.run_until_parked();
    assert!(
        has(
            &log(&gallery, cx),
            "Work finished; the button is live again"
        ),
        "{:?}",
        log(&gallery, cx)
    );
    press(cx, "enter");
    assert_eq!(started(&gallery, cx), 2);
}

/// A dialog opens by a key, takes focus, is left by Escape, and gives focus back to the button
/// that opened it: each step is a line in the log, in that order.
#[test]
fn a_dialog_takes_focus_and_gives_it_back() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app);
    tab_to(&gallery, cx, "ix-save");
    let opener = cx.debug_bounds("ix-open-dialog/button").unwrap();
    cx.simulate_click(opener.center(), Modifiers::default());
    let lines = log(&gallery, cx);
    assert!(has(&lines, "Dialog opened using the pointer"), "{lines:?}");
    assert!(
        lines
            .last()
            .is_some_and(|l| l.contains("ix-dialog") || l.contains("ix-dialog-keep")),
        "focus moved into the dialog: {lines:?}"
    );
    press(cx, "tab tab tab tab tab");
    let inside: Vec<_> = log(&gallery, cx)
        .into_iter()
        .filter(|l| l.starts_with("focus moved"))
        .collect();
    assert!(
        inside.iter().rev().take(5).all(|l| l.contains("ix-dialog")),
        "Tab stayed inside the dialog: {inside:?}"
    );
    press(cx, "escape");
    let lines = log(&gallery, cx);
    let at = |text: &str| lines.iter().position(|l| l.starts_with(text));
    let closed = at("Dialog closed").expect("closed");
    let back = lines
        .iter()
        .rposition(|l| l.starts_with("focus moved") && l.ends_with("to ix-open-dialog"))
        .expect("focus came back to the opener");
    assert!(
        closed < back,
        "focus came back after the dialog closed: {lines:?}"
    );
}
