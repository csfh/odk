//! Item 32: no state change moves anything. A test lists the parts it measures (by the names
//! components give them with `part_name`, plus any neighbours the test tags itself), takes
//! their bounds at rest, then goes through states and checks after each that every bound is
//! exactly as it was.

use gpui::{Bounds, Modifiers, MouseButton, Pixels, VisualTestContext, point, px};

use super::{press, redraw};

/// One thing done to the component before the bounds are measured again.
pub(crate) enum Step {
    /// Keys, as for [`press`].
    Keys(&'static str),
    /// The pointer moves to the middle of the part.
    Hover(&'static str),
    /// The left button goes down on the part and stays down while the bounds are measured,
    /// then comes up again.
    Press(&'static str),
    /// Something the test changes, such as a prop, named for the failure message.
    Change(&'static str, Box<dyn Fn()>),
}

fn bounds_of(cx: &mut VisualTestContext, part: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(part)
        .unwrap_or_else(|| panic!("no part named {part:?} was drawn"))
}

fn measure(cx: &mut VisualTestContext, parts: &[&'static str]) -> Vec<Bounds<Pixels>> {
    redraw(cx);
    parts.iter().map(|p| bounds_of(cx, p)).collect()
}

fn centre(b: Bounds<Pixels>) -> gpui::Point<Pixels> {
    point(
        b.origin.x + b.size.width / 2.0,
        b.origin.y + b.size.height / 2.0,
    )
}

/// Asserts that no step moves or resizes any of `parts`, naming the step and the part that
/// moved when one does.
pub(crate) fn assert_still(cx: &mut VisualTestContext, parts: &[&'static str], steps: &[Step]) {
    let rest = measure(cx, parts);
    let mut moved = Vec::new();
    for step in steps {
        let (what, now) = match step {
            Step::Keys(keys) => {
                press(cx, keys);
                (format!("after keys {keys:?}"), measure(cx, parts))
            }
            Step::Hover(part) => {
                let at = centre(bounds_of(cx, part));
                cx.simulate_mouse_move(at, None, Modifiers::default());
                (format!("hovering {part}"), measure(cx, parts))
            }
            Step::Change(name, change) => {
                change();
                (format!("after {name}"), measure(cx, parts))
            }
            Step::Press(part) => {
                let at = centre(bounds_of(cx, part));
                cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::default());
                let now = measure(cx, parts);
                cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::default());
                (format!("pressing {part}"), now)
            }
        };
        for ((part, before), after) in parts.iter().zip(&rest).zip(&now) {
            if before != after {
                moved.push(format!("{part} moved {what}: {before:?} became {after:?}"));
            }
        }
    }
    // Leave the pointer somewhere harmless for whatever the test does next.
    cx.simulate_mouse_move(point(px(-10.0), px(-10.0)), None, Modifiers::default());
    assert!(
        moved.is_empty(),
        "{} shifts:\n{}",
        moved.len(),
        moved.join("\n")
    );
}
