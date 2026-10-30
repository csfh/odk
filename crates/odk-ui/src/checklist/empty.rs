//! An empty state: an icon, a title, a description and actions, centred. Items 5 to 13, 16, 17,
//! 19 to 22, 25, 29, 31 to 33, 38 and 39 do not apply: it has no states, takes no keys and no
//! focus of its own, opens nothing, is not a composite control and has no motion, and its actions
//! keep their own.

use gpui::{Bounds, ParentElement, Pixels, Styled, TestAppContext, VisualTestContext, px};
use odk_a11y::{Describe, Role};
use odk_theme::component::EmptyStyle;

use super::open;
use crate::{Button, Empty, Icon, v_flex};

fn bounds(cx: &mut VisualTestContext, part: &str) -> Bounds<Pixels> {
    let name: &'static str = Box::leak(part.to_owned().into_boxed_str());
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{part} is not drawn"))
}

fn drawn(cx: &mut VisualTestContext, part: &str) -> bool {
    let name: &'static str = Box::leak(part.to_owned().into_boxed_str());
    cx.debug_bounds(name).is_some()
}

fn no_matches() -> Empty {
    Empty::new("No matching processes")
        .icon(Icon::Search)
        .description("Nothing matches that search.")
        .action(Button::new("clear").label("Clear search"))
        .action(Button::new("all").label("Show all"))
}

const TITLE: &str = "odk-empty/No matching processes";

/// Items 14, 15 and 27: it reports a status named by its title, with its description, so it is
/// said when a list empties.
#[test]
fn reports_a_status_named_by_its_title_with_its_description() {
    let a = no_matches().accessible();
    assert_eq!(
        (a.role, a.name.as_str()),
        (Role::Status, "No matching processes")
    );
    assert_eq!(
        a.description.as_deref(),
        Some("Nothing matches that search.")
    );
    assert_eq!(Empty::new("Bare").accessible().description, None);
}

/// Items 1 and 34: it draws its component tokens.
#[gpui::test]
fn draws_its_tokens(cx: &mut TestAppContext) {
    let _cx = open(cx, |_, _| v_flex().w(px(600.0)).child(no_matches()));
    let s = crate::drawn::last::<EmptyStyle>(TITLE);
    assert_eq!(s, crate::components().empty());
}

/// The icon's square, the title, the description and the actions are centred in the room it is
/// given, in that order, by its gaps, inside its padding.
#[gpui::test]
fn its_parts_are_centred_in_order_by_their_gaps(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| v_flex().w(px(600.0)).child(no_matches()));
    let s = crate::components().empty();
    let empty = bounds(cx, TITLE);
    let [media, title, description, actions] =
        ["media", "title", "description", "actions"].map(|p| bounds(cx, &format!("{TITLE}/{p}")));
    for (part, b) in [
        ("media", media),
        ("title", title),
        ("description", description),
        ("actions", actions),
    ] {
        let off = f32::from(b.center().x - empty.center().x).abs();
        assert!(off <= 0.5, "{part} is {off} px off the centre");
    }
    assert_eq!(f32::from(media.size.width), s.media_size);
    assert_eq!(f32::from(media.size.height), s.media_size);
    assert_eq!(
        f32::from(media.top() - empty.top()),
        s.padding,
        "padded above"
    );
    assert_eq!(f32::from(title.top() - media.bottom()), s.media_gap);
    assert_eq!(f32::from(description.top() - title.bottom()), s.gap);
    assert_eq!(
        f32::from(actions.top() - description.bottom()),
        s.actions_gap
    );
    assert_eq!(
        f32::from(empty.bottom() - actions.bottom()),
        s.padding,
        "padded below"
    );
}

/// Item 23: on a wide page its words wrap at its measure, centred; a long title wraps rather than
/// widening it; with no icon, description or action it is its title alone.
#[gpui::test]
fn its_words_keep_to_their_measure_and_the_bare_one_is_its_title(cx: &mut TestAppContext) {
    let long = "Keine passenden Prozesse gefunden, auch nicht nach dem Zurücksetzen aller Filter \
                und Suchbegriffe, die vorher in dieser Ansicht gesetzt waren 🔥";
    let cx = open(cx, move |_, _| {
        v_flex()
            .w(px(1400.0))
            .child(Empty::new(long).description(long))
            .child(Empty::new("Bare"))
    });
    let s = crate::components().empty();
    let empty = bounds(cx, &format!("odk-empty/{long}"));
    for part in ["title", "description"] {
        let b = bounds(cx, &format!("odk-empty/{long}/{part}"));
        assert!(
            f32::from(b.size.width) <= s.measure,
            "the {part} is {:?} wide, past {}",
            b.size.width,
            s.measure
        );
        let off = f32::from(b.center().x - empty.center().x).abs();
        assert!(off <= 0.5, "the {part} is {off} px off the centre");
        assert!(f32::from(b.size.height) > 30.0, "the {part} wraps");
    }
    for part in ["media", "description", "actions"] {
        assert!(!drawn(cx, &format!("odk-empty/Bare/{part}")), "{part}");
    }
    let (bare, title) = (
        bounds(cx, "odk-empty/Bare"),
        bounds(cx, "odk-empty/Bare/title"),
    );
    assert_eq!(f32::from(title.top() - bare.top()), s.padding);
    assert_eq!(f32::from(bare.bottom() - title.bottom()), s.padding);
}
