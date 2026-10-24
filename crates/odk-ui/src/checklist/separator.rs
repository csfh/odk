use gpui::{ParentElement, Styled, TestAppContext, px};
use odk_a11y::{Describe, Role};

use super::{open, redraw};
use crate::{Separator, Text, h_flex, v_flex};

/// Item 14: a separator reports itself as one, with no name, as separators have none.
#[test]
fn reports_a_separator() {
    let a = Separator::horizontal().accessible();
    assert_eq!((a.role, a.name.as_str()), (Role::Separator, ""));
}

/// A horizontal separator spans its column and is one border width thick.
#[gpui::test]
fn a_horizontal_separator_spans_its_column(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .w(px(300.0))
            .child(Text::new("Above"))
            .child(Separator::horizontal())
            .child(Text::new("Below"))
    });
    let line = cx.debug_bounds("odk-separator").unwrap();
    assert_eq!(line.size.width, px(300.0));
    assert_eq!(
        line.size.height,
        px(crate::components().separator().thickness)
    );
}

/// A vertical separator spans the height of its row and is one border width thick.
#[gpui::test]
fn a_vertical_separator_spans_its_row(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        h_flex()
            .h(px(40.0))
            .child(Text::new("Left"))
            .child(Separator::vertical())
            .child(Text::new("Right"))
    });
    let line = cx.debug_bounds("odk-separator").unwrap();
    assert_eq!(line.size.height, px(40.0));
    assert_eq!(
        line.size.width,
        px(crate::components().separator().thickness)
    );
}

/// Item 23: a column too short for its content does not squeeze the line away.
#[gpui::test]
fn a_squeezed_column_keeps_the_line(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .h(px(100.0))
            .child(Separator::horizontal())
            .child(gpui::div().h(px(4000.0)))
    });
    redraw(cx);
    let line = cx.debug_bounds("odk-separator").unwrap();
    assert_eq!(
        line.size.height,
        px(crate::components().separator().thickness)
    );
}

/// Items 1, 34: a separator is the subtle border colour, quieter than a control's edge, one
/// border width thick; an inset one stops a control's padding short of each end.
#[gpui::test]
fn a_separator_is_subtle_and_can_be_inset(cx: &mut TestAppContext) {
    use odk_theme::foundation::Density;
    let cx = open(cx, |_, _| {
        v_flex().w(px(300.0)).child(Separator::horizontal().inset())
    });
    let s = crate::drawn::last::<odk_theme::component::SeparatorStyle>("odk-separator");
    let c = crate::components();
    assert_eq!(s.colour, c.semantic().border.subtle);
    assert_eq!(s.thickness, c.foundation().geometry.border_width);
    let line = cx.debug_bounds("odk-separator").unwrap();
    let inset = Density::Default.metrics().padding_x;
    assert_eq!(line.size.width, px(300.0 - 2.0 * inset));
    assert_eq!(line.left(), px(inset));
}
