//! An item: a row of content that is not a tab stop. Items 5 to 13, 16, 17, 19 to 22, 25, 29,
//! 31 to 33, 38 and 39 do not apply: it has no states, takes no keys and no focus of its own,
//! opens nothing, is not a composite control and has no motion, and its actions keep their own.

use gpui::{
    Bounds, InteractiveElement, ParentElement, Pixels, Styled, TestAppContext, VisualTestContext,
    px,
};
use odk_a11y::{Describe, Role};
use odk_theme::component::ItemStyle;
use odk_theme::foundation::Density;

use super::{open, redraw};
use crate::{Button, Item, Select, Switch, Text, h_flex, v_flex};

/// The width the items are drawn in.
const WIDTH: f32 = 400.0;

fn bounds(cx: &mut VisualTestContext, part: &str) -> Bounds<Pixels> {
    let name: &'static str = Box::leak(part.to_owned().into_boxed_str());
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{part} is not drawn"))
}

/// Items 14, 15 and 27: an item reports a group named by its title, with its description.
#[test]
fn reports_a_group_named_by_its_title_with_its_description() {
    let a = Item::new("Natural scroll")
        .description("Content moves with your fingers.")
        .accessible();
    assert_eq!((a.role, a.name.as_str()), (Role::Group, "Natural scroll"));
    assert_eq!(
        a.description.as_deref(),
        Some("Content moves with your fingers.")
    );
    assert_eq!(Item::new("Bare").accessible().description, None);
}

/// Items 1, 34 and 37: its title and description are a form field's label and description, and
/// it is padded above and below by its density, its own or the application's.
#[gpui::test]
fn reads_as_a_form_field_and_pads_by_its_density(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .w(px(WIDTH))
            .child(Item::new("Applied").description("The application's density."))
            .child(
                Item::new("Own")
                    .description("Comfortable whatever the application's is.")
                    .density(Density::Comfortable),
            )
    });
    let c = crate::components();
    let field = c.form_field(odk_theme::component::States::NONE);
    let s = crate::drawn::last::<ItemStyle>("odk-item/Applied");
    assert_eq!(s, c.item(Density::Default));
    assert_eq!((s.title, s.title_role), (field.label, field.label_role));
    assert_eq!(
        (s.description, s.description_role),
        (field.description, field.description_role)
    );
    let padding = |cx: &mut VisualTestContext, title: &str| {
        let item = bounds(cx, &format!("odk-item/{title}"));
        let words = bounds(cx, &format!("odk-item/{title}/words"));
        (
            f32::from(words.top() - item.top()),
            f32::from(item.bottom() - words.bottom()),
        )
    };
    assert_eq!(padding(cx, "Applied"), (8.0, 8.0));
    assert_eq!(padding(cx, "Own"), (12.0, 12.0));

    cx.update(|_, cx| crate::configure(cx, |c| c.density = Density::Compact));
    redraw(cx);
    assert_eq!(padding(cx, "Applied"), (4.0, 4.0));
    assert_eq!(padding(cx, "Own"), (12.0, 12.0), "its own density stays");
}

/// Item 23: in a narrow column the words wrap into the room the actions leave, and an action
/// keeps the width it has alone, even one that would give it up, such as a line of text.
#[gpui::test]
fn the_words_wrap_and_an_action_is_never_squeezed(cx: &mut TestAppContext) {
    const LABEL: &str = "Sixteen listening ports";
    let long = "A description long enough to wrap several times in a narrow column, and then \
                some more, so that it would push anything beside it if it could.";
    let cx = open(cx, move |_, _| {
        v_flex()
            .w(px(WIDTH))
            .child(
                h_flex().child(
                    gpui::div()
                        .debug_selector(|| "alone".into())
                        .child(Text::new(LABEL).single_line()),
                ),
            )
            .child(
                v_flex().w(px(240.0)).child(
                    Item::new("Narrow")
                        .description(long)
                        .action(Text::new(LABEL).single_line()),
                ),
            )
    });
    let alone = bounds(cx, "alone");
    let action = bounds(cx, "odk-item/Narrow/action");
    assert_eq!(action.size.width, alone.size.width);
    let item = bounds(cx, "odk-item/Narrow");
    let words = bounds(cx, "odk-item/Narrow/words");
    let gap = crate::components().item(Density::Default).gap;
    assert_eq!(item.size.width, px(240.0));
    assert_eq!(
        words.size.width + px(gap) + action.size.width,
        item.size.width
    );
    let description = bounds(cx, "odk-item/Narrow/description");
    let line = crate::components()
        .foundation()
        .typography
        .style(crate::components().item(Density::Default).description_role)
        .line_height;
    assert!(
        f32::from(description.size.height) > 2.0 * line,
        "the description wraps"
    );
}

/// The actions sit at the item's end in the order they were given, `gap` apart, the last at its
/// right edge; the lines added with `child` follow the description.
#[gpui::test]
fn actions_end_the_item_in_order_and_lines_follow_the_description(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex().w(px(WIDTH)).child(
            Item::new("Ordered")
                .description("Two actions.")
                .child(Text::new("~/.config/hypr/input.lua").mono().muted())
                .action(Switch::new("first", "First"))
                .action(Button::new("second").label("Second")),
        )
    });
    let item = bounds(cx, "odk-item/Ordered");
    let first = bounds(cx, "first/track");
    let second = bounds(cx, "second/button");
    let gap = px(crate::components().item(Density::Default).gap);
    assert!(first.right() + gap <= second.left());
    assert_eq!(second.right(), item.right());
    let description = bounds(cx, "odk-item/Ordered/description");
    let words = bounds(cx, "odk-item/Ordered/words");
    assert!(
        words.bottom() > description.bottom(),
        "the line is under the description"
    );
}

/// A footer spans the item under its words, `footer_gap` below them; an item with no
/// description draws none.
#[gpui::test]
fn a_footer_spans_the_item_under_its_words(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex().w(px(WIDTH)).child(
            Item::new("Footed")
                .action(Button::new("star").label("Star"))
                .footer(Button::new("wide").label("Wide").full_width()),
        )
    });
    let item = bounds(cx, "odk-item/Footed");
    let footer = bounds(cx, "odk-item/Footed/footer");
    let words = bounds(cx, "odk-item/Footed/words");
    let action = bounds(cx, "odk-item/Footed/action");
    let s = crate::components().item(Density::Default);
    assert_eq!(footer.size.width, item.size.width);
    assert_eq!(footer.left(), item.left());
    assert_eq!(
        footer.top(),
        words.bottom().max(action.bottom()) + px(s.footer_gap)
    );
    assert!(cx.debug_bounds("odk-item/Footed/description").is_none());
}

/// A control that fills its parent, such as a select, takes its own width at the item's end
/// and leaves the words the rest.
#[gpui::test]
fn a_control_that_fills_its_parent_leaves_the_words_their_room(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex().w(px(WIDTH)).child(
            Item::new("Chosen")
                .description("Adaptive speeds up as you move.")
                .action(Select::new("accel", "Acceleration").item("default", "Default")),
        )
    });
    let item = bounds(cx, "odk-item/Chosen");
    let words = bounds(cx, "odk-item/Chosen/words");
    assert!(
        words.size.width > item.size.width / 2.0,
        "the select took the words' room: {words:?} in {item:?}"
    );
}

/// Item 23: under a scroll region, as on a settings page, a description that wraps keeps the
/// words inside the item: the words column is as tall as its lines, not as tall as the
/// description would be one word wide, which would centre it far above the item.
#[gpui::test]
fn a_wrapped_description_stays_inside_its_item_on_a_scrolling_page(cx: &mut TestAppContext) {
    let long = "How a high-resolution or free-spin mouse wheel is turned into scroll events. \
                Smooth keeps the fine motion. Stepped turns it into clicks.";
    let cx = open(cx, move |_, _| {
        v_flex().w(px(900.0)).h(px(700.0)).child(
            crate::scroll_y("page").child(
                crate::content_column().child(
                    Item::new("Scroll inertia")
                        .description(long)
                        .child(Text::new("~/.config/hypr/input.lua").mono().muted())
                        .action(Select::new("inertia", "Scroll inertia").item("1", "Default")),
                ),
            ),
        )
    });
    let item = bounds(cx, "odk-item/Scroll inertia");
    let words = bounds(cx, "odk-item/Scroll inertia/words");
    assert!(
        words.top() >= item.top() && words.bottom() <= item.bottom(),
        "the words {words:?} spill out of the item {item:?}"
    );
}
