use std::cell::Cell;
use std::fs;
use std::rc::Rc;

use gpui::{
    Bounds, KeyUpEvent, Keystroke, Modifiers, MouseButton, ParentElement, Pixels, Styled,
    TestAppContext, VisualTestContext, div, point, px,
};
use odk_a11y::{Describe, Role};
use odk_theme::component::{ButtonLayout, ButtonVariant, States};
use odk_theme::foundation::{CornerRadius, Density, MotionEasing};
use odk_theme::semantic::Emphasis;
use odk_theme::{JND, ThemeManager, ThemeSource};

use super::{Count, FIXTURES, open, press, publish_fixture, redraw, reduce_motion};
use crate::button::probe;
use crate::{Button, Size, h_flex, v_flex};

fn bounds(cx: &mut VisualTestContext, part: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(part)
        .unwrap_or_else(|| panic!("no part named {part}"))
}

fn centre_y(b: Bounds<Pixels>) -> Pixels {
    b.origin.y + b.size.height / 2.0
}

fn centre(b: Bounds<Pixels>) -> gpui::Point<Pixels> {
    point(b.origin.x + b.size.width / 2.0, centre_y(b))
}

fn part(id: &str, part: &str) -> &'static str {
    Box::leak(format!("{id}/{part}").into_boxed_str())
}

/// Items 8, 9, 10: Tab reaches the button, and Enter and Space press it.
#[gpui::test]
fn tab_reaches_it_and_enter_and_space_press_it(cx: &mut TestAppContext) {
    let clicks = Count::default();
    let c = clicks.clone();
    let cx = open(cx, move |_, _| {
        let c = c.clone();
        Button::new("b")
            .label("Go")
            .on_click(move |_, _, _| c.bump())
    });
    press(cx, "enter");
    assert_eq!(clicks.get(), 0, "nothing has focus yet");
    press(cx, "tab enter space");
    assert_eq!(clicks.get(), 2);
}

/// Item 9: each button is one tab stop, in order.
#[gpui::test]
fn each_button_is_one_tab_stop(cx: &mut TestAppContext) {
    let (first, second) = (Count::default(), Count::default());
    let (a, b) = (first.clone(), second.clone());
    let cx = open(cx, move |_, _| {
        let (a, b) = (a.clone(), b.clone());
        h_flex()
            .child(
                Button::new("a")
                    .label("A")
                    .on_click(move |_, _, _| a.bump()),
            )
            .child(
                Button::new("b")
                    .label("B")
                    .on_click(move |_, _, _| b.bump()),
            )
    });
    press(cx, "tab tab enter");
    assert_eq!((first.get(), second.get()), (0, 1));
    press(cx, "shift-tab enter");
    assert_eq!((first.get(), second.get()), (1, 1));
}

/// Item 7: a loading button ignores presses and reports itself busy.
#[gpui::test]
fn a_loading_button_ignores_presses(cx: &mut TestAppContext) {
    let clicks = Count::default();
    let c = clicks.clone();
    let cx = open(cx, move |_, _| {
        let c = c.clone();
        Button::new("b")
            .label("Save")
            .loading(true)
            .on_click(move |_, _, _| c.bump())
    });
    press(cx, "tab enter space");
    let button = bounds(cx, "b/button");
    cx.simulate_click(centre(button), Modifiers::default());
    assert_eq!(clicks.get(), 0);
    assert!(
        Button::new("b")
            .label("Save")
            .loading(true)
            .accessible()
            .busy
    );
}

/// The click test above can only pass if a click at the corner reaches the first button.
#[gpui::test]
fn a_click_at_the_corner_reaches_the_first_button(cx: &mut TestAppContext) {
    let clicks = Count::default();
    let c = clicks.clone();
    let cx = open(cx, move |_, _| {
        let c = c.clone();
        Button::new("b")
            .label("Go")
            .on_click(move |_, _, _| c.bump())
    });
    cx.simulate_click(point(px(4.0), px(4.0)), Modifiers::default());
    assert_eq!(clicks.get(), 1);
}

/// Items 14, 15: a button is a Button named by its label, or by `accessible_name` when the
/// label is only a glyph; an icon-only button takes its name in its constructor.
#[test]
fn reports_its_role_and_name() {
    let a = Button::new("b").label("Save").accessible();
    assert_eq!((a.role, a.name.as_str()), (Role::Button, "Save"));
    let glyph = Button::new("x")
        .label("×")
        .accessible_name("Close")
        .accessible();
    assert_eq!(glyph.name, "Close");
    let icon = Button::icon("s", "⚙", "Settings").accessible();
    assert_eq!((icon.role, icon.name.as_str()), (Role::Button, "Settings"));
}

/// Item 15: an icon with no label and no name would be an unnamed button, so development builds
/// refuse to draw one.
#[gpui::test]
#[should_panic(expected = "no accessible name")]
fn an_icon_alone_without_a_name_is_refused(cx: &mut TestAppContext) {
    open(cx, |_, _| Button::new("b").leading_icon("⚙"));
}

/// Item 6: a disabled button ignores Enter and Space, is skipped by Tab, and reports itself
/// disabled.
#[gpui::test]
fn a_disabled_button_is_skipped_and_ignores_presses(cx: &mut TestAppContext) {
    let (off, on) = (Count::default(), Count::default());
    let (a, b) = (off.clone(), on.clone());
    let cx = open(cx, move |_, _| {
        let (a, b) = (a.clone(), b.clone());
        h_flex()
            .child(
                Button::new("off")
                    .label("Off")
                    .disabled(true)
                    .on_click(move |_, _, _| a.bump()),
            )
            .child(
                Button::new("on")
                    .label("On")
                    .on_click(move |_, _, _| b.bump()),
            )
    });
    press(cx, "tab enter space");
    assert_eq!(
        (off.get(), on.get()),
        (0, 2),
        "Tab went past the disabled button"
    );
    // The disabled button is first in the window, at its top-left corner.
    cx.simulate_click(point(px(4.0), px(4.0)), Modifiers::default());
    assert_eq!(off.get(), 0, "a click does nothing");
    let a = Button::new("b").label("Save").disabled(true).accessible();
    assert!(a.disabled);
    let drawn = probe::last("off").style;
    assert!(!drawn.responds && !drawn.focus_visible);
}

/// Item 32: focusing, hovering, pressing, loading and disabling a button moves nothing, with
/// icons too.
#[gpui::test]
fn no_state_moves_anything(cx: &mut TestAppContext) {
    use super::stability::{Step, assert_still};
    let (loading, off) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
    let (l, o) = (loading.clone(), off.clone());
    let cx = open(cx, move |_, _| {
        h_flex()
            .child(
                Button::new("b")
                    .label("Save")
                    .loading(l.get())
                    .disabled(o.get()),
            )
            .child(
                Button::new("i")
                    .leading_icon("✓")
                    .label("Apply")
                    .trailing_icon("→")
                    .loading(l.get()),
            )
            .child(Button::new("after").label("After"))
    });
    let (start, disable) = (loading.clone(), off.clone());
    assert_still(
        cx,
        &[
            "b/button",
            "b/label",
            "i/button",
            "i/icon",
            "i/label",
            "i/trailing-icon",
            "after/button",
        ],
        &[
            Step::Keys("tab"),
            Step::Hover("b/button"),
            Step::Press("b/button"),
            Step::Hover("i/button"),
            Step::Press("i/button"),
            Step::Keys("tab"),
            Step::Change("disabling it", Box::new(move || disable.set(true))),
            Step::Change("loading", Box::new(move || start.set(true))),
        ],
    );
}

/// Item 32: while loading, a button without an icon lays the spinner over its whole inside, so
/// it is centred on the label's place and takes no space of its own; one with an icon puts the
/// spinner in the icon's place and keeps its label.
#[gpui::test]
fn the_spinner_takes_the_icon_s_place_or_covers_the_label(cx: &mut TestAppContext) {
    let cx = open(cx, move |_, _| {
        h_flex()
            .child(Button::new("b").label("Save").loading(true))
            .child(
                Button::new("i")
                    .leading_icon("✓")
                    .label("Saving…")
                    .loading(true),
            )
            .child(
                Button::new("t")
                    .label("Continue")
                    .trailing_icon("→")
                    .loading(true),
            )
    });
    reduce_motion();
    redraw(cx);
    let button = bounds(cx, "b/button");
    let spinner = bounds(cx, "b/spinner");
    let border = px(crate::foundation().geometry.border_width);
    assert_eq!(spinner.origin.x, button.origin.x + border);
    assert_eq!(spinner.origin.y, button.origin.y + border);
    assert_eq!(spinner.size.width, button.size.width - border * 2.0);
    assert_eq!(spinner.size.height, button.size.height - border * 2.0);
    assert_eq!(bounds(cx, "i/spinner"), bounds(cx, "i/icon"));
    assert_eq!(bounds(cx, "t/spinner"), bounds(cx, "t/trailing-icon"));
    assert_eq!(probe::last("i").busy, 1.0);
}

// Geometry: the density table of ADR 0040, squares and corners.

/// A button is as high as its density: 28, 34 or 40. One without a density of its own takes the
/// application's, and its `Size` steps from there; its own density wins over both.
#[gpui::test]
fn each_density_has_its_height_and_a_button_s_own_wins(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .child(
                Button::new("compact")
                    .label("Save")
                    .density(Density::Compact),
            )
            .child(
                Button::new("default")
                    .label("Save")
                    .density(Density::Default),
            )
            .child(
                Button::new("comfortable")
                    .label("Save")
                    .density(Density::Comfortable),
            )
            .child(Button::new("app").label("Save"))
            .child(Button::new("sm").label("Save").size(Size::Sm))
            .child(Button::new("lg").label("Save").size(Size::Lg))
            .child(
                Button::new("own")
                    .label("Save")
                    .size(Size::Lg)
                    .density(Density::Compact),
            )
    });
    let heights = |cx: &mut VisualTestContext| {
        [
            "compact",
            "default",
            "comfortable",
            "app",
            "sm",
            "lg",
            "own",
        ]
        .map(|id| f32::from(bounds(cx, part(id, "button")).size.height))
    };
    assert_eq!(heights(cx), [28.0, 34.0, 40.0, 34.0, 28.0, 40.0, 28.0]);

    cx.update(|_, cx| crate::configure(cx, |c| c.density = Density::Compact));
    redraw(cx);
    assert_eq!(
        heights(cx),
        [28.0, 34.0, 40.0, 28.0, 28.0, 34.0, 28.0],
        "the application's density moves only the buttons without their own"
    );
    cx.update(|_, cx| crate::configure(cx, |c| c.density = Density::Comfortable));
    redraw(cx);
    assert_eq!(heights(cx), [28.0, 34.0, 40.0, 40.0, 34.0, 40.0, 28.0]);
}

/// Icons are the density's icon size, a gap from the label, and centred on the same line as
/// the label, which is centred in the button; the content starts at the padding.
#[gpui::test]
fn icons_and_label_share_one_centred_line(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex().children(Density::ALL.map(|d| {
            Button::new(d.name())
                .leading_icon("✓")
                .label("Save")
                .trailing_icon("→")
                .density(d)
        }))
    });
    let border = px(crate::foundation().geometry.border_width);
    for d in Density::ALL {
        let m = d.metrics();
        let id = d.name();
        let button = bounds(cx, part(id, "button"));
        let icon = bounds(cx, part(id, "icon"));
        let label = bounds(cx, part(id, "label"));
        let trailing = bounds(cx, part(id, "trailing-icon"));
        for b in [icon, trailing] {
            assert_eq!(b.size, gpui::size(px(m.icon_size), px(m.icon_size)), "{id}");
        }
        assert_eq!(
            icon.left(),
            button.left() + border + px(m.padding_x),
            "{id}"
        );
        assert_eq!(icon.right() + px(m.gap), label.left(), "{id}");
        assert_eq!(label.right() + px(m.gap), trailing.left(), "{id}");
        assert_eq!(trailing.right() + px(m.padding_x) + border, button.right());
        for b in [icon, label, trailing] {
            assert!(
                (centre_y(b) - centre_y(button)).abs() <= px(0.5),
                "{id}: {b:?} is off the middle of {button:?}"
            );
        }
    }
}

/// An icon-only button is a square as high as its density, the icon in the middle.
#[gpui::test]
fn icon_only_buttons_are_squares(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        h_flex().children(Density::ALL.map(|d| Button::icon(d.name(), "⚙", "Settings").density(d)))
    });
    for d in Density::ALL {
        let id = d.name();
        let button = bounds(cx, part(id, "button"));
        let icon = bounds(cx, part(id, "icon"));
        let side = px(d.metrics().height);
        assert_eq!(button.size, gpui::size(side, side), "{id}");
        assert_eq!(icon.left() - button.left(), button.right() - icon.right());
        assert_eq!(icon.top() - button.top(), button.bottom() - icon.bottom());
        assert_eq!(probe::last(id).style.layout, ButtonLayout::IconOnly);
    }
}

/// A button keeps its content's width unless told to fill its space; then a label that does
/// not fit ends in `…` inside the button rather than leaving it.
#[gpui::test]
fn a_long_label_truncates_only_in_a_width_it_was_given(cx: &mut TestAppContext) {
    const LONG: &str = "Änderungen speichern und fortfahren";
    let cx = open(cx, |_, _| {
        v_flex()
            .child(
                div()
                    .w(px(120.0))
                    .child(Button::new("fill").label(LONG).full_width()),
            )
            .child(Button::new("natural").label(LONG))
    });
    let fill = bounds(cx, "fill/button");
    let label = bounds(cx, "fill/label");
    let natural = bounds(cx, "natural/label");
    assert_eq!(fill.size.width, px(120.0));
    assert!(label.right() <= fill.right(), "{label:?} leaves {fill:?}");
    assert!(label.size.width < natural.size.width);
    assert_eq!(fill.size.height, px(34.0), "it stays one line high");
}

/// Corners are the application's: exactly 2 px by default and 0 when it asks for square, for
/// every variant and density; no button rounds more.
#[gpui::test]
fn corners_follow_the_application(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        h_flex()
            .children(ButtonVariant::ALL.map(|v| Button::new(v.name()).label("Go").variant(v)))
            .children(Density::ALL.map(|d| Button::icon(d.name(), "⚙", "Settings").density(d)))
    });
    let ids: Vec<&str> = ButtonVariant::ALL
        .iter()
        .map(|v| v.name())
        .chain(Density::ALL.iter().map(|d| d.name()))
        .collect();
    for (corner, radius) in [(CornerRadius::Square, 0.0), (CornerRadius::Subtle, 2.0)] {
        cx.update(|_, cx| crate::configure(cx, |c| c.corner_radius = corner));
        redraw(cx);
        for id in &ids {
            assert_eq!(probe::last(id).style.geometry.radius, radius, "{id}");
        }
    }
}

// Pointer and keyboard.

/// A larger target takes the pointer around the button, which is drawn at its own size in the
/// middle of it; a click anywhere on it presses the button once, never twice.
#[gpui::test]
fn a_larger_target_takes_the_pointer_and_presses_once(cx: &mut TestAppContext) {
    let clicks = Count::default();
    let c = clicks.clone();
    let cx = open(cx, move |_, _| {
        let c = c.clone();
        h_flex().child(
            Button::icon("t", "⚙", "Settings")
                .density(Density::Compact)
                .target_size(44.0)
                .on_click(move |_, _, _| c.bump()),
        )
    });
    let target = bounds(cx, "t/target");
    let face = bounds(cx, "t/button");
    assert_eq!(target.size, gpui::size(px(44.0), px(44.0)));
    assert_eq!(
        face.size,
        gpui::size(px(28.0), px(28.0)),
        "drawn at its own size"
    );
    assert_eq!(centre(face), centre(target));

    let corner = target.origin + point(px(2.0), px(2.0));
    assert!(!face.contains(&corner));
    cx.simulate_click(corner, Modifiers::default());
    assert_eq!(clicks.get(), 1, "a click beside the face presses it");
    cx.simulate_click(centre(face), Modifiers::default());
    assert_eq!(clicks.get(), 2, "a click on the face presses it once");
    press(cx, "tab enter");
    assert_eq!(clicks.get(), 3, "the target is the tab stop");
}

/// One press, one action: a click, Enter and Space each press once, and a key held down presses
/// once, when it is released.
#[gpui::test]
fn each_press_acts_once(cx: &mut TestAppContext) {
    let clicks = Count::default();
    let c = clicks.clone();
    let cx = open(cx, move |_, _| {
        let c = c.clone();
        Button::new("b")
            .label("Go")
            .on_click(move |_, _, _| c.bump())
    });
    let button = bounds(cx, "b/button");
    cx.simulate_click(centre(button), Modifiers::default());
    assert_eq!(clicks.get(), 1);
    press(cx, "tab enter");
    assert_eq!(clicks.get(), 2);
    press(cx, "space");
    assert_eq!(clicks.get(), 3);
    let space = Keystroke::parse("space").unwrap();
    for _ in 0..3 {
        let space = space.clone();
        cx.update(move |window, cx| {
            window.dispatch_keystroke(space, cx);
        });
    }
    cx.run_until_parked();
    assert_eq!(clicks.get(), 3, "a key repeating does not press it");
    cx.simulate_event(KeyUpEvent { keystroke: space });
    cx.run_until_parked();
    assert_eq!(clicks.get(), 4);
}

/// A press that is released off the button does nothing, and the next click still works.
#[gpui::test]
fn a_press_released_elsewhere_does_nothing(cx: &mut TestAppContext) {
    let clicks = Count::default();
    let c = clicks.clone();
    let cx = open(cx, move |_, _| {
        let c = c.clone();
        Button::new("b")
            .label("Go")
            .on_click(move |_, _, _| c.bump())
    });
    let button = bounds(cx, "b/button");
    let away = point(px(400.0), px(300.0));
    cx.simulate_mouse_down(centre(button), MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(away, Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_up(away, MouseButton::Left, Modifiers::default());
    assert_eq!(clicks.get(), 0);
    cx.simulate_click(centre(button), Modifiers::default());
    assert_eq!(clicks.get(), 1);
}

/// Item 11: the ring is drawn outside the button while it has focus, `focus_offset` away and
/// `focus_width` wide, with the button's corners; it stays while the button is hovered and
/// pressed, and a disabled button never has one.
#[gpui::test]
fn the_ring_is_drawn_outside_while_focused(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        h_flex()
            .p(px(8.0))
            .child(Button::new("off").label("Off").disabled(true))
            .child(
                Button::new("b")
                    .label("Save")
                    .variant(ButtonVariant::Primary),
            )
    });
    assert!(!probe::last("b").style.focus_visible);
    press(cx, "tab");
    redraw(cx);
    let drawn = probe::last("b").style;
    assert!(drawn.focus_visible);
    let g = crate::foundation().geometry.clone();
    assert_eq!(
        (drawn.focus.width, drawn.focus.offset),
        (g.focus_width, g.focus_offset)
    );
    let out = px(g.focus_offset + g.focus_width);
    let button = bounds(cx, "b/button");
    let ring = bounds(cx, "b/ring");
    assert_eq!(ring.origin, button.origin - point(out, out));
    assert_eq!(ring.size, button.size + gpui::size(out * 2.0, out * 2.0));

    cx.simulate_mouse_move(centre(button), None, Modifiers::default());
    redraw(cx);
    assert!(probe::last("b").style.focus_visible, "with hover");
    cx.simulate_mouse_down(centre(button), MouseButton::Left, Modifiers::default());
    redraw(cx);
    assert!(probe::last("b").style.focus_visible, "while pressed");
    cx.simulate_mouse_up(centre(button), MouseButton::Left, Modifiers::default());
    assert!(!probe::last("off").style.focus_visible);
}

// Motion.

/// Items 19, 20, 21, 22: the spinner fades in over `motion.normal` on the standard curve, fades
/// out again when loading ends, carries on from where it is when reversed, and appears at once
/// with reduced motion.
#[gpui::test]
fn loading_fades_on_the_motion_tokens(cx: &mut TestAppContext) {
    let loading = Rc::new(Cell::new(false));
    let l = loading.clone();
    let cx = open(cx, move |_, _| {
        Button::new("b")
            .leading_icon("✓")
            .label("Save")
            .loading(l.get())
    });
    let normal = crate::foundation().motion.normal;
    let ease = |t: f32| MotionEasing::Standard.apply(t);
    let wait = |cx: &mut VisualTestContext, by| {
        cx.executor().advance_clock(by);
        redraw(cx);
    };
    let busy = || probe::last("b").busy;
    let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
    assert_eq!(busy(), 0.0, "it starts idle without fading");

    loading.set(true);
    redraw(cx);
    assert_eq!(busy(), 0.0);
    wait(cx, normal / 2);
    assert!(close(busy(), ease(0.5)), "{}", busy());
    wait(cx, normal / 2);
    assert_eq!(busy(), 1.0);

    // Ended a quarter of the way into fading out, then started again: no jump either way.
    loading.set(false);
    redraw(cx);
    wait(cx, normal / 4);
    assert!(close(busy(), ease(0.75)), "{}", busy());
    loading.set(true);
    redraw(cx);
    assert!(close(busy(), ease(0.75)), "reversed from where it was");
    wait(cx, normal / 8);
    assert!(close(busy(), ease(0.875)), "{}", busy());
    wait(cx, normal);
    assert_eq!(busy(), 1.0);
    loading.set(false);
    redraw(cx);
    wait(cx, normal);
    assert_eq!(busy(), 0.0, "the fade out ran to the end");

    reduce_motion();
    loading.set(true);
    redraw(cx);
    assert_eq!(busy(), 1.0, "at once with reduced motion");
}

// Tokens and themes.

/// What a button draws is its style from the component tokens, for its variant, emphasis,
/// density and layout, in every fixture theme; a monochrome theme's buttons are grey.
#[gpui::test]
fn it_draws_its_component_tokens_in_every_fixture(cx: &mut TestAppContext) {
    let emphases = [
        ("danger-muted", ButtonVariant::Destructive, Emphasis::Muted),
        ("warning-muted", ButtonVariant::Warning, Emphasis::Muted),
        ("success-subtle", ButtonVariant::Success, Emphasis::Subtle),
        ("primary-muted", ButtonVariant::Primary, Emphasis::Muted),
    ];
    let cx = open(cx, move |_, _| {
        h_flex()
            .flex_wrap()
            .children(ButtonVariant::ALL.map(|v| Button::new(v.name()).label("Go").variant(v)))
            .children(emphases.map(|(id, v, e)| Button::new(id).label("Go").variant(v).emphasis(e)))
    });
    for (theme, colors) in FIXTURES {
        publish_fixture(colors);
        redraw(cx);
        let c = crate::components();
        let lookup = |appearance| {
            c.button(
                appearance,
                Density::Default,
                ButtonLayout::Text,
                States::NONE,
            )
        };
        let mut drawn = Vec::new();
        for v in ButtonVariant::ALL {
            let style = probe::last(v.name()).style;
            assert_eq!(style, lookup(v.appearance()), "{theme} {v:?}");
            drawn.push(style);
        }
        for (id, v, e) in emphases {
            let style = probe::last(id).style;
            assert_eq!(
                style,
                lookup(v.appearance().with_emphasis(e)),
                "{theme} {id}"
            );
            drawn.push(style);
        }
        if theme == "vantablack" {
            for style in drawn {
                let all = [
                    style.colours.default,
                    style.colours.hover,
                    style.colours.pressed,
                ];
                for t in all {
                    for colour in [t.background, t.foreground, t.border] {
                        assert!(colour.to_oklch().c <= JND, "{theme}: {colour:?} has a hue");
                    }
                }
            }
        }
    }
}

/// A theme change recolours a button on the next frame and leaves its density, corners and
/// size alone.
#[gpui::test]
fn a_theme_change_recolours_buttons_and_keeps_their_size(cx: &mut TestAppContext) {
    let [(_, dark), .., (_, light)] = FIXTURES;
    let cx = open(cx, |_, _| {
        Button::new("b")
            .leading_icon("✓")
            .label("Save")
            .variant(ButtonVariant::Primary)
    });
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("colors.toml"), dark).unwrap();
    let mut manager = ThemeManager::new(ThemeSource::Dir(dir.path().to_owned()));
    manager.reload();
    cx.update(|window, cx| {
        crate::theme::follow(cx, manager, false);
        crate::configure(cx, |c| c.density = Density::Compact);
        window.refresh();
    });
    cx.run_until_parked();
    let before = probe::last("b").style;
    let size = bounds(cx, "b/button");
    assert_eq!(size.size.height, px(28.0));

    fs::write(dir.path().join("colors.toml"), light).unwrap();
    cx.update(|_, cx| crate::reload_theme(cx));
    cx.run_until_parked();
    let after = probe::last("b").style;
    assert_ne!(
        after.colours, before.colours,
        "recoloured without a restart"
    );
    assert_eq!(after.geometry, before.geometry, "same density and corners");
    assert_eq!(bounds(cx, "b/button"), size);
    let c = crate::components();
    assert_eq!(
        after,
        c.button(
            ButtonVariant::Primary,
            Density::Compact,
            ButtonLayout::LeadingIcon,
            States::NONE
        )
    );
}
