//! Tests for the component checklist (`docs/spec/component-checklist.md`). Each test names the
//! checklist items it checks. `source` scans the crate's code for the rules every component
//! shares; the other modules drive one component each on GPUI's headless test platform.
//!
//! A known failure is a test marked `#[ignore = "known gap: <item>"]` with the matching row in
//! the checklist's "Known gaps" table. `cargo test -p odk-ui -- --ignored` runs them; when one
//! starts passing, drop the `ignore` and the row.

mod alert;
mod badge;
mod breadcrumb;
mod button;
mod checkbox;
mod command;
mod content;
mod dialog;
mod form_field;
mod forms;
mod hover_card;
mod input;
mod input_group;
mod interaction;
mod kbd;
mod menu;
mod motion;
mod nav_list;
mod navigation;
mod number_input;
mod overlays;
mod password_input;
mod performance;
mod popover;
mod progress;
mod radio;
mod search_input;
mod select;
mod separator;
mod sheet;
mod skeleton;
mod source;
mod spinner;
mod stability;
mod status;
mod switch;
mod tabs;
mod text;
mod textarea;
mod toast;
mod toggle_group;
mod tooltip;

use std::cell::Cell;
use std::rc::Rc;

use gpui::{KeyUpEvent, Keystroke, VisualTestContext};

pub(crate) use crate::test_support::{open, redraw};

/// Presses each space-separated key, down and up, the way a keyboard does. GPUI's own
/// `simulate_keystrokes` sends only the key-down, and keyboard clicks fire on the key-up.
pub(crate) fn press(cx: &mut VisualTestContext, keys: &str) {
    for key in keys.split(' ') {
        let keystroke = Keystroke::parse(key).unwrap();
        cx.update(|window, cx| {
            window.dispatch_keystroke(keystroke.clone(), cx);
        });
        cx.simulate_event(KeyUpEvent { keystroke });
        cx.run_until_parked();
    }
}

/// A counter shared between a test and the handlers it installs.
#[derive(Clone, Default)]
pub(crate) struct Count(Rc<Cell<usize>>);

impl Count {
    pub(crate) fn bump(&self) {
        self.0.set(self.0.get() + 1);
    }
    pub(crate) fn get(&self) -> usize {
        self.0.get()
    }
}

/// Reduced motion, for this thread only, through the same path as `configure`, so the
/// foundation tokens and the test theme agree.
pub(crate) fn reduce_motion() {
    crate::theme::apply(&odk_theme::foundation::FoundationConfig {
        reduced_motion: true,
        ..Default::default()
    });
    if crate::theme::current().is_none() {
        crate::theme::install_test_theme();
    }
}

/// The fixtures `odk-theme` tests the semantic layer with: three dark themes, a monochrome one
/// and a light one.
pub(crate) const FIXTURES: [(&str, &str); 5] = [
    (
        "tokyo-night",
        include_str!("../../../odk-theme/tests/fixtures/tokyo-night/colors.toml"),
    ),
    (
        "nord",
        include_str!("../../../odk-theme/tests/fixtures/nord/colors.toml"),
    ),
    (
        "catppuccin",
        include_str!("../../../odk-theme/tests/fixtures/catppuccin/colors.toml"),
    ),
    (
        "vantablack",
        include_str!("../../../odk-theme/tests/fixtures/vantablack/colors.toml"),
    ),
    (
        "catppuccin-latte",
        include_str!("../../../odk-theme/tests/fixtures/catppuccin-latte/colors.toml"),
    ),
];

/// Publishes a fixture's theme, as a reload would.
pub(crate) fn publish_fixture(colors: &str) {
    crate::theme::publish(std::sync::Arc::new(
        odk_theme::ThemeSnapshot::from_toml(colors, None).unwrap(),
    ));
}

/// Asserts that the checklist's "Known gaps" table has a row for `component` failing `item`,
/// so an exception made in a test is always written down in the spec.
pub(crate) fn assert_known_gap(component: &str, item: u32) {
    let spec = include_str!("../../../../docs/spec/component-checklist.md");
    let gaps = &spec[spec
        .find("## Known gaps")
        .expect("the checklist has a Known gaps table")..];
    let found = gaps.lines().filter(|l| l.starts_with('|')).any(|row| {
        let cells: Vec<&str> = row.split('|').map(str::trim).collect();
        cells.len() > 2
            && (cells[1].contains(&format!("`{component}`")) || cells[1] == component)
            && cells[2]
                .split(',')
                .any(|n| n.trim().parse::<u32>() == Ok(item))
    });
    assert!(
        found,
        "{component} is excused from checklist item {item} but has no row for it in Known gaps"
    );
}
