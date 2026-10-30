//! The layout helpers (`layout.rs`, `grid.rs`): `sidebar_frame`, `content_column`, `scroll_y`,
//! `Grid` and `content_column_for_grid`. They are not components, so no checklist item applies to
//! them as such; these tests check that the sizes they lay out are the foundation's (item 2) and
//! that they behave in a window of any size.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{InteractiveElement, ParentElement, Styled, TestAppContext, div, px, size};

use super::open;
use crate::{
    Grid, Item, Select, content_column, content_column_for_grid, foundation, scroll_y,
    sidebar_frame,
};

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
        cx.debug_bounds("sidebar-frame").unwrap(),
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
    let side = cx.debug_bounds("sidebar-frame").unwrap();
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
    let side = cx.debug_bounds("sidebar-frame").unwrap();
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

/// What the sidebar holds is inset from its edges by the foundation's sidebar padding, so a
/// row's fill and the current page's indicator stand clear of the window's edge and the line.
#[gpui::test]
fn the_sidebars_content_is_inset_by_its_padding(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        sidebar_frame(
            div().debug_selector(|| "side".into()).size_full(),
            div().size_full(),
        )
    });
    resize(cx, 1200.0, 700.0);
    let (frame, side) = (
        cx.debug_bounds("sidebar-frame").unwrap(),
        cx.debug_bounds("side").unwrap(),
    );
    let pad = px(foundation().layout.sidebar_padding);
    assert!(pad > px(0.0));
    assert_eq!(side.left() - frame.left(), pad, "left");
    assert_eq!(frame.right() - side.right(), pad, "right");
    assert_eq!(side.top() - frame.top(), pad, "top");
    assert_eq!(frame.bottom() - side.bottom(), pad, "bottom");
}

/// A page of `cells` cells and the `wide` children after them, as an application puts a page of
/// groups: the column for a grid in a scroll region, the grid in the column under a heading.
fn grid_page(
    cells: usize,
    wide: usize,
) -> impl Fn(&mut gpui::Window, &mut gpui::App) -> gpui::Stateful<gpui::Div> {
    move |window, cx| {
        let column = content_column_for_grid("page-column", 2, cells, window, cx);
        let grid = (0..wide).fold(
            Grid::new(column.columns()).children((0..cells).map(|i| {
                div()
                    .debug_selector(move || format!("cell-{i}"))
                    .h(px(40.0 + 10.0 * i as f32))
            })),
            |grid, i| {
                grid.wide(
                    div()
                        .debug_selector(move || format!("wide-{i}"))
                        .h(px(30.0)),
                )
            },
        );
        scroll_y("page").child(
            column
                .child(div().debug_selector(|| "heading".into()).h(px(20.0)))
                .child(grid),
        )
    }
}

/// The column grows to the wide content width only once two of the grid's columns fit beside
/// each other: one pixel short and it is the content column, with one column of cells.
#[gpui::test]
fn the_column_grows_for_two_columns_only_once_they_fit(cx: &mut TestAppContext) {
    let page = grid_page(2, 0);
    let cx = open(cx, move |window, cx| page(window, cx));
    let layout = foundation().layout.clone();
    let pad = layout.panel_padding;
    // Two columns, the gap and the padding on both sides.
    let fits = 2.0 * layout.grid_column_width + layout.section_gap + 2.0 * pad;

    resize(cx, fits - 1.0, 700.0);
    let (heading, a, b) = (
        cx.debug_bounds("heading").unwrap(),
        cx.debug_bounds("cell-0").unwrap(),
        cx.debug_bounds("cell-1").unwrap(),
    );
    assert_eq!(f32::from(a.size.width), layout.content_width - 2.0 * pad);
    assert_eq!(
        a.origin.x, heading.origin.x,
        "the cells line up with the heading"
    );
    assert!(b.top() > a.bottom(), "one column: {a:?} then {b:?}");

    resize(cx, fits, 700.0);
    let (heading, a, b) = (
        cx.debug_bounds("heading").unwrap(),
        cx.debug_bounds("cell-0").unwrap(),
        cx.debug_bounds("cell-1").unwrap(),
    );
    assert_eq!(
        f32::from(heading.size.width),
        fits - 2.0 * pad,
        "the column grew"
    );
    assert_eq!(a.top(), b.top(), "two columns: {a:?} beside {b:?}");
    assert_eq!(f32::from(a.size.width), layout.grid_column_width);
    assert_eq!(f32::from(b.left() - a.right()), layout.section_gap);
    assert_eq!(a.origin.x, heading.origin.x);

    resize(cx, 2400.0, 700.0);
    let heading = cx.debug_bounds("heading").unwrap();
    assert_eq!(
        f32::from(heading.size.width),
        layout.wide_content_width - 2.0 * pad,
        "up to the wide content width"
    );
    assert_eq!(
        f32::from(heading.origin.x),
        (2400.0 - layout.wide_content_width) / 2.0 + pad,
        "centred"
    );

    resize(cx, fits - 1.0, 700.0);
    let (a, b) = (
        cx.debug_bounds("cell-0").unwrap(),
        cx.debug_bounds("cell-1").unwrap(),
    );
    assert!(b.top() > a.bottom(), "and one column again when it narrows");
}

/// A page with one cell keeps the content width however wide the window is.
#[gpui::test]
fn a_page_with_one_cell_stays_at_the_content_width(cx: &mut TestAppContext) {
    let page = grid_page(1, 1);
    let cx = open(cx, move |window, cx| page(window, cx));
    resize(cx, 2400.0, 700.0);
    let layout = foundation().layout.clone();
    let width = layout.content_width - 2.0 * layout.panel_padding;
    assert_eq!(
        f32::from(cx.debug_bounds("heading").unwrap().size.width),
        width
    );
    assert_eq!(
        f32::from(cx.debug_bounds("cell-0").unwrap().size.width),
        width
    );
    assert_eq!(
        f32::from(cx.debug_bounds("wide-0").unwrap().size.width),
        width
    );
}

/// A wide child takes a row of its own across every column, after the cells before it, and a
/// row is as tall as its tallest cell.
#[gpui::test]
fn a_wide_child_takes_a_row_across_every_column(cx: &mut TestAppContext) {
    let page = grid_page(3, 1);
    let cx = open(cx, move |window, cx| page(window, cx));
    resize(cx, 2400.0, 700.0);
    let mut cell = |n: &'static str| cx.debug_bounds(n).unwrap();
    let (heading, a, b, c, wide) = (
        cell("heading"),
        cell("cell-0"),
        cell("cell-1"),
        cell("cell-2"),
        cell("wide-0"),
    );
    assert_eq!(a.top(), b.top());
    assert_eq!(c.origin.x, a.origin.x, "the third cell starts the next row");
    assert_eq!(c.size.height, px(60.0));
    assert_eq!(
        f32::from(c.top() - a.top()),
        50.0 + foundation().layout.section_gap,
        "under the taller of the first row"
    );
    assert_eq!(wide.origin.x, a.origin.x);
    assert_eq!(wide.size.width, heading.size.width, "across every column");
    assert!(wide.top() > c.bottom(), "on a row of its own");
    assert!(
        b.right() > c.right(),
        "leaving the third row's second cell empty"
    );
}

/// The column asks for one more frame when the count changes and none while it holds: a page
/// at rest does not draw again (item 35).
#[gpui::test]
fn the_column_draws_again_only_when_the_count_changes(cx: &mut TestAppContext) {
    let frames = Rc::new(Cell::new(0_u32));
    let page = grid_page(2, 0);
    let counted = frames.clone();
    let cx = open(cx, move |window, cx| {
        counted.set(counted.get() + 1);
        page(window, cx)
    });
    resize(cx, 1600.0, 700.0);
    let settled = frames.get();
    cx.run_until_parked();
    assert_eq!(frames.get(), settled, "a page at rest draws nothing");
    resize(cx, 1700.0, 700.0);
    let same = frames.get() - settled;
    resize(cx, 900.0, 700.0);
    let changed = frames.get() - settled - same;
    assert_eq!(
        changed,
        same + 1,
        "one frame more when the count changes, and only then"
    );
}

/// The #75 case on a page of columns: an item whose description wraps is its column's width and
/// keeps its words inside it.
#[gpui::test]
fn a_wrapped_item_stays_inside_its_cell(cx: &mut TestAppContext) {
    let long = "How a high-resolution or free-spin mouse wheel is turned into scroll events. \
                Smooth keeps the fine motion. Stepped turns it into clicks.";
    let cx = open(cx, move |window, cx| {
        let column = content_column_for_grid("page-column", 2, 2, window, cx);
        // Words of different lengths, which a row that sized its cells by them would not
        // share out evenly.
        let item = |title: &'static str, description: &'static str| {
            div().debug_selector(move || format!("cell/{title}")).child(
                Item::new(title)
                    .description(description)
                    .action(Select::new(title, title).item("1", "Default")),
            )
        };
        let columns = column.columns();
        scroll_y("page").child(
            column.child(
                Grid::new(columns)
                    .child(item("Scroll inertia", long))
                    .child(item("Scroll factor", "How far one notch scrolls.")),
            ),
        )
    });
    resize(cx, 1600.0, 700.0);
    for (cell, item, words) in [
        (
            "cell/Scroll inertia",
            "odk-item/Scroll inertia",
            "odk-item/Scroll inertia/words",
        ),
        (
            "cell/Scroll factor",
            "odk-item/Scroll factor",
            "odk-item/Scroll factor/words",
        ),
    ] {
        let layout = foundation().layout.clone();
        let column =
            (layout.wide_content_width - 2.0 * layout.panel_padding - layout.section_gap) / 2.0;
        let cell = cx.debug_bounds(cell).unwrap();
        let item = cx.debug_bounds(item).unwrap();
        let words = cx.debug_bounds(words).unwrap();
        assert_eq!(f32::from(cell.size.width), column, "the cell is a column");
        assert_eq!(item.size.width, cell.size.width);
        assert!(
            words.top() >= item.top() && words.bottom() <= item.bottom(),
            "the words {words:?} spill out of the item {item:?}"
        );
    }
    let (a, b) = (
        cx.debug_bounds("cell/Scroll inertia").unwrap(),
        cx.debug_bounds("cell/Scroll factor").unwrap(),
    );
    assert_eq!(a.top(), b.top(), "side by side");
}
