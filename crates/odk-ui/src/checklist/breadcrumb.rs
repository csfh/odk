use gpui::{ParentElement, TestAppContext};
use odk_a11y::{Describe, Role};

use super::{Count, open, press};
use crate::{Breadcrumb, Button, v_flex};

/// Items 8, 9, 10: each link is a tab stop pressed with Enter, and the current page is not
/// a tab stop.
#[gpui::test]
fn links_are_tab_stops_and_the_current_page_is_not(cx: &mut TestAppContext) {
    let (settings, display, after) = (Count::default(), Count::default(), Count::default());
    let (s, d, a) = (settings.clone(), display.clone(), after.clone());
    let cx = open(cx, move |_, _| {
        let (s, d, a) = (s.clone(), d.clone(), a.clone());
        v_flex()
            .child(
                Breadcrumb::new("b")
                    .item("Settings", move |_, _| s.bump())
                    .item("Display", move |_, _| d.bump())
                    .current("Scaling"),
            )
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| a.bump()),
            )
    });
    press(cx, "tab enter tab enter tab enter");
    assert_eq!((settings.get(), display.get(), after.get()), (1, 1, 1));
}

/// Item 9: the last item is the current page even when it was added with a link.
#[gpui::test]
fn the_last_item_is_never_a_tab_stop(cx: &mut TestAppContext) {
    let (last, after) = (Count::default(), Count::default());
    let (l, a) = (last.clone(), after.clone());
    let cx = open(cx, move |_, _| {
        let (l, a) = (l.clone(), a.clone());
        v_flex()
            .child(
                Breadcrumb::new("b")
                    .item("Settings", |_, _| {})
                    .item("Display", move |_, _| l.bump()),
            )
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| a.bump()),
            )
    });
    press(cx, "tab tab enter");
    assert_eq!((last.get(), after.get()), (0, 1));
}

/// Items 14, 15.
#[test]
fn reports_navigation() {
    let a = Breadcrumb::new("b").current("Home").accessible();
    assert_eq!(a.role, Role::Navigation);
    assert!(!a.name.is_empty());
}

/// Item 17: each part reports itself in order: links, the folded middle as a button, and
/// the current page.
#[test]
fn reports_each_part() {
    let b = Breadcrumb::new("b")
        .max_items(3)
        .item("Home", |_, _| {})
        .item("Settings", |_, _| {})
        .item("Hardware", |_, _| {})
        .item("Input", |_, _| {})
        .current("Keyboard");
    let parts: Vec<(Role, String)> = b
        .item_accessibles()
        .into_iter()
        .map(|a| (a.role, a.name))
        .collect();
    assert_eq!(
        parts,
        [
            (Role::Link, "Home".into()),
            (Role::Button, "Show 2 hidden items".into()),
            (Role::Link, "Input".into()),
            (Role::CurrentPage, "Keyboard".into()),
        ]
    );
}

/// Items 8, 9, 10: the folded middle is a tab stop; Enter opens a menu of the hidden items,
/// Down moves in it, and Enter follows one. Down also opens it.
#[gpui::test]
fn the_folded_middle_opens_a_menu_of_the_hidden_items(cx: &mut TestAppContext) {
    use std::cell::RefCell;
    use std::rc::Rc;
    let went: Rc<RefCell<Vec<&'static str>>> = Rc::default();
    let w = went.clone();
    let cx = open(cx, move |_, _| {
        let go = |name: &'static str| {
            let w = w.clone();
            move |_: &mut gpui::Window, _: &mut gpui::App| w.borrow_mut().push(name)
        };
        Breadcrumb::new("b")
            .max_items(3)
            .item("Home", go("home"))
            .item("Settings", go("settings"))
            .item("Hardware", go("hardware"))
            .item("Input", go("input"))
            .current("Keyboard")
    });
    super::reduce_motion();
    // Home, then the fold.
    press(cx, "tab tab enter down enter");
    assert_eq!(*went.borrow(), ["hardware"]);
    press(cx, "down enter");
    assert_eq!(
        *went.borrow(),
        ["hardware", "settings"],
        "Down opened it on Settings"
    );
}

/// Item 23: a path too long for its width stays inside it. Labels give up width, the middle
/// ones a larger share of theirs than the ends; separators keep theirs; no two parts overlap.
#[gpui::test]
fn a_long_path_fits_its_width(cx: &mut TestAppContext) {
    use gpui::{Styled, px};
    use std::cell::Cell;
    use std::rc::Rc;
    let width = Rc::new(Cell::new(4000.0));
    let w = width.clone();
    let cx = open(cx, move |_, _| {
        v_flex().child(
            gpui::div().w(px(w.get())).child(
                Breadcrumb::new("b")
                    .item("A very long first step of the path", |_, _| {})
                    .item("Ein sehr langer mittlerer Schritt", |_, _| {})
                    .item("とても長い三番目のステップ", |_, _| {})
                    .current("The page you are on, with a long name"),
            ),
        )
    });
    let measure = |cx: &mut gpui::VisualTestContext| {
        super::redraw(cx);
        let parts = [
            "b/item-0", "b/item-1", "b/item-2", "b/item-3", "b/sep-1", "b/sep-2", "b/sep-3",
        ];
        parts.map(|p| cx.debug_bounds(p).unwrap())
    };
    let natural = measure(cx);
    width.set(260.0);
    let narrow = measure(cx);

    let row = cx.debug_bounds("b/row").unwrap();
    assert!(row.size.width <= px(260.0), "the row fits: {:?}", row.size);
    for pair in narrow[..4].windows(2) {
        assert!(pair[0].right() <= pair[1].left(), "parts overlap: {pair:?}");
    }
    assert!(
        narrow[3].right() <= row.right(),
        "the current page stays inside"
    );
    for sep in 4..7 {
        assert_eq!(
            narrow[sep].size.width, natural[sep].size.width,
            "a separator shrank"
        );
    }
    let kept = |i: usize| narrow[i].size.width / natural[i].size.width;
    for middle in [1, 2] {
        for end in [0, 3] {
            assert!(
                kept(middle) < kept(end),
                "item {middle} kept {} of its width, item {end} {}",
                kept(middle),
                kept(end)
            );
        }
    }
}

/// Item 32: focusing, hovering and pressing links moves nothing.
#[gpui::test]
fn no_state_moves_anything(cx: &mut TestAppContext) {
    use super::stability::{Step, assert_still};
    let cx = open(cx, move |_, _| {
        v_flex()
            .child(
                Breadcrumb::new("b")
                    .item("Settings", |_, _| {})
                    .item("Display", |_, _| {})
                    .current("Scaling"),
            )
            .child(Button::new("after").label("After"))
    });
    assert_still(
        cx,
        &["b/item-0", "b/item-1", "b/item-2", "after/button"],
        &[
            Step::Keys("tab"),
            Step::Keys("tab"),
            Step::Hover("b/item-0"),
            Step::Press("b/item-1"),
            Step::Keys("tab"),
        ],
    );
}

/// Item 23: a column too short for its content squeezes its children, but never the
/// breadcrumb: it keeps the height it has when there is room.
#[gpui::test]
fn a_squeezed_column_does_not_collapse_it(cx: &mut TestAppContext) {
    use gpui::{Styled, px};
    use std::cell::Cell;
    use std::rc::Rc;
    let squeezed = Rc::new(Cell::new(false));
    let s = squeezed.clone();
    let cx = open(cx, move |_, _| {
        let column = v_flex()
            .child(Breadcrumb::new("b").item("Home", |_, _| {}).current("Page"))
            .child(gpui::div().h(px(4000.0)));
        if s.get() { column.h(px(100.0)) } else { column }
    });
    super::redraw(cx);
    let room = cx.debug_bounds("b/row").unwrap().size.height;
    assert!(room > px(0.0));
    squeezed.set(true);
    super::redraw(cx);
    assert_eq!(cx.debug_bounds("b/row").unwrap().size.height, room);
}

/// Item 23: links, separators, the folded middle and the current page share one line: every
/// part has the same top and the same height, so their text sits on one baseline.
#[gpui::test]
fn every_part_sits_on_one_line(cx: &mut TestAppContext) {
    let cx = open(cx, move |_, _| {
        Breadcrumb::new("b")
            .max_items(3)
            .item("Home", |_, _| {})
            .item("Settings", |_, _| {})
            .item("Input", |_, _| {})
            .current("Keyboard")
    });
    let parts = [
        "b/item-0",
        "b/sep-1",
        "b/more/link",
        "b/sep-2",
        "b/item-2",
        "b/sep-3",
        "b/item-3",
    ];
    let bounds: Vec<_> = parts
        .iter()
        .map(|p| (p, cx.debug_bounds(p).unwrap()))
        .collect();
    let (first, line) = bounds[0];
    for (part, b) in &bounds {
        assert_eq!(
            (b.top(), b.size.height),
            (line.top(), line.size.height),
            "{part} is off the line of {first}"
        );
    }
}

/// Item 23: in a very narrow space every label keeps room for a letter and its `…`; none
/// shrinks to nothing and leaves two separators side by side.
#[gpui::test]
fn no_label_vanishes_in_a_narrow_space(cx: &mut TestAppContext) {
    use gpui::{Styled, px};
    let cx = open(cx, move |_, _| {
        v_flex().child(
            gpui::div().w(px(220.0)).child(
                Breadcrumb::new("b")
                    .item("Home", |_, _| {})
                    .item("Deutsch (Schweiz, ohne Akzenttasten)", |_, _| {})
                    .item("Eingabegeräte", |_, _| {})
                    .current("Erweiterte Tastenkombinationen für Barrierefreiheit"),
            ),
        )
    });
    let floor = px(crate::foundation().spacing.s6);
    for part in ["b/item-0", "b/item-1", "b/item-2", "b/item-3"] {
        let w = cx.debug_bounds(part).unwrap().size.width;
        assert!(w >= floor, "{part} shrank to {w:?}");
    }
}

/// Item 23: a label shorter than the floor keeps its own width, so short labels are not
/// padded and every gap between labels is the same.
#[gpui::test]
fn a_short_label_is_not_padded(cx: &mut TestAppContext) {
    let cx = open(cx, move |_, _| {
        Breadcrumb::new("b")
            .item("S", |_, _| {})
            .item("Ä", |_, _| {})
            .current("Page")
    });
    let (s, sep, a) = (
        cx.debug_bounds("b/item-0").unwrap(),
        cx.debug_bounds("b/sep-1").unwrap(),
        cx.debug_bounds("b/item-1").unwrap(),
    );
    let sep2 = cx.debug_bounds("b/sep-2").unwrap();
    assert_eq!(
        sep.left() - s.right(),
        sep2.left() - a.right(),
        "equal gaps after S and Ä"
    );
    assert!(s.size.width < gpui::px(crate::foundation().spacing.s6));
}

/// A second click on the folded `…` closes its menu, as on any menu button.
#[gpui::test]
fn a_second_click_on_the_fold_closes_its_menu(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Breadcrumb::new("b")
            .max_items(3)
            .item("Home", |_, _| {})
            .item("Settings", |_, _| {})
            .item("Hardware", |_, _| {})
            .item("Input", |_, _| {})
            .current("Keyboard")
    });
    super::reduce_motion();
    let click = |cx: &mut gpui::VisualTestContext| {
        let at = cx.debug_bounds("b/more/link").unwrap().center();
        cx.simulate_click(at, gpui::Modifiers::none());
        super::redraw(cx);
    };
    let name = "Name(\"b\")";
    click(cx);
    assert_eq!(crate::overlay::probe::last(name), Some(1.0), "opens");
    click(cx);
    assert_eq!(crate::overlay::probe::last(name), None, "closes");
}

/// Item 33: the folded middle of a breadcrumb inside a dialog opens its menu above the dialog.
#[gpui::test]
fn a_folded_breadcrumb_in_a_dialog(cx: &mut TestAppContext) {
    use super::dialog::dialog_around;
    let cx = dialog_around(cx, Count::default(), || {
        Breadcrumb::new("b")
            .max_items(3)
            .item("Home", |_, _| {})
            .item("Settings", |_, _| {})
            .item("Hardware", |_, _| {})
            .item("Input", |_, _| {})
            .current("Keyboard")
    });
    // Close, Home, the fold.
    press(cx, "tab tab tab enter");
    assert_eq!(crate::overlay::probe::last("Name(\"b\")"), Some(1.0));
}

/// Items 5, 11, 34: links draw the nav link's tokens, the current page primary text and the
/// ancestors secondary; a focused link's ring is painted around it from outside the row, which
/// clips the labels, and goes when focus does.
#[gpui::test]
fn links_draw_their_tokens_and_their_ring(cx: &mut TestAppContext) {
    use crate::interaction::painted;
    use odk_theme::component::{LinkStyle, States};
    let cx = open(cx, move |_, _| {
        v_flex()
            .child(
                Breadcrumb::new("b")
                    .item("Settings", |_, _| {})
                    .item("Display", |_, _| {})
                    .current("Scaling"),
            )
            .child(Button::new("after").label("After"))
    });
    let c = crate::components();
    let link = |i: usize| crate::drawn::last::<LinkStyle>(&format!("b/item-{i}/link"));
    assert_eq!(link(0), c.nav_link(States::NONE));
    assert_eq!(link(0).current.foreground, c.semantic().text.secondary);
    assert_eq!(
        c.nav_link(States::NONE.selected(true)).current.foreground,
        c.semantic().text.primary,
        "the current page"
    );
    super::redraw(cx);
    assert_eq!(painted::take("b"), None);
    press(cx, "tab tab");
    super::redraw(cx);
    assert_eq!(link(1), c.nav_link(States::NONE.focused(true)));
    let ring = link(1).focus;
    let display = cx.debug_bounds("b/item-1/link").unwrap();
    assert_eq!(
        painted::take("b"),
        Some(display.dilate(gpui::px(ring.offset + ring.width)))
    );
    press(cx, "tab");
    super::redraw(cx);
    assert_eq!(painted::take("b"), None);
}
