use std::cell::RefCell;
use std::rc::Rc;

use gpui::{ParentElement, SharedString, TestAppContext};
use odk_a11y::{Describe, Role};

use odk_theme::component::{SegmentStyle, States};

use super::{Count, FIXTURES, open, press, publish_fixture, redraw};
use crate::{Button, Density, ToggleGroup, v_flex};

type Seen = Rc<RefCell<Vec<Vec<String>>>>;

fn record(seen: &Seen) -> impl Fn(&[SharedString], &mut gpui::Window, &mut gpui::App) + 'static {
    let seen = seen.clone();
    move |value, _, _| {
        seen.borrow_mut()
            .push(value.iter().map(|v| v.to_string()).collect())
    }
}

/// Items 8, 9, 10: one tab stop on the selected option, arrows move between options, Space
/// and Enter choose, and the next Tab leaves the group.
#[gpui::test]
fn single_choice_from_the_keyboard(cx: &mut TestAppContext) {
    let seen = Seen::default();
    let after = Count::default();
    let (s, a) = (seen.clone(), after.clone());
    let cx = open(cx, move |_, _| {
        let a = a.clone();
        v_flex()
            .child(
                ToggleGroup::new("align", "Alignment")
                    .item("left", "Left")
                    .item("center", "Center")
                    .item("right", "Right")
                    .default_value(["center"])
                    .on_change(record(&s)),
            )
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| a.bump()),
            )
    });
    // Tab lands on the selected option, Center; Right moves to Right; Space chooses it.
    press(cx, "tab right space");
    // Home goes to Left, Enter chooses it; the arrow wraps from Left to Right.
    press(cx, "home enter left space");
    assert_eq!(*seen.borrow(), [["right"], ["left"], ["right"]]);
    press(cx, "tab enter");
    assert_eq!(after.get(), 1, "one Tab leaves the group");
}

/// Items 8, 10: in multiple mode each option toggles on its own.
#[gpui::test]
fn multiple_choice_toggles(cx: &mut TestAppContext) {
    let seen = Seen::default();
    let s = seen.clone();
    let cx = open(cx, move |_, _| {
        ToggleGroup::new("style", "Text style")
            .multiple(true)
            .icon_item("bold", "B", "Bold")
            .icon_item("italic", "I", "Italic")
            .on_change(record(&s))
    });
    press(cx, "tab space right space left space");
    assert_eq!(
        *seen.borrow(),
        [vec!["bold"], vec!["bold", "italic"], vec!["italic"]]
    );
}

/// Items 14, 16, 17: the group and each option report their role and state, and a glyph
/// option reports its accessible name.
#[test]
fn reports_the_group_and_each_option() {
    let group = ToggleGroup::new("g", "Alignment")
        .item("left", "Left")
        .icon_item("right", "→", "Right");
    assert_eq!(group.accessible().role, Role::RadioGroup);
    let items = group.item_accessibles(&["right".into()]);
    assert_eq!(
        (items[0].role, items[0].selected),
        (Role::Radio, Some(false))
    );
    assert_eq!(
        (items[1].name.as_str(), items[1].selected),
        ("Right", Some(true))
    );
    let multiple = ToggleGroup::new("m", "Options")
        .multiple(true)
        .item("b", "B");
    assert_eq!(multiple.accessible().role, Role::Group);
    assert_eq!(multiple.item_accessibles(&[])[0].role, Role::ToggleButton);
}

/// Item 15: the group is named by its constructor, so it cannot be left without one.
#[test]
fn is_named_by_its_constructor() {
    let a = ToggleGroup::new("g", "Alignment")
        .item("a", "A")
        .accessible();
    assert_eq!(a.name, "Alignment");
}

/// Item 6: the arrow keys skip a disabled option, which reports itself disabled.
#[gpui::test]
fn a_disabled_option_is_skipped(cx: &mut TestAppContext) {
    let seen = Seen::default();
    let s = seen.clone();
    let cx = open(cx, move |_, _| {
        ToggleGroup::new("align", "Alignment")
            .item("left", "Left")
            .item("center", "Center")
            .item("right", "Right")
            .disable("center")
            .on_change(record(&s))
    });
    press(cx, "tab right space");
    assert_eq!(
        *seen.borrow(),
        [["right"]],
        "Right skipped the disabled Center"
    );
    let group = ToggleGroup::new("g", "Options")
        .item("a", "A")
        .item("b", "B")
        .disable("b");
    let items = group.item_accessibles(&[]);
    assert!(!items[0].disabled && items[1].disabled);
}

/// Item 6: a disabled group is not a tab stop at all.
#[gpui::test]
fn a_disabled_group_is_skipped(cx: &mut TestAppContext) {
    let (seen, after) = (Seen::default(), Count::default());
    let (s, a) = (seen.clone(), after.clone());
    let cx = open(cx, move |_, _| {
        let a = a.clone();
        v_flex()
            .child(
                ToggleGroup::new("align", "Alignment")
                    .item("left", "Left")
                    .item("right", "Right")
                    .disabled(true)
                    .on_change(record(&s)),
            )
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| a.bump()),
            )
    });
    press(cx, "tab space");
    assert!(seen.borrow().is_empty());
    assert_eq!(after.get(), 1, "Tab went straight past the group");
    assert!(
        ToggleGroup::new("g", "Options")
            .disabled(true)
            .accessible()
            .disabled
    );
}

/// Item 32: focusing, hovering, pressing, choosing and disabling options moves nothing, in
/// the group or around it.
#[gpui::test]
fn no_state_moves_anything(cx: &mut TestAppContext) {
    use super::stability::{Step, assert_still};
    use std::cell::Cell;
    let off = Rc::new(Cell::new(false));
    let o = off.clone();
    let cx = open(cx, move |_, _| {
        let group = ToggleGroup::new("align", "Alignment")
            .item("left", "Left")
            .item("center", "Center")
            .item("right", "Right")
            .default_value(["left"]);
        let group = if o.get() {
            group.disable("center")
        } else {
            group
        };
        v_flex()
            .child(group)
            .child(Button::new("after").label("After"))
    });
    let disable = off.clone();
    assert_still(
        cx,
        &[
            "align/group",
            "align/item-0",
            "align/item-1",
            "align/item-2",
            "align/label-0",
            "align/label-1",
            "align/label-2",
            "after/button",
        ],
        &[
            Step::Keys("tab"),
            Step::Keys("right"),
            Step::Keys("space"),
            Step::Keys("right"),
            Step::Hover("align/item-0"),
            Step::Hover("align/item-2"),
            Step::Press("align/item-0"),
            Step::Press("align/item-1"),
            Step::Keys("tab"),
            Step::Change("disabling Center", Box::new(move || disable.set(true))),
        ],
    );
}

/// Item 23: in a column the group is as wide as its options, not stretched to the column's
/// width, so its border hugs the last option.
#[gpui::test]
fn the_group_hugs_its_options(cx: &mut TestAppContext) {
    let cx = open(cx, move |_, _| {
        v_flex().child(
            ToggleGroup::new("align", "Alignment")
                .item("left", "Left")
                .item("right", "Right"),
        )
    });
    let group = cx.debug_bounds("align/group").unwrap();
    let last = cx.debug_bounds("align/item-1").unwrap();
    let border = crate::components()
        .segmented(Density::Default)
        .geometry
        .border_width;
    assert_eq!(group.right(), last.right() + gpui::px(border));
}

/// Item 23: options too long for the column truncate instead of running past it; each keeps
/// room for a letter and its `…`, and short options keep their own width.
#[gpui::test]
fn long_options_fit_a_narrow_column(cx: &mut TestAppContext) {
    use gpui::{Styled, px};
    let cx = open(cx, move |_, _| {
        v_flex().w(px(180.0)).child(
            ToggleGroup::new("align", "Alignment")
                .item("left", "Linksbündig")
                .item("center", "Zentriert")
                .item("right", "R")
                .item("justify", "Blocksatz mit Silbentrennung"),
        )
    });
    let group = cx.debug_bounds("align/group").unwrap();
    assert!(group.right() <= px(180.0), "the group fits: {group:?}");
    let last = cx.debug_bounds("align/item-3").unwrap();
    assert!(
        last.right() <= group.right(),
        "every option ends inside the group: {last:?}"
    );
    let floor = px(crate::foundation().spacing.s6);
    for part in ["align/label-0", "align/label-1", "align/label-3"] {
        let w = cx.debug_bounds(part).unwrap().size.width;
        assert!(w >= floor, "{part} shrank to {w:?}");
    }
    let one_letter = cx.debug_bounds("align/label-2").unwrap().size.width;
    assert!(one_letter < floor, "a one-letter option is not padded");
}

fn segment(part: &str) -> SegmentStyle {
    crate::drawn::last::<SegmentStyle>(part)
}

/// The frame is a control's height at each density and its segments fill it inside its border;
/// an icon alone is a square segment, and an icon before text is the density's icon size.
#[gpui::test]
fn segments_follow_their_density(cx: &mut TestAppContext) {
    let cx = open(cx, move |_, _| {
        v_flex().children(Density::ALL.map(|d| {
            ToggleGroup::new(gpui::SharedString::from(d.name()), "View")
                .density(d)
                .item("list", "List")
                .icon("list", "☰")
                .icon_item("grid", "▦", "Grid")
        }))
    });
    for d in Density::ALL {
        let n = d.name();
        let part = |p: &str| -> &'static str { Box::leak(format!("{n}/{p}").into_boxed_str()) };
        let group = cx.debug_bounds(part("group")).unwrap();
        assert_eq!(group.size.height, gpui::px(d.metrics().height), "{n}");
        let g = segment(part("item-0")).geometry;
        for item in ["item-0", "item-1"] {
            let b = cx.debug_bounds(part(item)).unwrap();
            assert_eq!(
                b.size.height,
                group.size.height - gpui::px(2.0),
                "{n} {item}"
            );
        }
        let square = cx.debug_bounds(part("item-1")).unwrap();
        assert_eq!(
            square.size.width, square.size.height,
            "{n}: an icon alone is square"
        );
        assert_eq!(g.icon_size, d.metrics().icon_size);
        let label = cx.debug_bounds(part("label-0")).unwrap();
        let first = cx.debug_bounds(part("item-0")).unwrap();
        assert_eq!(
            label.left(),
            first.left() + gpui::px(g.padding_x + g.icon_size + g.gap)
        );
    }
}

/// Items 5, 18, 34: each segment draws its lookup for its state; the one that is on has a fill
/// that the others, hovered or not, do not, in every fixture; a disabled one keeps showing that
/// it is on.
#[gpui::test]
fn segments_draw_their_tokens_in_every_fixture(cx: &mut TestAppContext) {
    let cx = open(cx, move |_, _| {
        ToggleGroup::new("v", "View")
            .item("a", "Day")
            .item("b", "Week")
            .item("c", "Month")
            .default_value(["b", "c"])
            .multiple(true)
            .disable("c")
    });
    let d = Density::Default;
    for (theme, colors) in FIXTURES {
        publish_fixture(colors);
        redraw(cx);
        let c = crate::components();
        let (off, on) = (segment("v/item-0"), segment("v/item-1"));
        assert_eq!(off, c.segment(d, States::NONE), "{theme}");
        assert_eq!(on, c.segment(d, States::NONE.selected(true)), "{theme}");
        assert_eq!(
            segment("v/item-2"),
            c.segment(d, States::NONE.selected(true).disabled(true))
        );
        let ground = c.semantic().surface.canvas;
        let fill = |t: odk_theme::semantic::Treatment| t.background.over(ground);
        for pointer in [off.colours.default, off.colours.hover] {
            assert!(
                fill(on.current).apparent_distance(fill(pointer)) >= odk_theme::JND,
                "{theme}: on and off read apart"
            );
        }
    }
}

/// Item 11: the focused segment's ring is painted around it, over the selected segment after it,
/// whose fill would cover a ring drawn as the focused segment's own child.
#[gpui::test]
fn the_ring_is_painted_over_the_next_segment(cx: &mut TestAppContext) {
    use crate::interaction::painted;
    let cx = open(cx, move |_, _| {
        ToggleGroup::new("v", "View")
            .item("a", "Day")
            .item("b", "Week")
            .default_value(["b"])
    });
    // Tab lands on Week, the selected one; Left moves focus to Day, before it.
    press(cx, "tab left");
    redraw(cx);
    let ring = segment("v/item-0").focus;
    let day = cx.debug_bounds("v/item-0").unwrap();
    assert_eq!(
        painted::take("v"),
        Some(day.dilate(gpui::px(ring.offset + ring.width)))
    );
    assert!(segment("v/item-0").focus_visible && !segment("v/item-1").focus_visible);
}
