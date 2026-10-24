//! The form controls together: one height system with buttons, one corner radius, their
//! component tokens in every fixture theme, live theme changes, and what the select's list and a
//! growing textarea do. Each control's own behaviour is in its own module.

use std::fs;

use gpui::{Modifiers, ParentElement, Styled, TestAppContext, VisualTestContext, px};
use odk_theme::component::{Shape, States};
use odk_theme::foundation::{CornerRadius, Density};
use odk_theme::{JND, ThemeManager, ThemeSource};

use super::{FIXTURES, open, press, publish_fixture, redraw};
use crate::input::{drawn, probe};
use crate::{
    Button, Checkbox, Input, InputGroup, NumberInput, PasswordInput, RadioGroup, SearchInput,
    Select, Switch, Textarea, h_flex, v_flex,
};

fn bounds(cx: &mut VisualTestContext, part: String) -> gpui::Bounds<gpui::Pixels> {
    let name: &'static str = Box::leak(part.into_boxed_str());
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("no part named {name}"))
}

fn centre_y(b: gpui::Bounds<gpui::Pixels>) -> gpui::Pixels {
    b.origin.y + b.size.height / 2.0
}

/// A row of every one-line control at density `d`, ids suffixed with the density's name.
fn row(d: Density) -> gpui::Div {
    let n = d.name();
    let id = |what: &str| gpui::SharedString::from(format!("{what}-{n}"));
    h_flex()
        .items_center()
        .gap(px(8.0))
        .child(Button::new(id("button")).label("Save").density(d))
        .child(
            v_flex()
                .w(px(160.0))
                .child(Input::new(id("input"), "Name").density(d)),
        )
        .child(
            v_flex().w(px(160.0)).child(
                Select::new(id("select"), "Fruit")
                    .item("a", "Apple")
                    .density(d),
            ),
        )
        .child(
            v_flex().w(px(200.0)).child(
                InputGroup::new(id("group"), "Site")
                    .prefix("https://")
                    .density(d)
                    .action(Button::new(id("copy")).label("Copy")),
            ),
        )
        .child(
            v_flex().w(px(180.0)).child(
                SearchInput::new(id("search"), "Search")
                    .default_value("dock")
                    .density(d),
            ),
        )
        .child(
            v_flex().w(px(180.0)).child(
                PasswordInput::new(id("password"), "Password")
                    .default_value("x")
                    .density(d),
            ),
        )
        .child(
            v_flex()
                .w(px(180.0))
                .child(NumberInput::new(id("number"), "Quantity").density(d)),
        )
}

/// Buttons, inputs, selects, groups and the search, password and number fields share one
/// height system: exactly 28, 34 and 40 at the three densities, centred on one line, with the
/// buttons inside fields lower than them and inside their box.
#[gpui::test]
fn every_control_shares_the_height_system(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| v_flex().children(Density::ALL.map(row)));
    for d in Density::ALL {
        let n = d.name();
        let height = px(d.metrics().height);
        let parts = [
            format!("button-{n}/button"),
            format!("input-{n}/box"),
            format!("select-{n}/trigger"),
            format!("group-{n}/box"),
            format!("search-{n}/box"),
            format!("password-{n}/box"),
            format!("number-{n}/box"),
        ];
        let centre = centre_y(bounds(cx, parts[0].clone()));
        for part in parts {
            let b = bounds(cx, part.clone());
            assert_eq!(b.size.height, height, "{part}");
            assert_eq!(centre_y(b), centre, "{part} is off the row's centre");
        }
        // The buttons inside fields: lower than the field, inside it, and on its centre.
        for (field, button) in [
            (format!("group-{n}/box"), format!("copy-{n}/button")),
            (
                format!("search-{n}/box"),
                format!("search-{n}/clear/button"),
            ),
            (
                format!("password-{n}/box"),
                format!("password-{n}/reveal/button"),
            ),
            (
                format!("number-{n}/box"),
                format!("number-{n}/increment/button"),
            ),
        ] {
            let (f, b) = (bounds(cx, field), bounds(cx, button.clone()));
            assert!(b.top() > f.top() && b.bottom() < f.bottom(), "{button}");
            assert_eq!(centre_y(b), centre_y(f), "{button}");
        }
    }
}

/// Every rectangular form control takes the application's corners, 0 or exactly 2, never more;
/// a radio is a circle and a switch a pill, whatever the setting.
#[gpui::test]
fn corners_follow_the_application(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .child(Input::new("i", "Name"))
            .child(Textarea::new("t", "Notes"))
            .child(Select::new("s", "Fruit").item("a", "Apple"))
            .child(Checkbox::new("c", "Wrap"))
            .child(Switch::new("w", "Wi-Fi"))
            .child(RadioGroup::new("r", "Fruit").item("a", "Apple"))
    });
    for (corner, radius) in [(CornerRadius::Square, 0.0), (CornerRadius::Subtle, 2.0)] {
        cx.update(|_, cx| crate::configure(cx, |c| c.corner_radius = corner));
        redraw(cx);
        for field in ["i", "t", "s/trigger"] {
            assert_eq!(drawn::last(field).geometry.radius, radius, "{field}");
        }
        assert_eq!(
            crate::checkbox::drawn::last("c").geometry.shape,
            Shape::Rect { radius }
        );
        assert_eq!(
            crate::radio::drawn::last("r/radio-0").geometry.shape,
            Shape::Circle
        );
        let sw = crate::switch::drawn::last("w").style.geometry;
        assert_eq!((sw.track, sw.thumb_shape), (Shape::Pill, Shape::Circle));
    }
}

/// What each form control draws is its style from the component tokens, for its density and
/// state, in every fixture theme; in a monochrome theme they are all grey.
#[gpui::test]
fn they_draw_their_component_tokens_in_every_fixture(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .child(Input::new("i", "Name"))
            .child(Input::new("bad", "Name").invalid(true))
            .child(Input::new("ro", "Name").read_only(true))
            .child(Input::new("off", "Name").disabled(true))
            .child(Textarea::new("t", "Notes").rows(2))
            .child(Select::new("s", "Fruit").item("a", "Apple"))
            .child(Checkbox::new("c", "Wrap").default_checked(true))
            .child(Checkbox::new("m", "All").indeterminate(true))
            .child(Switch::new("w", "Wi-Fi").default_checked(true))
    });
    let d = Density::Default;
    for (theme, colors) in FIXTURES {
        publish_fixture(colors);
        redraw(cx);
        let c = crate::components();
        let fields = [
            ("i", c.field(d, States::NONE)),
            ("bad", c.field(d, States::NONE.invalid(true))),
            ("ro", c.field(d, States::NONE.read_only(true))),
            ("off", c.field(d, States::NONE.disabled(true))),
            ("t", c.textarea(d, 2, States::NONE)),
            ("s/trigger", c.select(d, States::NONE).trigger),
        ];
        let mut colours = Vec::new();
        for (id, want) in fields {
            let got = drawn::last(id);
            assert_eq!(got, want, "{theme} {id}");
            colours.extend([got.colours.default, got.colours.hover]);
        }
        let check = crate::checkbox::drawn::last("c");
        assert_eq!(check, c.checkbox(d, States::NONE.checked(true)), "{theme}");
        let mixed = crate::checkbox::drawn::last("m");
        assert_eq!(mixed, c.checkbox(d, States::NONE.indeterminate(true)));
        let switch = crate::switch::drawn::last("w").style;
        assert_eq!(switch, c.switch(d, States::NONE.checked(true)), "{theme}");
        colours.extend([check.current, mixed.current, switch.current]);
        if theme == "vantablack" {
            for t in colours {
                for colour in [t.background, t.foreground, t.border] {
                    assert!(colour.to_oklch().c <= JND, "{theme}: {colour:?} has a hue");
                }
            }
            assert!(
                drawn::last("bad").invalid_mark.is_some(),
                "a wrong value has a mark where its edge is grey"
            );
        }
    }
}

/// A theme change recolours the form controls on the next frame and keeps what is typed and
/// chosen, where focus is, the density and the corners.
#[gpui::test]
fn a_theme_change_keeps_values_focus_and_geometry(cx: &mut TestAppContext) {
    let [(_, dark), .., (_, light)] = FIXTURES;
    let cx = open(cx, |_, _| {
        v_flex()
            .child(Input::new("i", "Name"))
            .child(
                Select::new("s", "Fruit")
                    .item("a", "Apple")
                    .item("b", "Banana"),
            )
            .child(Checkbox::new("c", "Wrap"))
    });
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("colors.toml"), dark).unwrap();
    let mut manager = ThemeManager::new(ThemeSource::Dir(dir.path().to_owned()));
    manager.reload();
    cx.update(|window, cx| {
        crate::theme::follow(cx, manager, false);
        crate::configure(cx, |c| {
            c.density = Density::Compact;
            c.corner_radius = CornerRadius::Square;
        });
        window.refresh();
    });
    cx.run_until_parked();
    // Choose a fruit and check the box, then type into the field and leave focus there.
    press(cx, "tab tab down down enter");
    let check = bounds(cx, "c/box".into());
    cx.simulate_click(check.center(), Modifiers::default());
    press(cx, "shift-tab shift-tab");
    cx.simulate_input("Ada Lovelace");
    redraw(cx);
    let before = drawn::last("i");
    let boxes = [bounds(cx, "i/box".into()), bounds(cx, "s/trigger".into())];
    assert!(before.focus_visible);
    assert_eq!(boxes[0].size.height, px(28.0));

    fs::write(dir.path().join("colors.toml"), light).unwrap();
    cx.update(|_, cx| crate::reload_theme(cx));
    cx.run_until_parked();
    let after = drawn::last("i");
    assert_ne!(
        after.colours, before.colours,
        "recoloured without a restart"
    );
    assert_eq!(after.geometry, before.geometry, "same density and corners");
    assert_eq!(after.geometry.radius, 0.0);
    assert!(after.focus_visible, "focus stayed in the field");
    assert_eq!(probe(cx, "i").0, "Ada Lovelace", "the text is kept");
    assert_ne!(
        crate::checkbox::drawn::last("c").check,
        odk_theme::component::Check::Unchecked,
        "the box stays checked"
    );

    assert_eq!(
        [bounds(cx, "i/box".into()), bounds(cx, "s/trigger".into())],
        boxes
    );
    cx.simulate_input("!");
    assert_eq!(probe(cx, "i").0, "Ada Lovelace!");
    // The select kept its choice: its list opens with Banana chosen and Apple not.
    press(cx, "tab enter");
    redraw(cx);
    use crate::menu_list::drawn as rows;
    assert!(
        rows::last("s/row-1").indicator.is_some(),
        "Banana is still chosen"
    );
    assert!(rows::last("s/row-0").indicator.is_none());
}

/// Items 3, 32: a change of density, of corners or of reduced motion keeps what the user has
/// done: the text typed into a field and the focus in it, a chosen option, a checked box, and
/// the page a sidebar is on; none of them is built again, and a key pressed afterwards acts on
/// the same control.
#[gpui::test]
fn density_corners_and_reduced_motion_keep_values_focus_and_choices(cx: &mut TestAppContext) {
    use crate::{NavList, Tabs, Text};
    let cx = open(cx, |_, _| {
        v_flex()
            .child(Input::new("i", "Name"))
            .child(
                Select::new("s", "Fruit")
                    .item("a", "Apple")
                    .item("b", "Banana"),
            )
            .child(Checkbox::new("c", "Wrap"))
            .child(
                Tabs::new("t", "Sections")
                    .tab("a", "General", |_, _| Text::new("a"))
                    .tab("b", "Display", |_, _| Text::new("b")),
            )
            .child(
                NavList::new("n", "Settings")
                    .item("g", "General")
                    .item("m", "Members")
                    .default_value("g"),
            )
    });
    // Choose Banana, check the box, choose the Display tab, open Members, and then type into
    // the field, which keeps focus.
    press(
        cx,
        "tab tab down down enter tab space tab right tab down enter",
    );
    press(cx, "shift-tab shift-tab shift-tab shift-tab");
    cx.simulate_input("Ada");
    redraw(cx);
    assert!(drawn::last("i").focus_visible);
    let members = || crate::drawn::last::<odk_theme::component::NavItemStyle>("n/item-1");
    let tab = || crate::drawn::last::<(odk_theme::component::TabStyle, f32)>("t/tab-1").0;
    assert!(members().indicator.is_some() && tab().indicator.is_some());

    type Step = Box<dyn Fn(&mut gpui::App)>;
    let mut steps: Vec<Step> = Vec::new();
    for d in Density::ALL {
        steps.push(Box::new(move |cx| crate::configure(cx, |c| c.density = d)));
    }
    for r in [CornerRadius::Square, CornerRadius::Subtle] {
        steps.push(Box::new(move |cx| {
            crate::configure(cx, |c| c.corner_radius = r)
        }));
    }
    steps.push(Box::new(|cx| crate::set_reduced_motion(cx, true)));
    steps.push(Box::new(|cx| crate::set_reduced_motion(cx, false)));
    for (n, step) in steps.iter().enumerate() {
        cx.update(|_, cx| step(cx));
        redraw(cx);
        assert!(drawn::last("i").focus_visible, "step {n}: focus stayed");
        assert_eq!(probe(cx, "i").0, "Ada", "step {n}: the text is kept");
        assert!(
            members().indicator.is_some() && tab().indicator.is_some(),
            "step {n}: the page and the tab are kept"
        );
        assert_ne!(
            crate::checkbox::drawn::last("c").check,
            odk_theme::component::Check::Unchecked,
            "step {n}: the box stays checked"
        );
    }
    // The key goes to the same field after all of it.
    cx.simulate_input("!");
    assert_eq!(probe(cx, "i").0, "Ada!");
    press(cx, "tab enter");
    redraw(cx);
    use crate::menu_list::drawn as rows;
    assert!(rows::last("s/row-1").indicator.is_some(), "Banana is kept");
}

/// The select's list draws hovered, highlighted and selected apart: the chosen option keeps its
/// check mark when the keyboard moves on, and the option the keyboard is on is highlighted.
#[gpui::test]
fn the_select_list_tells_selected_from_highlighted(cx: &mut TestAppContext) {
    use crate::menu_list::drawn as rows;
    let cx = open(cx, |_, _| {
        Select::new("s", "Fruit")
            .item("a", "Apple")
            .item("b", "Banana")
            .item("c", "Cherry")
            .disable("c")
            .default_value("b")
    });
    press(cx, "tab enter");
    redraw(cx);
    let chosen = rows::last("s/row-1");
    assert!(chosen.indicator.is_some(), "the chosen option has its mark");
    let c = crate::components();
    let d = c.density(None);
    assert_eq!(
        chosen,
        c.menu_item(d, States::NONE.selected(true).highlighted(true), false),
        "and is where the keyboard is"
    );
    press(cx, "up");
    redraw(cx);
    assert_eq!(
        rows::last("s/row-1"),
        c.menu_item(d, States::NONE.selected(true), false),
        "still chosen, no longer highlighted"
    );
    assert_eq!(
        rows::last("s/row-0"),
        c.menu_item(d, States::NONE.highlighted(true), false)
    );
    assert!(!rows::last("s/row-2").responds, "a disabled option");
    assert_eq!(
        bounds(cx, "s/row-0".into()).size.height,
        px(d.metrics().height),
        "options are a control's height"
    );
    press(cx, "escape");
    redraw(cx);
    assert!(
        drawn::last("s/trigger").focus_visible,
        "focus came back to the box"
    );
}

/// A textarea with `max_rows` grows a row at a time with its text, from its rows up to its
/// most, then scrolls; its height is always rows of the field's line and its padding.
#[gpui::test]
fn a_textarea_grows_between_its_rows(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .w(px(240.0))
            .child(Textarea::new("t", "Notes").rows(2).max_rows(4))
            .child(Button::new("after").label("After"))
    });
    let c = crate::components();
    let height = |rows| {
        px(c.textarea(Density::Default, rows, States::NONE)
            .geometry
            .height)
    };
    let field = |cx: &mut VisualTestContext| bounds(cx, "t/box".into()).size.height;
    assert_eq!(field(cx), height(2));
    press(cx, "tab");
    cx.simulate_input("one\ntwo\nthree");
    redraw(cx);
    redraw(cx);
    assert_eq!(field(cx), height(3));
    cx.simulate_input("\nfour\nfive\nsix");
    redraw(cx);
    redraw(cx);
    assert_eq!(field(cx), height(4), "no higher than its most");
    press(cx, "ctrl-a backspace");
    redraw(cx);
    redraw(cx);
    assert_eq!(field(cx), height(2), "back to its rows");
    // Without `max_rows` it keeps its rows whatever it holds.
    assert_eq!(height(2), px(2.0 * 20.0 + 2.0 * (6.0 + 1.0)));
}

/// A field's icons are the density's icon size, a gap from its text, in its icon colour; a
/// read-only field takes no hover and a disabled one draws its placeholder disabled.
#[gpui::test]
fn icons_and_pointer_states_of_a_field(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .w(px(240.0))
            .child(
                Input::new("i", "Amount")
                    .leading_icon("$")
                    .trailing_icon("€")
                    .placeholder("0"),
            )
            .child(Input::new("ro", "Amount").read_only(true))
            .child(Input::new("off", "Amount").disabled(true).placeholder("0"))
    });
    let s = drawn::last("i");
    let g = s.geometry;
    let (field, lead, trail) = (
        bounds(cx, "i/box".into()),
        bounds(cx, "i/leading-icon".into()),
        bounds(cx, "i/trailing-icon".into()),
    );
    assert_eq!(lead.size, gpui::size(px(g.icon_size), px(g.icon_size)));
    assert_eq!(lead.left(), field.left() + px(g.border_width + g.padding_x));
    assert_eq!(
        trail.right(),
        field.right() - px(g.border_width + g.padding_x)
    );
    assert_eq!(centre_y(lead), centre_y(field));
    let placeholder = bounds(cx, "i/placeholder".into());
    assert_eq!(placeholder.left(), lead.right() + px(g.gap));
    assert!(!drawn::last("ro").responds, "read-only takes no hover");
    let semantic = crate::components().semantic().clone();
    assert_eq!(drawn::last("off").placeholder, semantic.text.disabled);
    assert_eq!(s.icon, semantic.text.tertiary);
}
