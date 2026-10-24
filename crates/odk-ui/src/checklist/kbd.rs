use gpui::{ParentElement, TestAppContext};
use odk_a11y::{Describe, Role};

use super::open;
use crate::{Kbd, v_flex};

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
        v_flex()
            .child(Kbd::new(""))
            .child(Kbd::new("+"))
            .child(Kbd::new("Ctrl++"))
            .child(Kbd::new("Ctrl+Alt+Shift+Super+Hyper+PageDown"))
            .child(Kbd::new("Strg+Ä"))
            .child(Kbd::keys(Vec::<&str>::new()))
    });
}
