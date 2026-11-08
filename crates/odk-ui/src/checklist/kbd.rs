use gpui::{InteractiveElement, ParentElement, Styled, TestAppContext, div, px};
use odk_a11y::{Describe, Role};

use super::open;
use crate::{Kbd, Row, Stack};

/// Items 14, 15: a chord is reported as Keys, named by its keys.
#[test]
fn reports_its_keys() {
    let a = Kbd::new("Ctrl+Shift+K").accessible();
    assert_eq!((a.role, a.name.as_str()), (Role::Keys, "Ctrl+Shift+K"));
}

/// Item 23: odd chords render.
#[gpui::test]
fn renders_odd_chords(cx: &mut TestAppContext) {
    open(cx, |_, _| {
        Stack::new()
            .child(Kbd::new(""))
            .child(Kbd::new("+"))
            .child(Kbd::new("Ctrl++"))
            .child(Kbd::new("Ctrl+Alt+Shift+Super+Hyper+PageDown"))
            .child(Kbd::new("Strg+Ä"))
            .child(Kbd::keys(Vec::<&str>::new()))
    });
}

/// Item 23: a chord keeps its size, and a container sized by its content grows to hold it
/// instead of the keycaps running past it.
#[gpui::test]
fn a_content_sized_container_grows_to_the_chord(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Row::new().child(
            div()
                .min_w(px(40.0))
                .debug_selector(|| "box".into())
                .child(Kbd::new("Ctrl+Alt+Shift+Super+PageDown")),
        )
    });
    let boxed = cx.debug_bounds("box").unwrap();
    assert!(
        boxed.size.width > px(40.0),
        "the container did not grow to hold the chord: {boxed:?}"
    );
}
