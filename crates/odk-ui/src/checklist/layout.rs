//! The layout helpers (`layout.rs`): `sidebar_frame`, `content_column` and `scroll_y`. They are not
//! components, so no checklist item applies to them as such; these tests check that the sizes they
//! lay out are the foundation's (item 2) and that they behave in a window of any size.

use gpui::{InteractiveElement, ParentElement, Styled, TestAppContext, div, px, size};

use super::open;
use crate::{content_column, foundation, scroll_y, sidebar_frame};

fn resize(cx: &mut gpui::VisualTestContext, width: f32, height: f32) {
    cx.simulate_resize(size(px(width), px(height)));
    cx.run_until_parked();
}

/// The sidebar is the foundation's width, the content takes the rest, and both fill the height.
#[gpui::test]
fn the_sidebar_has_its_token_width_and_the_content_takes_the_rest(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        sidebar_frame(
            div().debug_selector(|| "side".into()).size_full(),
            div().debug_selector(|| "main".into()).size_full(),
        )
    });
    resize(cx, 1200.0, 700.0);
    let (side, main) = (
        cx.debug_bounds("side").unwrap(),
        cx.debug_bounds("main").unwrap(),
    );
    let want = foundation().layout.sidebar_width;
    assert_eq!(
        f32::from(side.size.width),
        want,
        "the sidebar's width is the token"
    );
    assert_eq!(f32::from(side.origin.x), 0.0);
    assert!(
        main.origin.x >= side.right(),
        "the content starts after the sidebar"
    );
    assert_eq!(
        f32::from(main.right()),
        1200.0,
        "the content reaches the window's edge"
    );
    assert_eq!(
        side.size.height, main.size.height,
        "both are as tall as each other"
    );
    assert!(
        f32::from(main.size.height) > 600.0,
        "and fill the window's height"
    );
}

/// A narrow window shrinks the content and never the sidebar.
#[gpui::test]
fn a_narrow_window_shrinks_the_content_not_the_sidebar(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        sidebar_frame(
            div().debug_selector(|| "side".into()).size_full(),
            div().debug_selector(|| "main".into()).size_full(),
        )
    });
    resize(cx, 500.0, 400.0);
    let side = cx.debug_bounds("side").unwrap();
    assert_eq!(
        f32::from(side.size.width),
        foundation().layout.sidebar_width
    );
    assert!(cx.debug_bounds("main").unwrap().size.width > px(0.0));
}

/// A wide window centres a column of the foundation's content width; a narrow one fills it.
#[gpui::test]
fn the_column_is_centred_up_to_its_width_and_fills_a_narrow_window(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| content_column().debug_selector(|| "col".into()));
    let width = foundation().layout.content_width;
    resize(cx, 2400.0, 600.0);
    let col = cx.debug_bounds("col").unwrap();
    assert_eq!(f32::from(col.size.width), width);
    assert_eq!(f32::from(col.origin.x), (2400.0 - width) / 2.0, "centred");
    resize(cx, 600.0, 600.0);
    let col = cx.debug_bounds("col").unwrap();
    assert_eq!(
        f32::from(col.size.width),
        600.0,
        "a narrow window is filled"
    );
    assert_eq!(f32::from(col.origin.x), 0.0);
}

/// The column pads its content by the panel padding.
#[gpui::test]
fn the_column_pads_its_content_by_the_panel_padding(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        content_column()
            .debug_selector(|| "col".into())
            .child(div().debug_selector(|| "kid".into()).h(px(10.0)))
    });
    resize(cx, 600.0, 600.0);
    let (col, kid) = (
        cx.debug_bounds("col").unwrap(),
        cx.debug_bounds("kid").unwrap(),
    );
    let pad = foundation().layout.panel_padding;
    assert_eq!(f32::from(kid.origin.x - col.origin.x), pad);
    assert_eq!(f32::from(kid.origin.y - col.origin.y), pad);
}

/// A scroll region takes the room it is given and no more, however tall its content is.
#[gpui::test]
fn a_scroll_region_does_not_grow_with_its_content(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        scroll_y("scroll")
            .debug_selector(|| "scroll".into())
            .child(div().debug_selector(|| "tall".into()).h(px(5000.0)))
    });
    resize(cx, 800.0, 400.0);
    let region = cx.debug_bounds("scroll").unwrap();
    assert!(
        f32::from(region.size.height) <= 400.0,
        "the region is {:?} high in a window 400 high",
        region.size.height
    );
    assert_eq!(
        f32::from(cx.debug_bounds("tall").unwrap().size.height),
        5000.0
    );
}

/// The three together, as an application puts them: a column inside a scroll region inside the
/// frame. A wide window centres the column in the content area, not in the window, and a tall
/// column scrolls in place instead of growing the frame.
#[gpui::test]
fn the_column_is_centred_in_the_content_area_of_the_frame(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        sidebar_frame(
            div().debug_selector(|| "side".into()).size_full(),
            scroll_y("page").child(
                content_column()
                    .debug_selector(|| "col".into())
                    .child(div().h(px(5000.0))),
            ),
        )
    });
    resize(cx, 2600.0, 700.0);
    let side = cx.debug_bounds("side").unwrap();
    let col = cx.debug_bounds("col").unwrap();
    let width = foundation().layout.content_width;
    assert_eq!(f32::from(col.size.width), width);
    // The content area is what is left of the window after the sidebar and its line.
    let area_left = f32::from(side.right()) + 1.0;
    let area = 2600.0 - area_left;
    assert_eq!(
        f32::from(col.origin.x),
        area_left + (area - width) / 2.0,
        "centred in the content area"
    );
    assert!(
        f32::from(side.size.height) <= 700.0,
        "a tall page scrolls and does not grow the frame"
    );
}

/// A field that fills its row still fills it when a long paragraph of text sits above it in a
/// scroll region. A scroll region without a width of its own was measured at the width of its
/// content, and a `SearchInput` under a wrapping `Text` came out as wide as its icon.
#[gpui::test]
fn a_field_fills_its_row_under_a_long_paragraph_in_a_scroll_region(cx: &mut TestAppContext) {
    const LONG: &str = "Processes that stay busy, hold a lot of memory, or keep a port open for days. Linux does not record when a socket opened, so a port's age is its process's age. Ports owned by root services show without a process.";
    let cx = open(cx, |_, _| {
        // As an application puts it: the page in a scroll region in the content of the frame.
        sidebar_frame(
            div().size_full(),
            scroll_y("page").child(
                content_column()
                    .child(crate::Text::new(LONG))
                    .child(crate::SearchInput::new("find", "Find").placeholder("Find")),
            ),
        )
    });
    resize(cx, 1400.0, 700.0);
    let width = foundation().layout.content_width - 2.0 * foundation().layout.panel_padding;
    assert_eq!(
        f32::from(cx.debug_bounds("find/box").unwrap().size.width),
        width,
        "the field is the column's width inside its padding"
    );
}
