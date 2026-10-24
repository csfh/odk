//! `RadioGroup`: one choice among options, on the radio's component tokens.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{Modifiers, ParentElement, TestAppContext, VisualTestContext, px};
use odk_a11y::{Describe, Role};
use odk_theme::JND;
use odk_theme::component::{Shape, States};
use odk_theme::foundation::Density;

use super::stability::{Step, assert_still};
use super::{FIXTURES, open, press, publish_fixture, redraw};
use crate::radio::drawn;
use crate::{Button, RadioGroup, v_flex};

type Seen = Rc<RefCell<Vec<String>>>;

fn record(seen: &Seen) -> impl Fn(&str, &mut gpui::Window, &mut gpui::App) + 'static {
    let seen = seen.clone();
    move |v, _, _| seen.borrow_mut().push(v.to_owned())
}

/// Four fruits, the third disabled.
fn fruits(id: &'static str) -> RadioGroup {
    RadioGroup::new(id, "Fruit")
        .item("apple", "Apple")
        .item("banana", "Banana")
        .item("cherry", "Cherry")
        .item("damson", "Damson")
        .disable("cherry")
}

fn click(cx: &mut VisualTestContext, part: &str) {
    let b = cx
        .debug_bounds(Box::leak(part.to_owned().into_boxed_str()))
        .unwrap_or_else(|| panic!("no part {part}"));
    cx.simulate_click(b.center(), Modifiers::default());
    redraw(cx);
}

fn chosen(id: &str) -> Vec<bool> {
    (0..4)
        .map(|i| drawn::last(&format!("{id}/radio-{i}")).checked)
        .collect()
}

/// Items 8, 9, 10: Tab enters the group on its first option when none is chosen, Space chooses
/// it, Enter does not, and the arrows move and choose, wrapping and skipping the disabled one.
#[gpui::test]
fn the_arrows_move_and_choose_and_skip_disabled_options(cx: &mut TestAppContext) {
    let seen = Seen::default();
    let s = seen.clone();
    let cx = open(cx, move |_, _| fruits("g").on_change(record(&s)));
    press(cx, "tab enter");
    assert!(seen.borrow().is_empty(), "Enter does not choose");
    press(cx, "space");
    assert_eq!(*seen.borrow(), ["apple"]);
    press(cx, "down down");
    assert_eq!(
        *seen.borrow(),
        ["apple", "banana", "damson"],
        "Cherry is skipped"
    );
    press(cx, "down");
    assert_eq!(seen.borrow().last().unwrap(), "apple", "wraps to the first");
    press(cx, "up right left");
    assert_eq!(
        seen.borrow()[4..],
        ["damson", "apple", "damson"],
        "both axes move"
    );
    redraw(cx);
    assert_eq!(
        chosen("g"),
        [false, false, false, true],
        "one chosen at a time"
    );
    press(cx, "space");
    assert_eq!(
        seen.borrow().len(),
        7,
        "choosing the chosen one again reports nothing"
    );
}

/// Item 9: the group is one tab stop, entered on the chosen option.
#[gpui::test]
fn tab_enters_on_the_chosen_option_and_leaves_the_group(cx: &mut TestAppContext) {
    let after = Rc::new(Cell::new(0));
    let a = after.clone();
    let cx = open(cx, move |_, _| {
        let a = a.clone();
        v_flex().child(fruits("g").default_value("banana")).child(
            Button::new("after")
                .label("After")
                .on_click(move |_, _, _| a.set(a.get() + 1)),
        )
    });
    press(cx, "tab");
    redraw(cx);
    assert!(drawn::last("g/radio-1").focus_visible, "on the chosen one");
    assert!(!drawn::last("g/radio-0").focus_visible);
    press(cx, "tab enter");
    assert_eq!(after.get(), 1, "one more Tab leaves the group");
}

/// Clicks on a circle or a label choose once, and give that option focus; a disabled option
/// cannot be chosen (item 6).
#[gpui::test]
fn clicks_choose_once_and_disabled_options_cannot_be_chosen(cx: &mut TestAppContext) {
    let seen = Seen::default();
    let s = seen.clone();
    let cx = open(cx, move |_, _| fruits("g").on_change(record(&s)));
    click(cx, "g/radio-1");
    click(cx, "g/option-3/label");
    click(cx, "g/radio-2");
    click(cx, "g/option-2/label");
    assert_eq!(*seen.borrow(), ["banana", "damson"]);
    press(cx, "up");
    assert_eq!(
        seen.borrow().last().unwrap(),
        "banana",
        "the label gave its option focus"
    );
    let disabled = drawn::last("g/radio-2");
    assert_eq!(disabled.colours.hover, disabled.colours.default, "no hover");
}

/// Item 6: a disabled group is skipped by Tab and ignores clicks.
#[gpui::test]
fn a_disabled_group_is_skipped(cx: &mut TestAppContext) {
    let (seen, after) = (Seen::default(), Rc::new(Cell::new(0)));
    let (s, a) = (seen.clone(), after.clone());
    let cx = open(cx, move |_, _| {
        let a = a.clone();
        v_flex()
            .child(fruits("g").disabled(true).on_change(record(&s)))
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| a.set(a.get() + 1)),
            )
    });
    press(cx, "tab enter");
    assert_eq!(after.get(), 1);
    click(cx, "g/radio-0");
    assert!(seen.borrow().is_empty());
    assert!(fruits("g").disabled(true).accessible().disabled);
}

/// A controlled group shows what it is given and follows `on_change`.
#[gpui::test]
fn a_controlled_group_shows_its_value(cx: &mut TestAppContext) {
    let value = Rc::new(RefCell::new("banana".to_owned()));
    let v = value.clone();
    let cx = open(cx, move |_, _| {
        let set = v.clone();
        fruits("g")
            .value(v.borrow().clone())
            .on_change(move |next, _, _| *set.borrow_mut() = next.to_owned())
    });
    assert_eq!(chosen("g"), [false, true, false, false]);
    click(cx, "g/radio-0");
    assert_eq!(*value.borrow(), "apple");
    redraw(cx);
    assert_eq!(chosen("g"), [true, false, false, false]);
}

/// Items 14, 16, 17.
#[test]
fn reports_the_group_and_its_options() {
    let g = fruits("g").default_value("apple").invalid(true);
    let a = g.accessible();
    assert_eq!(a.role, Role::RadioGroup);
    assert!(a.invalid);
    let items = g.item_accessibles(Some("apple"));
    assert_eq!(items.len(), 4);
    assert!(items.iter().all(|i| i.role == Role::Radio));
    assert_eq!(items[0].selected, Some(true));
    assert!(items[2].disabled);
}

/// Item 32: choosing, focusing, hovering, pressing and disabling move nothing.
#[gpui::test]
fn no_state_moves_anything(cx: &mut TestAppContext) {
    let off = Rc::new(Cell::new(false));
    let o = off.clone();
    let cx = open(cx, move |_, _| {
        v_flex()
            .child(fruits("g").disabled(o.get()))
            .child(Button::new("after").label("After"))
    });
    let disable = off.clone();
    assert_still(
        cx,
        &["g/radio-0", "g/radio-1", "g/option-1/label", "after/button"],
        &[
            Step::Keys("tab"),
            Step::Keys("space"),
            Step::Keys("down"),
            Step::Hover("g/radio-0"),
            Step::Press("g/radio-0"),
            Step::Change("disabling it", Box::new(move || disable.set(true))),
        ],
    );
}

/// The circle is a check box's colours on a circle of the density's icon size, the chosen one
/// with a dot, in every fixture; a monochrome theme's radios are grey and the chosen one is
/// told apart by its dot, not its colour (item 18).
#[gpui::test]
fn it_draws_its_component_tokens_in_every_fixture(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex().children(Density::ALL.map(|d| {
            fruits(match d {
                Density::Compact => "compact",
                Density::Default => "default",
                _ => "comfortable",
            })
            .default_value("apple")
            .density(d)
        }))
    });
    for (theme, colors) in FIXTURES {
        publish_fixture(colors);
        redraw(cx);
        let c = crate::components();
        for d in Density::ALL {
            let id = d.name();
            let drawn = drawn::last(&format!("{id}/radio-0"));
            assert_eq!(
                drawn,
                c.radio(d, States::NONE.checked(true)),
                "{theme} {id}"
            );
            assert_eq!(drawn.geometry.shape, Shape::Circle);
            let b = cx
                .debug_bounds(Box::leak(format!("{id}/radio-0").into_boxed_str()))
                .unwrap();
            assert_eq!(b.size.width, px(d.metrics().icon_size));
            assert_eq!(b.size.height, px(d.metrics().icon_size));
            assert!(
                cx.debug_bounds(Box::leak(format!("{id}/dot-0").into_boxed_str()))
                    .is_some(),
                "the chosen one has a dot"
            );
            assert!(
                cx.debug_bounds(Box::leak(format!("{id}/dot-1").into_boxed_str()))
                    .is_none(),
                "the others have none"
            );
            if theme == "vantablack" {
                for t in [drawn.colours.default, drawn.colours.hover] {
                    for colour in [t.background, t.foreground, t.border] {
                        assert!(colour.to_oklch().c <= JND, "{theme}: {colour:?}");
                    }
                }
            }
        }
    }
}

/// Item 11: the focused option has a ring outside it, in the danger colour when the choice is
/// wrong, and the circle keeps its danger edge in focus.
#[gpui::test]
fn the_ring_takes_the_danger_colour_when_wrong(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| fruits("g").invalid(true));
    press(cx, "tab");
    redraw(cx);
    let s = drawn::last("g/radio-0");
    assert!(s.focus_visible);
    let semantic = crate::components().semantic().clone();
    assert_eq!(s.focus.colour, semantic.border.danger);
    assert_eq!(s.current.border, semantic.border.danger);
    // Item 18: and a mark once, after the first option, so it never rests on the colour.
    assert!(cx.debug_bounds("g/option-0/invalid-mark").is_some());
    assert!(cx.debug_bounds("g/option-1/invalid-mark").is_none());
    assert!(!fruits("v").accessible().invalid);
}

/// Item 23: a label too long for the column wraps beside its circle, and the next option sits
/// below the whole of it.
#[gpui::test]
fn a_long_label_wraps_and_the_next_option_sits_below_it(cx: &mut TestAppContext) {
    use gpui::Styled;
    let cx = open(cx, |_, _| {
        v_flex().max_w(px(200.0)).child(
            RadioGroup::new("g", "Region")
                .item("de", "Bundesrepublik Deutschland (Deutschland)")
                .item(
                    "url",
                    "https://wiki.omarchy.org/troubleshooting/backups/restic-repository-integrity-check-failed",
                )
                .item("jp", "日本国（日本語のとても長いラベル）"),
        )
    });
    let rows: Vec<_> = (0..3)
        .map(|i| {
            cx.debug_bounds(Box::leak(format!("g/option-{i}/row").into_boxed_str()))
                .unwrap()
        })
        .collect();
    let label = cx.debug_bounds("g/option-1/label").unwrap();
    assert!(label.size.height > px(20.0), "the long label wrapped");
    assert!(
        rows[1].bottom() >= label.bottom(),
        "the row holds its label"
    );
    assert!(
        rows[2].top() >= rows[1].bottom(),
        "{:?} overlaps {:?}",
        rows[2],
        rows[1]
    );
    let group = cx.debug_bounds("g/group").unwrap();
    assert!(
        group.bottom() >= rows[2].bottom(),
        "the group holds its rows"
    );
    assert!(group.right() <= px(200.0));
}

/// Item 23: in a row, options too long for the column wrap onto lines of their own, and
/// nothing overlaps.
#[gpui::test]
fn long_options_in_a_row_wrap_without_overlapping(cx: &mut TestAppContext) {
    use gpui::Styled;
    let cx = open(cx, |_, _| {
        v_flex()
            .max_w(px(200.0))
            .child(
                RadioGroup::new("g", "Region")
                    .horizontal(true)
                    .item("de", "Bundesrepublik Deutschland (Deutschland)")
                    .item(
                        "url",
                        "https://wiki.omarchy.org/troubleshooting/backups/restic",
                    )
                    .item("a", "A"),
            )
            .child(Button::new("after").label("After"))
    });
    let rows: Vec<_> = (0..3)
        .map(|i| {
            cx.debug_bounds(Box::leak(format!("g/option-{i}/row").into_boxed_str()))
                .unwrap()
        })
        .collect();
    for (i, row) in rows.iter().enumerate() {
        let label = cx
            .debug_bounds(Box::leak(format!("g/option-{i}/label").into_boxed_str()))
            .unwrap();
        assert!(row.bottom() >= label.bottom(), "row {i} holds its label");
        assert!(row.right() <= px(200.0), "row {i} fits the column");
    }
    for (a, b) in [(0, 1), (1, 2), (0, 2)] {
        assert!(!rows[a].intersects(&rows[b]), "{a} and {b} overlap");
    }
    let after = cx.debug_bounds("after/button").unwrap();
    assert!(
        after.top()
            >= rows
                .iter()
                .map(|r| r.bottom())
                .fold(px(0.0), |a, b| a.max(b))
    );
}

/// Item 23: in a row of other controls a group keeps its options' width; it is not squeezed.
#[gpui::test]
fn in_a_row_it_keeps_its_width(cx: &mut TestAppContext) {
    use gpui::Styled;
    let cx = open(cx, |_, _| {
        crate::h_flex()
            .w(px(320.0))
            .child(crate::Input::new("i", "Name"))
            .child(RadioGroup::new("g", "Fruit").item("a", "Apple"))
    });
    let label = cx.debug_bounds("g/option-0/label").unwrap();
    assert_eq!(label.size.height, px(20.0), "the label stays on one line");
}
