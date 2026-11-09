//! `Badge`, `StatusIndicator`, `Empty`, `Item` and `ItemGroup` on the Rune host. Nothing here
//! opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_theme::component::ItemGroupVariant;
use odk_view::{
    Badge, Empty, Face, Flow, Glyph, Hosted, Icon, Item, ItemGroup, Status, StatusIndicator, Tone,
    frame, headless,
};
use rune::{Cx, El, Runtime, col, el, row};

fn face() -> Face {
    static FACE: OnceLock<Face> = OnceLock::new();
    *FACE.get_or_init(|| {
        let output = std::process::Command::new("fc-match")
            .args(["-f", "%{file}", "sans"])
            .output()
            .expect("fc-match");
        assert!(output.status.success(), "fc-match sans");
        let path = String::from_utf8(output.stdout).expect("fc-match speaks utf-8");
        let bytes = std::fs::read(path.trim()).expect("read the sans font");
        Face::leak(bytes, "Inter".into())
    })
}

type View = fn(&Hosted<()>, &mut Cx<Hosted<()>>) -> El;

fn open(view: View, size: (f32, f32)) -> Runtime<Hosted<()>> {
    headless(
        Hosted::new((), ThemeSnapshot::emergency(), face()),
        view,
        size,
    )
}

fn rect(runtime: &Runtime<Hosted<()>>, key: &str) -> [f32; 4] {
    runtime
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no element {key}"))
        .rect
}

fn nodes(runtime: &Runtime<Hosted<()>>) -> Vec<serde_json::Value> {
    fn walk(node: &serde_json::Value, into: &mut Vec<serde_json::Value>) {
        into.push(node.clone());
        for child in node["children"].as_array().into_iter().flatten() {
            walk(child, into);
        }
    }
    let mut all = Vec::new();
    walk(&runtime.accessibility_json(), &mut all);
    all
}

fn find<'a>(all: &'a [serde_json::Value], role: &str, name: &str) -> &'a serde_json::Value {
    all.iter()
        .find(|node| node["role"] == role && node["name"] == name)
        .unwrap_or_else(|| panic!("no {role} named {name}: {all:#?}"))
}

fn describes(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    frame(
        hosted,
        cx,
        col()
            .child(Badge::new("Beta").show(hosted))
            .child(StatusIndicator::new(Status::Active, "Online").show(hosted))
            .child(
                Empty::new("No results")
                    .description("Try another search.")
                    .icon(Icon::Search)
                    .show(hosted),
            )
            .child(
                Item::new("Notifications")
                    .description("When something happens.")
                    .show(hosted),
            )
            .child(
                ItemGroup::new("Devices")
                    .description("Where you are signed in.")
                    .child(Item::new("Laptop").show(hosted))
                    .show(hosted),
            ),
        [],
    )
}

#[test]
fn each_one_describes_itself_to_the_tree() {
    let runtime = open(describes, (600.0, 700.0));
    let all = nodes(&runtime);
    find(&all, "label", "Beta");
    find(&all, "label", "Online");
    let empty = find(&all, "status", "No results");
    assert_eq!(empty["description"], "Try another search.");
    let item = find(&all, "group", "Notifications");
    assert_eq!(item["description"], "When something happens.");
    let group = find(&all, "group", "Devices");
    assert_eq!(group["description"], "Where you are signed in.");
    // What a component says of itself is said once: its words are not a second node.
    for repeated in ["No results", "Notifications", "Devices"] {
        assert_eq!(
            all.iter().filter(|node| node["name"] == repeated).count(),
            1,
            "{repeated} is in the tree more than once"
        );
    }
    for description in ["Try another search.", "When something happens."] {
        assert!(
            all.iter().all(|node| node["name"] != description),
            "{description} is a node of its own as well as a description"
        );
    }
}

fn badges(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let badge = |key: &str, badge: Badge| row().child(badge.show(hosted).key(key));
    frame(
        hosted,
        cx,
        col()
            .child(badge("plain", Badge::new("Beta")))
            .child(badge(
                "marked",
                Badge::new("Beta").tone(Tone::Warning).marked(),
            ))
            .child(badge("outlined", Badge::new("Beta").outlined()))
            .child(badge("glyph", Badge::new("Beta").icon("★")))
            .child(badge(
                "icon",
                Badge::new("Beta").icon(Glyph::Icon(Icon::Check)),
            ))
            .child(
                col().key("long-column").w(60.0).child(
                    Badge::new("A badge with a label far too long for its column").show(hosted),
                ),
            )
            .child(
                row().key("row-column").w(60.0).child(
                    Badge::new("A badge with a label far too long for its column").show(hosted),
                ),
            )
            .child(
                col().key("wrapped-column").w(60.0).child(
                    Badge::new("A badge with a label far too long for its column")
                        .flow(Flow::Wrap)
                        .show(hosted),
                ),
            ),
        [],
    )
}

#[test]
fn a_badge_is_as_wide_as_its_label_and_a_mark_or_an_edge_changes_nothing_else() {
    let runtime = open(badges, (600.0, 400.0));
    let plain = rect(&runtime, "plain");
    let marked = rect(&runtime, "marked");
    let outlined = rect(&runtime, "outlined");
    assert!(plain[2] < 120.0, "as wide as its label: {plain:?}");
    assert_eq!(plain[3], marked[3], "every badge is as high as the next");
    assert_eq!(plain[3], outlined[3]);
    assert_eq!(plain[2], outlined[2], "an edge changes no size");
    assert!(marked[2] > plain[2], "a status mark takes room");
    assert!(rect(&runtime, "glyph")[2] > plain[2]);
    assert!(rect(&runtime, "icon")[2] > plain[2]);
}

#[test]
fn a_badge_narrower_than_its_label_is_cut_unless_it_may_wrap() {
    let runtime = open(badges, (600.0, 400.0));
    // The label is what runs out of a column, so it is the label's edge that is held to it.
    let label = |column: &str| {
        let column = rect(&runtime, column);
        let text = runtime
            .elements()
            .into_iter()
            .filter(|e| e.text.as_deref().is_some_and(|t| t.starts_with("A badge")))
            .find(|e| e.rect[1] >= column[1] && e.rect[1] < column[1] + column[3])
            .expect("the label");
        (column, text.rect)
    };
    let (column, long) = label("long-column");
    assert!(
        long[0] + long[2] <= column[0] + column[2],
        "the label overflows: {long:?}"
    );
    let one_line = runtime
        .elements()
        .into_iter()
        .find(|e| e.text.as_deref() == Some("Beta"))
        .expect("a one-line label")
        .rect[3];
    assert_eq!(long[3], one_line, "cut to one line");
    let (column, in_a_row) = label("row-column");
    assert!(
        in_a_row[0] + in_a_row[2] <= column[0] + column[2],
        "in a row the label overflows: {in_a_row:?}"
    );
    let (column, wrapped) = label("wrapped-column");
    assert!(
        wrapped[0] + wrapped[2] <= column[0] + column[2],
        "{wrapped:?}"
    );
    assert!(wrapped[3] > long[3], "wrapped onto more lines");
}

fn statuses(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let line = |key: &str, status: Status, label: &str| {
        row().child(StatusIndicator::new(status, label).show(hosted).key(key))
    };
    frame(
        hosted,
        cx,
        col()
            .child(line("on", Status::Active, "Online"))
            .child(line("off", Status::Inactive, "Offline"))
            .child(
                col().key("column").w(80.0).child(
                    StatusIndicator::new(
                        Status::Error,
                        "A build that failed for a reason with a long name",
                    )
                    .show(hosted),
                ),
            ),
        [],
    )
}

#[test]
fn a_status_is_a_dot_and_its_label_and_is_cut_in_a_narrow_column() {
    let runtime = open(statuses, (400.0, 300.0));
    let on = rect(&runtime, "on");
    let off = rect(&runtime, "off");
    assert_eq!(on[3], off[3]);
    assert!(on[2] > 20.0, "a dot and a label: {on:?}");
    let column = rect(&runtime, "column");
    let label = runtime
        .elements()
        .into_iter()
        .find(|e| e.text.as_deref().is_some_and(|t| t.starts_with("A build")))
        .expect("the label")
        .rect;
    assert!(
        label[0] + label[2] <= column[0] + column[2],
        "the label overflows its column: {label:?}"
    );
    let one_line = runtime
        .elements()
        .into_iter()
        .find(|e| e.text.as_deref() == Some("Online"))
        .expect("a one-line label")
        .rect[3];
    assert_eq!(label[3], one_line, "cut to one line, not wrapped");
}

fn empties(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    frame(
        hosted,
        cx,
        col().w_full().child(
            Empty::new("No matching processes")
                .description(
                    "Nothing matches that search, however it is spelled, so there is nothing \
                     to show here until the search is made a little wider than it is now.",
                )
                .icon(Icon::Search)
                .action(el().key("clear").size(60.0, 24.0))
                .action(el().key("reload").size(60.0, 24.0))
                .show(hosted)
                .key("empty"),
        ),
        [],
    )
}

#[test]
fn an_empty_state_is_centred_with_its_words_in_a_measure_and_its_actions_below() {
    let runtime = open(empties, (1000.0, 600.0));
    let empty = rect(&runtime, "empty");
    let centre = empty[0] + empty[2] / 2.0;
    let (clear, reload) = (rect(&runtime, "clear"), rect(&runtime, "reload"));
    let actions_centre = (clear[0] + reload[0] + reload[2]) / 2.0;
    assert!((actions_centre - centre).abs() < 1.5, "actions are centred");
    assert!(
        clear[1] == reload[1] && reload[0] > clear[0],
        "actions side by side"
    );
    let all = runtime.elements();
    let description = all
        .iter()
        .find(|e| {
            e.text
                .as_deref()
                .is_some_and(|t| t.starts_with("Nothing matches"))
        })
        .expect("the description");
    assert!(
        description.rect[2] < 0.6 * empty[2],
        "the words keep a measure on a wide page: {} of {}",
        description.rect[2],
        empty[2]
    );
    assert!(
        description.rect[1] < clear[1],
        "actions come after the words"
    );
    let title = all
        .iter()
        .find(|e| e.text.as_deref() == Some("No matching processes"))
        .expect("the title");
    let title_centre = title.rect[0] + title.rect[2] / 2.0;
    assert!((title_centre - centre).abs() < 1.5, "the title is centred");
}

fn items(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    frame(
        hosted,
        cx,
        col().w(300.0).child(
            Item::new("A setting whose title is far too long to sit beside its control")
                .description("What it does.")
                .child(el().key("line").size(40.0, 10.0))
                .action(el().key("action").size(100.0, 24.0))
                .action(Badge::new("On").show(hosted).key("on"))
                .footer(el().key("footer").h(20.0))
                .show(hosted)
                .key("item"),
        ),
        [],
    )
}

#[test]
fn an_item_keeps_its_actions_at_its_end_and_wraps_its_words_in_what_is_left() {
    let runtime = open(items, (600.0, 400.0));
    let item = rect(&runtime, "item");
    let action = rect(&runtime, "action");
    assert_eq!(action[2], 100.0, "an action keeps its own width");
    let on = rect(&runtime, "on");
    assert_eq!(
        on[0] + on[2],
        item[0] + item[2],
        "the last action is at the item's end"
    );
    assert!(action[0] + action[2] < on[0], "after the one before it");
    let all = runtime.elements();
    let title = all
        .iter()
        .find(|e| {
            e.text
                .as_deref()
                .is_some_and(|t| t.starts_with("A setting"))
        })
        .expect("the title");
    assert!(
        title.rect[0] + title.rect[2] <= action[0],
        "the title wraps beside the action"
    );
    // An action keeps its own height: a badge beside tall words is no taller than any badge.
    let badge_height = open(badges, (600.0, 400.0));
    assert_eq!(rect(&runtime, "on")[3], rect(&badge_height, "plain")[3]);
    let (line, footer) = (rect(&runtime, "line"), rect(&runtime, "footer"));
    assert!(line[1] > title.rect[1], "a line comes under the words");
    assert_eq!(footer[2], item[2], "a footer spans the item");
    assert!(footer[1] > line[1] + line[3], "and sits under its words");
}

fn groups(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let group = |variant: ItemGroupVariant, key: &str| {
        row().child(
            col().w(300.0).child(
                ItemGroup::new("Devices")
                    .variant(variant)
                    .child(Item::new("One").show(hosted))
                    .child(Item::new("Two").show(hosted))
                    .child(Item::new("Three").show(hosted))
                    .show(hosted)
                    .key(key),
            ),
        )
    };
    frame(
        hosted,
        cx,
        col()
            .child(group(ItemGroupVariant::Default, "plain"))
            .child(group(ItemGroupVariant::Outline, "outline")),
        [],
    )
}

#[test]
fn a_group_puts_a_line_between_items_and_only_an_outline_pads_its_words() {
    let runtime = open(groups, (700.0, 700.0));
    let lines = runtime
        .elements()
        .iter()
        .filter(|e| e.role == "separator")
        .count();
    assert_eq!(lines, 4, "two between three items, in each of two groups");
    let all = runtime.elements();
    let title_x = |nth: usize| {
        all.iter()
            .filter(|e| e.text.as_deref() == Some("Devices"))
            .nth(nth)
            .expect("a title")
            .rect[0]
    };
    let (plain, outline) = (rect(&runtime, "plain"), rect(&runtime, "outline"));
    assert_eq!(title_x(0), plain[0], "no padding by default");
    assert!(title_x(1) > outline[0], "an outline pads what is inside it");
}
