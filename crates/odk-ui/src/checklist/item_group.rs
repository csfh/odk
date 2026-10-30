//! A group of items: a title, a description and items with a line between two. Items 5 to 13, 16,
//! 17, 19 to 22, 25, 29, 31 to 33, 38 and 39 do not apply: it has no states, takes no keys and no
//! focus of its own, opens nothing, is not a composite control and has no motion, and its items
//! keep their own.

use gpui::{Bounds, ParentElement, Pixels, Styled, TestAppContext, VisualTestContext, px};
use odk_a11y::{Describe, Role};
use odk_theme::component::ItemGroupStyle;

use super::open;
use crate::{Item, ItemGroup, Select, Switch, v_flex};

/// The width the groups are drawn in.
const WIDTH: f32 = 400.0;

fn bounds(cx: &mut VisualTestContext, part: &str) -> Bounds<Pixels> {
    let name: &'static str = Box::leak(part.to_owned().into_boxed_str());
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{part} is not drawn"))
}

fn drawn(cx: &mut VisualTestContext, part: &str) -> bool {
    let name: &'static str = Box::leak(part.to_owned().into_boxed_str());
    cx.debug_bounds(name).is_some()
}

fn touchpad(items: usize) -> ItemGroup {
    let titles = ["Natural scroll", "Tap to click", "Ignore while typing"];
    titles[..items].iter().fold(
        ItemGroup::new("Touchpad").description("Feel for the trackpad."),
        |group, title| {
            group.child(
                Item::new(*title)
                    .description("What it does.")
                    .action(Switch::new(*title, *title)),
            )
        },
    )
}

/// Items 14, 15 and 27: a group reports a group named by its title, with its description.
#[test]
fn reports_a_group_named_by_its_title_with_its_description() {
    let a = touchpad(1).accessible();
    assert_eq!((a.role, a.name.as_str()), (Role::Group, "Touchpad"));
    assert_eq!(a.description.as_deref(), Some("Feel for the trackpad."));
    assert_eq!(ItemGroup::new("Bare").accessible().description, None);
}

/// Items 1 and 34: its title is drawn as a level 2 heading and its description as secondary
/// body text, from its component tokens.
#[gpui::test]
fn reads_as_a_heading_and_secondary_text(cx: &mut TestAppContext) {
    let _cx = open(cx, |_, _| v_flex().w(px(WIDTH)).child(touchpad(2)));
    let c = crate::components();
    let s = crate::drawn::last::<ItemGroupStyle>("odk-item-group/Touchpad");
    assert_eq!(s, c.item_group());
    let (heading, text) = (c.heading(2), c.text(true));
    assert_eq!((s.title, s.title_role), (heading.colour, heading.role));
    assert_eq!(
        (s.description, s.description_role),
        (text.colour, text.role)
    );
}

/// A line divides one item from the next, as wide as the group, and none sits before the first
/// item or after the last.
#[gpui::test]
fn a_line_divides_one_item_from_the_next_and_only_there(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| v_flex().w(px(WIDTH)).child(touchpad(3)));
    let group = bounds(cx, "odk-item-group/Touchpad");
    let items = ["Natural scroll", "Tap to click", "Ignore while typing"]
        .map(|t| bounds(cx, &format!("odk-item/{t}")));
    for i in 1..3 {
        let line = bounds(cx, &format!("odk-item-group/Touchpad/separator-{i}"));
        assert_eq!(
            line.top(),
            items[i - 1].bottom(),
            "line {i} follows item {i}"
        );
        assert_eq!(
            line.bottom(),
            items[i].top(),
            "and the next item follows it"
        );
        assert_eq!(line.size.width, group.size.width, "across the group");
        assert_eq!(
            f32::from(line.size.height),
            crate::components().separator().thickness,
            "a separator's thickness"
        );
    }
    assert!(!drawn(cx, "odk-item-group/Touchpad/separator-0"));
    assert!(!drawn(cx, "odk-item-group/Touchpad/separator-3"));
    let description = bounds(cx, "odk-item-group/Touchpad/description");
    assert_eq!(
        f32::from(items[0].top() - description.bottom()),
        crate::components().item_group().gap,
        "the first item follows the description by the gap, with no line"
    );
}

/// Item 23: a group of one item has no line, and a group of none is its title and description.
#[gpui::test]
fn one_item_has_no_line_and_none_is_its_words(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .w(px(WIDTH))
            .child(touchpad(1))
            .child(ItemGroup::new("Empty").description("Nothing in it yet."))
    });
    assert!(!drawn(cx, "odk-item-group/Touchpad/separator-1"));
    let (group, words) = (
        bounds(cx, "odk-item-group/Empty"),
        bounds(cx, "odk-item-group/Empty/description"),
    );
    assert!(
        group.bottom() >= words.bottom(),
        "{words:?} inside {group:?}"
    );
}

/// Its title, its description and its items' titles start at the same edge: it has no padding of
/// its own.
#[gpui::test]
fn its_words_line_up_with_its_items(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| v_flex().w(px(WIDTH)).child(touchpad(2)));
    let x = bounds(cx, "odk-item-group/Touchpad").left();
    for part in [
        "odk-item-group/Touchpad/title",
        "odk-item-group/Touchpad/description",
        "odk-item/Natural scroll/title",
        "odk-item/Tap to click/title",
    ] {
        assert_eq!(bounds(cx, part).left(), x, "{part}");
    }
}

/// Item 23, the #75 case: under a scroll region, a description that wraps stays inside the group,
/// above its first item.
#[gpui::test]
fn a_wrapped_description_stays_above_its_items_on_a_scrolling_page(cx: &mut TestAppContext) {
    let long = "Sensitivity and acceleration for the mouse and trackpad. These write a managed \
                block in ~/.config/hypr/input.lua, beside the lines Atmos leaves alone.";
    let cx = open(cx, move |_, _| {
        v_flex().w(px(500.0)).h(px(700.0)).child(
            crate::scroll_y("page").child(
                crate::content_column().child(
                    ItemGroup::new("Pointer").description(long).child(
                        Item::new("Acceleration")
                            .action(Select::new("accel", "Acceleration").item("1", "Default")),
                    ),
                ),
            ),
        )
    });
    let group = bounds(cx, "odk-item-group/Pointer");
    let words = bounds(cx, "odk-item-group/Pointer/description");
    let item = bounds(cx, "odk-item/Acceleration");
    assert!(
        words.top() >= group.top() && words.bottom() <= item.top(),
        "the description {words:?} spills over the item {item:?}"
    );
    assert!(
        f32::from(words.size.height) < 100.0,
        "it wraps across the column, not a word a line: {words:?}"
    );
}
