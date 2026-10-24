//! The component layer on real palettes: the fixtures, every stock theme installed, the
//! emergency palette and the user's own themes (`docs/spec/odk-component-tokens.md`).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use odk_theme::component::{
    self, ButtonAppearance, ButtonLayout, ButtonVariant, Check, ComponentTokens, Measure, NavLook,
    Shape, States, status_mark,
};
use odk_theme::foundation::{CornerRadius, Density, FoundationConfig, FoundationTokens, TypeRole};
use odk_theme::semantic::{
    BorderToken, ColourRef, Emphasis, ResolveOptions, Role, SemanticColours, State, SurfaceToken,
    TextToken, resolve,
};
use odk_theme::{JND, Palette, ThemeManager, ThemeSnapshot, ThemeSource, drawn_contrast};

fn fixture(name: &str) -> Palette {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .join("colors.toml");
    Palette::load(&path).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn themes_in(dir: &Path) -> Vec<(String, Palette)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    dirs.sort();
    dirs.into_iter()
        .filter_map(|dir| {
            let palette = Palette::load(&dir.join("colors.toml")).ok()?;
            Some((dir.file_name()?.to_string_lossy().into_owned(), palette))
        })
        .collect()
}

/// The fixtures the brief names: Tokyo Night, Nord, Catppuccin Mocha, Vantablack, a light theme
/// (Catppuccin Latte) and an incomplete custom theme.
const FIXTURES: [&str; 6] = [
    "tokyo-night",
    "nord",
    "catppuccin",
    "catppuccin-latte",
    "vantablack",
    "custom-incomplete",
];

/// The fixtures, the emergency palette, then every stock theme installed.
fn every_palette() -> Vec<(String, Palette)> {
    let mut all: Vec<_> = FIXTURES
        .iter()
        .map(|name| (format!("fixture {name}"), fixture(name)))
        .collect();
    all.push((
        "emergency".to_owned(),
        (*ThemeSnapshot::emergency().palette).clone(),
    ));
    all.extend(themes_in(Path::new("/usr/share/omarchy/themes")));
    all
}

fn semantic(palette: &Palette) -> SemanticColours {
    resolve(palette, &ResolveOptions::default())
        .unwrap()
        .colours
}

fn foundation(change: impl FnOnce(&mut FoundationConfig)) -> FoundationTokens {
    let mut config = FoundationConfig::default();
    change(&mut config);
    FoundationTokens::from_config(&config)
}

fn tokens_for(palette: &Palette) -> ComponentTokens {
    component::resolve(
        &FoundationTokens::default(),
        &semantic(palette),
        &ResolveOptions::default(),
    )
    .unwrap()
}

fn tokens(name: &str) -> ComponentTokens {
    tokens_for(&fixture(name))
}

const RESTING: [State; 3] = [State::Default, State::Hover, State::Pressed];

// Geometry.

#[test]
fn corners_are_exactly_0_or_exactly_2_and_round_parts_are_shapes() {
    for (corner, radius) in [(CornerRadius::Square, 0.0), (CornerRadius::Subtle, 2.0)] {
        let f = Arc::new(foundation(|c| c.corner_radius = corner));
        let t = tokens("tokyo-night").with_foundation(f);
        for d in Density::ALL {
            let button = t.button(ButtonVariant::Primary, d, ButtonLayout::Text, States::NONE);
            assert_eq!(button.geometry.radius, radius, "{corner:?}");
            assert_eq!(t.field(d, States::NONE).geometry.radius, radius);
            assert_eq!(t.select(d, States::NONE).trigger.geometry.radius, radius);
            assert_eq!(t.segmented(d).geometry.radius, radius);
            // A check box is a control: it takes the application's corners (the brief for the
            // form controls, 2026-10-22). A radio is round by nature.
            assert_eq!(
                t.checkbox(d, States::NONE).geometry.shape,
                Shape::Rect { radius }
            );
            assert_eq!(t.radio(d, States::NONE).geometry.shape, Shape::Circle);
            assert_eq!(
                t.field_action(
                    ButtonVariant::Ghost,
                    d,
                    ButtonLayout::IconOnly,
                    States::NONE
                )
                .geometry
                .radius,
                radius
            );
            let sw = t.switch(d, States::NONE).geometry;
            assert_eq!((sw.track, sw.thumb_shape), (Shape::Pill, Shape::Circle));
        }
        for overlay in [
            t.popover(),
            t.tooltip(),
            t.dialog(),
            t.toast(Role::Info).overlay,
        ] {
            assert_eq!(overlay.geometry.radius, radius);
        }
        // A sheet is against a window edge: no corner of its own, whatever the application's.
        assert_eq!(t.sheet().geometry.radius, 0.0);
        assert_eq!(t.menu().geometry.radius, radius);
        // Badges, alerts, progress bars, skeletons and sidebar items take the application's
        // corners too (the brief for navigation and feedback, 2026-10-23); keys keep square ones.
        assert_eq!(
            t.badge(component::BadgeAppearance::new(Role::Info, Emphasis::Muted))
                .geometry
                .radius,
            radius
        );
        let alert = t.alert(Role::Warning, Emphasis::Muted);
        assert_eq!(
            (alert.geometry.radius, alert.compact.radius),
            (radius, radius)
        );
        assert_eq!(t.progress(Role::Accent, States::NONE).radius, radius);
        assert_eq!(t.skeleton().radius, radius);
        assert_eq!(
            t.nav_item(Density::Default, States::NONE).geometry.radius,
            radius
        );
        assert_eq!(t.kbd().geometry.radius, 0.0);
        assert_eq!(t.status_dot(Role::Success).shape, Shape::Circle);

        // No rectangle anywhere is rounder than 2 px; only the shapes that say what they
        // are, a switch, a radio, a spinner and a status dot, are round.
        for row in t.diagnostics(&ResolveOptions::default()) {
            match row.shape {
                Shape::Rect { radius } => assert!((0.0..=2.0).contains(&radius), "{row}"),
                Shape::Pill | Shape::Circle => {
                    assert!(
                        matches!(row.component, "switch" | "radio" | "spinner" | "status dot"),
                        "{row}"
                    )
                }
            }
        }
    }
}

/// The subtle radius reaches a control exactly as the foundation stores it: 2 px (ADR 0042).
#[test]
fn the_subtle_radius_reaches_controls_as_it_is() {
    let t = tokens("nord");
    let g = t
        .button(
            ButtonVariant::Primary,
            Density::Default,
            ButtonLayout::Text,
            States::NONE,
        )
        .geometry;
    assert_eq!(g.radius.to_bits(), CornerRadius::Subtle.px().to_bits());
    assert_eq!(g.radius, 2.0);
}

#[test]
fn controls_are_28_34_and_40_high_and_follow_their_density() {
    let t = tokens("tokyo-night");
    for (d, height) in [
        (Density::Compact, 28.0),
        (Density::Default, 34.0),
        (Density::Comfortable, 40.0),
    ] {
        let m = d.metrics();
        let button = t.button(
            ButtonVariant::Secondary,
            d,
            ButtonLayout::Text,
            States::NONE,
        );
        for g in [
            button.geometry,
            t.field(d, States::NONE).geometry,
            t.select(d, States::NONE).trigger.geometry,
            t.segmented(d).geometry,
            t.tab(NavLook::Text, d, States::NONE).geometry,
            t.menu_item(d, States::NONE, false).geometry,
        ] {
            assert_eq!(g.height, height, "{d:?}");
            assert_eq!(g.icon_size, m.icon_size);
            // The text sits in the middle: what is left inside is one line.
            assert_eq!(g.content_height(), g.line_height, "{d:?} {g:?}");
            assert!(g.padding_y >= 0.0);
        }
        assert_eq!(button.geometry.padding_x, m.padding_x);
        assert_eq!(button.geometry.gap, m.gap);
        assert_eq!(button.geometry.type_role, TypeRole::Button);
        assert_eq!(t.field(d, States::NONE).geometry.type_role, TypeRole::Body);
        assert_eq!(button.width, None, "a text button has no minimum width");
        // A segment fits inside its frame's border.
        assert_eq!(t.segment(d, States::NONE).geometry.height, height - 2.0);
    }
    let paddings: Vec<_> = Density::ALL
        .map(|d| {
            t.button(ButtonVariant::Primary, d, ButtonLayout::Text, States::NONE)
                .geometry
                .padding_y
        })
        .into();
    assert_eq!(paddings, [4.0, 7.0, 10.0]);
}

#[test]
fn borders_are_one_pixel_and_focus_rings_two_outside_the_control() {
    let t = tokens("catppuccin");
    for d in Density::ALL {
        let button = t.button(ButtonVariant::Primary, d, ButtonLayout::Text, States::NONE);
        assert_eq!(button.geometry.border_width, 1.0);
        for ring in [
            button.focus,
            t.field(d, States::NONE).focus,
            t.checkbox(d, States::NONE).focus,
            t.switch(d, States::NONE).focus,
            t.tab(NavLook::Text, d, States::NONE).focus,
            t.segment(d, States::NONE).focus,
            t.nav_link(States::NONE).focus,
        ] {
            assert_eq!(ring.width, 2.0);
            // A gap of the surface lies between the ring and a solid fill.
            assert!(ring.offset > 0.0);
        }
        assert_eq!(t.checkbox(d, States::NONE).geometry.border_width, 1.0);
        assert_eq!(t.switch(d, States::NONE).geometry.border_width, 1.0);
    }
    assert_eq!(t.separator().thickness, 1.0);
    assert_eq!(
        t.tab(NavLook::Text, Density::Default, States::NONE)
            .indicator_width,
        2.0
    );
}

#[test]
fn not_everything_is_a_control_s_height() {
    let t = tokens("nord");
    // A check box is square, as large as the density's icon.
    for d in Density::ALL {
        assert_eq!(
            t.checkbox(d, States::NONE).geometry.size,
            d.metrics().icon_size
        );
    }
    // A switch's thumb fills its track less the border and the gap, and moves its own
    // diameter and one more gap.
    let sizes: Vec<_> = Density::ALL
        .map(|d| {
            let g = t.switch(d, States::NONE).geometry;
            assert_eq!(g.thumb + 2.0 * g.inset, g.height);
            assert_eq!(g.width - g.thumb - 2.0 * g.inset, g.travel);
            assert_eq!(g.track.corner_radius(g.width, g.height), g.height / 2.0);
            assert_eq!(g.thumb_shape.corner_radius(g.thumb, g.thumb), g.thumb / 2.0);
            (g.width, g.height, g.thumb)
        })
        .into();
    assert_eq!(
        sizes,
        [(24.0, 14.0, 8.0), (28.0, 16.0, 10.0), (32.0, 18.0, 12.0)]
    );
    // Badges and keys are as high as their text.
    let badge = t.badge(component::BadgeAppearance::new(
        Role::Success,
        Emphasis::Muted,
    ));
    assert_eq!(badge.geometry.height, 18.0 + 2.0);
    assert_eq!(t.kbd().geometry.height, 18.0 + 2.0);
    // A textarea is rows of text, and one row is a field's height.
    for d in Density::ALL {
        let field = t.field(d, States::NONE).geometry;
        assert_eq!(t.textarea(d, 1, States::NONE).geometry.height, field.height);
        let three = t.textarea(d, 3, States::NONE).geometry.height;
        assert_eq!(three, field.height + 2.0 * field.line_height);
    }
    // Dialogs follow layout spacing, not density.
    let f = FoundationTokens::default();
    assert_eq!(t.dialog().geometry.padding_x, f.layout.dialog_padding);
    assert_eq!(t.popover().geometry.padding_x, f.layout.panel_padding);
}

// Buttons.

#[test]
fn every_button_variant_density_and_state_resolves_from_its_semantic_treatment() {
    let t = tokens("tokyo-night");
    let s = t.semantic().clone();
    for variant in ButtonVariant::ALL {
        let a = variant.appearance();
        for d in Density::ALL {
            for state in RESTING {
                let states = States::NONE
                    .hovered(state == State::Hover)
                    .pressed(state == State::Pressed);
                let b = t.button(variant, d, ButtonLayout::Text, states);
                assert_eq!(b.current, b.colours.get(state));
                if a.link {
                    assert_eq!(b.current.background.a, 0.0, "a link has no fill");
                    assert!(b.underline);
                    assert_eq!(b.current.foreground, s.text.link);
                } else {
                    let semantic = s.treatment(a.role, a.emphasis, state);
                    assert_eq!(b.current.background, semantic.background, "{variant:?}");
                    assert_eq!(b.current.foreground, semantic.foreground, "{variant:?}");
                    assert!(!b.underline);
                }
            }
        }
    }
    use Emphasis::{Muted, Solid, Subtle};
    let expected = [
        (ButtonVariant::Primary, Role::Accent, Solid),
        (ButtonVariant::Secondary, Role::Neutral, Muted),
        (ButtonVariant::Outline, Role::Neutral, Subtle),
        (ButtonVariant::Ghost, Role::Neutral, Subtle),
        (ButtonVariant::Destructive, Role::Danger, Solid),
        (ButtonVariant::Success, Role::Success, Solid),
        (ButtonVariant::Warning, Role::Warning, Solid),
        (ButtonVariant::Link, Role::Accent, Subtle),
    ];
    for (variant, role, emphasis) in expected {
        let a = variant.appearance();
        assert_eq!((a.role, a.emphasis), (role, emphasis), "{variant:?}");
        assert_eq!(a.outline, variant == ButtonVariant::Outline);
        assert_eq!(a.link, variant == ButtonVariant::Link);
    }
    let outline = t.button(
        ButtonVariant::Outline,
        Density::Default,
        ButtonLayout::Text,
        States::NONE,
    );
    assert_eq!(outline.current.border, s.border.strong);
}

#[test]
fn any_role_takes_any_emphasis() {
    let t = tokens("catppuccin");
    let s = t.semantic().clone();
    for (variant, emphasis) in [
        (ButtonVariant::Destructive, Emphasis::Muted),
        (ButtonVariant::Warning, Emphasis::Muted),
        (ButtonVariant::Success, Emphasis::Subtle),
        (ButtonVariant::Destructive, Emphasis::Solid),
    ] {
        let a = variant.appearance().with_emphasis(emphasis);
        let b = t.button(a, Density::Default, ButtonLayout::Text, States::NONE);
        assert_eq!(b.current, *s.treatment(a.role, emphasis, State::Default));
    }
    let danger_outline = ButtonAppearance::new(Role::Danger, Emphasis::Subtle).outlined();
    let b = t.button(
        danger_outline,
        Density::Default,
        ButtonLayout::Text,
        States::NONE,
    );
    assert_eq!(b.current.border, s.roles.danger.focus);
}

#[test]
fn disabled_suppresses_hover_and_pressed() {
    let t = tokens("nord");
    let s = t.semantic().clone();
    for variant in ButtonVariant::ALL {
        let a = variant.appearance();
        let disabled = States::NONE.disabled(true);
        for states in [
            disabled,
            disabled.hovered(true),
            disabled.pressed(true),
            disabled.focused(true),
        ] {
            let b = t.button(variant, Density::Default, ButtonLayout::Text, states);
            assert_eq!(b.colours.default, b.colours.hover, "{variant:?}");
            assert_eq!(b.colours.default, b.colours.pressed, "{variant:?}");
            assert_eq!(b.current, b.colours.default);
            assert_eq!(b.current.foreground, s.text.disabled, "{variant:?}");
            assert!(!b.responds);
            assert!(!b.focus_visible, "a disabled button draws no focus");
            if !a.link {
                assert_eq!(
                    b.current.background,
                    s.treatment(a.role, a.emphasis, State::Disabled).background
                );
            }
        }
    }
}

#[test]
fn focus_composes_with_pointer_states_without_changing_them() {
    let t = tokens("tokyo-night");
    for variant in ButtonVariant::ALL {
        for states in [
            States::NONE,
            States::NONE.hovered(true),
            States::NONE.pressed(true),
            States::NONE.loading(true),
        ] {
            let without = t.button(variant, Density::Default, ButtonLayout::Text, states);
            let with = t.button(
                variant,
                Density::Default,
                ButtonLayout::Text,
                states.focused(true),
            );
            assert_eq!(with.current, without.current, "{variant:?} {states:?}");
            assert_eq!(with.colours, without.colours);
            assert_eq!(with.focus, without.focus, "one ring for every state");
            assert!(with.focus_visible && !without.focus_visible);
        }
    }
    // A neutral control's ring is the accent's; a role's is its own.
    let s = t.semantic().clone();
    let ghost = t.button(
        ButtonVariant::Ghost,
        Density::Default,
        ButtonLayout::Text,
        States::NONE,
    );
    assert_eq!(ghost.focus.colour, s.border.focus);
    let destructive = t.button(
        ButtonVariant::Destructive,
        Density::Default,
        ButtonLayout::Text,
        States::NONE,
    );
    assert_eq!(destructive.focus.colour, s.roles.danger.focus);
}

#[test]
fn icon_only_buttons_are_square_targets() {
    let t = tokens("nord");
    for d in Density::ALL {
        let b = t.button(
            ButtonVariant::Ghost,
            d,
            ButtonLayout::IconOnly,
            States::NONE,
        );
        let g = b.geometry;
        assert_eq!(b.width, Some(g.height));
        assert_eq!(2.0 * (g.padding_x + g.border_width) + g.icon_size, g.height);
        assert!(g.height >= 24.0, "WCAG 2.5.8's least target");
        for layout in [
            ButtonLayout::Text,
            ButtonLayout::LeadingIcon,
            ButtonLayout::TrailingIcon,
        ] {
            let text = t.button(ButtonVariant::Ghost, d, layout, States::NONE);
            assert_eq!(text.geometry.height, g.height);
            assert_eq!(text.width, None);
        }
    }
}

#[test]
fn loading_keeps_the_size_and_the_resting_colours() {
    let t = tokens("catppuccin-latte");
    for variant in ButtonVariant::ALL {
        for layout in [ButtonLayout::Text, ButtonLayout::IconOnly] {
            for d in Density::ALL {
                let idle = t.button(variant, d, layout, States::NONE);
                let busy = t.button(variant, d, layout, States::NONE.loading(true).hovered(true));
                assert_eq!(busy.geometry, idle.geometry);
                assert_eq!(busy.width, idle.width);
                assert_eq!(busy.current, idle.current, "loading keeps its look");
                assert_eq!(busy.colours.hover, idle.colours.default);
                assert!(!busy.responds);
                assert_eq!(busy.spinner_size, d.metrics().icon_size);
            }
        }
    }
}

// Forms.

#[test]
fn a_wrong_value_keeps_its_edge_when_focused_and_hovered() {
    let t = tokens("tokyo-night");
    let s = t.semantic().clone();
    let invalid = States::NONE.invalid(true);
    for states in [
        invalid,
        invalid.hovered(true),
        invalid.focused(true),
        invalid.focused(true).hovered(true),
    ] {
        let f = t.field(Density::Default, states);
        assert_eq!(f.current.border, s.border.danger, "{states:?}");
        assert_eq!(f.focus_border, s.border.danger);
        assert_eq!(f.focus.colour, s.border.danger);
    }
    let valid = t.field(Density::Default, States::NONE.focused(true));
    assert_eq!(valid.current.border, s.border.focus);
    assert_eq!(valid.focus.colour, s.border.focus);
    let normal = t.field(Density::Default, States::NONE);
    assert_eq!(normal.current.border, s.border.strong);
    assert_eq!(normal.colours.hover.border, s.border.hover);
    assert_eq!(normal.placeholder, s.text.placeholder);
    assert_eq!(normal.caret, s.text.primary);
    assert_eq!(normal.selection.background, s.surface.selection);
    assert_eq!(normal.selection.foreground, s.text.on_selection);
}

#[test]
fn disabled_and_read_only_fields() {
    let t = tokens("nord");
    let s = t.semantic().clone();
    let disabled = t.field(Density::Default, States::NONE.disabled(true).invalid(true));
    assert_eq!(
        disabled.current,
        *s.treatment(Role::Neutral, Emphasis::Muted, State::Disabled)
    );
    assert_eq!(disabled.colours.hover, disabled.colours.default);
    assert_eq!(disabled.placeholder, s.text.disabled);
    assert!(!disabled.responds);

    let read_only = t.field(Density::Default, States::NONE.read_only(true));
    assert_eq!(
        read_only.colours.hover, read_only.colours.default,
        "no hover"
    );
    assert_eq!(read_only.current.border, s.border.default);
    assert_eq!(read_only.current.foreground, s.text.primary);
    assert!(!read_only.responds);
    // A required field's mark is the danger role's text colour.
    assert_eq!(read_only.required, s.roles.danger.subtle.default.foreground);
}

/// Hovered, highlighted and selected are three different things in a list, also in a
/// monochrome theme.
#[test]
fn select_items_tell_hovered_highlighted_and_selected_apart() {
    for name in FIXTURES {
        let t = tokens(name);
        let overlay = t.semantic().surface.overlay;
        let drawn = |states: States| {
            t.menu_item(Density::Default, states, false)
                .current
                .background
                .over(overlay)
        };
        let rest = drawn(States::NONE);
        let hovered = drawn(States::NONE.hovered(true));
        let highlighted = drawn(States::NONE.highlighted(true));
        let pressed = drawn(States::NONE.pressed(true));
        for (a, b, what) in [
            (rest, hovered, "rest and hover"),
            (rest, highlighted, "rest and highlight"),
            (hovered, highlighted, "hover and highlight"),
            (hovered, pressed, "hover and pressed"),
        ] {
            assert!(a.apparent_distance(b) >= JND, "{name}: {what}");
        }
        // The selected item has a mark whether or not it is highlighted, and a highlighted
        // item is not selected unless it is.
        let selected = t.menu_item(Density::Default, States::NONE.selected(true), false);
        assert!(selected.indicator.is_some());
        assert_eq!(
            selected.current,
            t.menu_item(Density::Default, States::NONE, false).current
        );
        let both = t.menu_item(
            Density::Default,
            States::NONE.selected(true).highlighted(true),
            false,
        );
        assert_eq!(both.indicator, Some(both.current.foreground));
        assert!(
            t.menu_item(Density::Default, States::NONE.highlighted(true), false)
                .indicator
                .is_none()
        );
        // The box of an open select keeps a focused field's edge.
        let open = t.select(Density::Default, States::NONE.expanded(true));
        assert_eq!(open.trigger.current.border, t.semantic().border.focus);
        assert_eq!(open.indicator, t.semantic().text.primary);
    }
}

#[test]
fn checkboxes_have_checked_and_indeterminate_marks() {
    let t = tokens("tokyo-night");
    let s = t.semantic().clone();
    let unchecked = t.checkbox(Density::Default, States::NONE);
    assert_eq!(unchecked.check, Check::Unchecked);
    assert_eq!(unchecked.current.border, s.border.strong);
    assert_eq!(unchecked.current.foreground.a, 0.0, "no mark");
    for states in [States::NONE.checked(true), States::NONE.indeterminate(true)] {
        let c = t.checkbox(Density::Default, states);
        assert_ne!(c.check, Check::Unchecked);
        let fill = &s.roles.accent.solid;
        assert_eq!(c.current.background, fill.default.background);
        assert_eq!(c.current.foreground, s.text.on_accent);
        assert_eq!(
            c.current.border, s.roles.accent.focus,
            "an edge that shows on any surface"
        );
        assert_eq!(c.colours.hover.background, fill.hover.background);
    }
    assert_eq!(
        t.checkbox(
            Density::Default,
            States::NONE.checked(true).indeterminate(true)
        )
        .check,
        Check::Indeterminate
    );
    // Disabled keeps its mark, so ticked and clear still differ; neither carries the accent.
    let off = t.checkbox(Density::Default, States::NONE.disabled(true));
    let on = t.checkbox(Density::Default, States::NONE.disabled(true).checked(true));
    assert_eq!(on.current.foreground, s.text.disabled);
    assert_ne!(on.current, off.current);
    assert_ne!(
        on.current.background,
        s.roles.accent.solid.default.background
    );
    // A wrong value keeps the danger edge, ticked or not.
    let wrong = t.checkbox(
        Density::Default,
        States::NONE.checked(true).invalid(true).hovered(true),
    );
    assert_eq!(wrong.current.border, s.border.danger);
    assert_eq!(wrong.focus.colour, s.border.danger);
}

/// On and off differ by the track's fill and the thumb, and the thumb shows on its track, in
/// every theme, monochrome ones included.
#[test]
fn switches_are_told_apart_on_and_off() {
    for (name, palette) in every_palette() {
        let t = tokens_for(&palette);
        let s = t.semantic().clone();
        let on = t.switch(Density::Default, States::NONE.checked(true));
        let off = t.switch(Density::Default, States::NONE);
        assert!(on.on && !off.on);
        for g in s.surface.grounds() {
            let (a, b) = (
                on.current.background.over(g),
                off.current.background.over(g),
            );
            assert!(
                a.contrast(b) >= 1.5 || a.apparent_distance(b) >= 6.0 * JND,
                "{name}"
            );
            for style in [&on, &off] {
                let thumb = drawn_contrast(style.current.foreground, style.current.background, g);
                assert!(thumb >= 3.0, "{name}: thumb {thumb:.2}:1");
            }
        }
    }
}

/// A focused field keeps its focused edge under the pointer, so hover never hides it; a wrong
/// value has a mark as well as its edge, so it never shows by colour alone.
#[test]
fn a_focused_field_keeps_its_edge_and_a_wrong_one_has_a_mark() {
    let t = tokens("catppuccin");
    let s = t.semantic().clone();
    for states in [States::NONE.focused(true), States::NONE.expanded(true)] {
        let f = t.field(Density::Default, states);
        for c in [f.colours.default, f.colours.hover, f.colours.pressed] {
            assert_eq!(c.border, s.border.focus, "{states:?}");
        }
    }
    let wrong = t.field(
        Density::Default,
        States::NONE.invalid(true).focused(true).pressed(true),
    );
    assert_eq!(wrong.colours.hover.border, s.border.danger);
    assert_eq!(wrong.focus.colour, s.border.danger);
    assert_eq!(
        wrong.invalid_mark,
        Some(s.roles.danger.subtle.default.foreground)
    );
    assert_eq!(t.field(Density::Default, States::NONE).invalid_mark, None);
    assert_eq!(
        t.field(Density::Default, States::NONE.invalid(true).disabled(true))
            .invalid_mark,
        None,
        "a disabled field is not judged"
    );
    let f = t.field(Density::Default, States::NONE);
    assert_eq!(f.icon, s.text.tertiary);
    assert_eq!(f.addon, s.text.tertiary);
    assert_eq!(
        t.select(Density::Default, States::NONE).indicator,
        f.icon,
        "a select's indicator is a field's icon"
    );
}

/// A button inside a field is the field's height less its borders and the thumb gap on each
/// side: 22, 28 and 34, squares when they hold an icon alone, with the density's icon size.
#[test]
fn a_field_action_fits_inside_its_field() {
    let t = tokens("nord");
    let f = t.foundation().clone();
    for (d, side) in Density::ALL.into_iter().zip([22.0, 28.0, 34.0]) {
        let field = t.field(d, States::NONE).geometry;
        let icon = t.field_action(
            ButtonVariant::Ghost,
            d,
            ButtonLayout::IconOnly,
            States::NONE,
        );
        assert_eq!(icon.geometry.height, side, "{d:?}");
        assert_eq!(icon.width, Some(side));
        assert_eq!(
            icon.geometry.height + 2.0 * (field.border_width + f.geometry.thumb_gap),
            field.height
        );
        assert_eq!(icon.geometry.icon_size, d.metrics().icon_size);
        let text = t.field_action(ButtonVariant::Ghost, d, ButtonLayout::Text, States::NONE);
        assert_eq!((text.geometry.height, text.width), (side, None));
        assert!(text.geometry.content_height() >= text.geometry.line_height - 2.0);
        // Its colours are the button's: no colour of its own.
        let button = t.button(
            ButtonVariant::Ghost,
            d,
            ButtonLayout::IconOnly,
            States::NONE,
        );
        assert_eq!(icon.colours, button.colours);
    }
}

/// A radio is a check box's colours on a circle: the chosen one is the accent's fill with a dot
/// that shows on it in every theme, the others have no dot.
#[test]
fn radios_are_circles_with_a_dot_when_chosen() {
    for (name, palette) in every_palette() {
        let t = tokens_for(&palette);
        let chosen = t.radio(Density::Default, States::NONE.checked(true));
        let clear = t.radio(Density::Default, States::NONE);
        assert!(chosen.checked && !clear.checked);
        assert_eq!(
            chosen.colours,
            t.checkbox(Density::Default, States::NONE.checked(true))
                .colours
        );
        assert_eq!(clear.current.foreground.a, 0.0, "{name}: no dot");
        for g in t.semantic().surface.grounds() {
            let dot = drawn_contrast(chosen.current.foreground, chosen.current.background, g);
            assert!(dot >= 3.0, "{name}: dot {dot:.2}:1");
        }
    }
    let t = tokens("tokyo-night");
    for (d, dot) in Density::ALL.into_iter().zip([8.0, 10.0, 12.0]) {
        let g = t.radio(d, States::NONE).geometry;
        assert_eq!(
            (g.size, g.dot, g.shape),
            (d.metrics().icon_size, dot, Shape::Circle)
        );
    }
    let wrong = t.radio(Density::Default, States::NONE.invalid(true).hovered(true));
    assert_eq!(wrong.current.border, t.semantic().border.danger);
    assert!(
        wrong.invalid_mark.is_some(),
        "a wrong choice is not colour alone"
    );
    assert!(
        t.checkbox(Density::Default, States::NONE.invalid(true))
            .invalid_mark
            .is_some()
    );
    assert!(
        t.checkbox(Density::Default, States::NONE.invalid(true).disabled(true))
            .invalid_mark
            .is_none()
    );
}

/// A form's label, description and messages: 6 px from the control, 16 px between fields, and
/// every message but helper text with a status mark, so validation is never colour alone.
#[test]
fn form_fields_space_their_parts_and_mark_their_messages() {
    let t = tokens("vantablack");
    let s = t.semantic().clone();
    let f = t.form_field(States::NONE);
    assert_eq!((f.gap, f.form_gap), (6.0, 16.0));
    assert_eq!(f.label, s.text.primary);
    assert_eq!(f.description, s.text.secondary);
    assert_eq!(f.required, s.roles.danger.subtle.default.foreground);
    let off = t.form_field(States::NONE.disabled(true));
    assert_eq!(
        (off.label, off.description),
        (s.text.disabled, s.text.disabled)
    );
    assert_eq!(t.form_message(Role::Neutral).mark, None);
    assert_eq!(t.form_message(Role::Neutral).colour, s.text.secondary);
    for role in [Role::Success, Role::Warning, Role::Danger] {
        let m = t.form_message(role);
        assert_eq!(m.mark, status_mark(role));
        assert!(m.mark.is_some());
        assert_eq!(m.colour, t.status(role).colour);
    }
}

/// Body text is the body role in primary or secondary text. A heading's level picks its role:
/// 1 (and 0) the title, 2 the heading, 3 and below the subtitle, always in primary text. The
/// diagnostics list each with its role, and every palette's are checked for contrast.
#[test]
fn text_and_headings_take_their_type_roles() {
    let levels = [
        (0, TypeRole::Title),
        (1, TypeRole::Title),
        (2, TypeRole::Heading),
        (3, TypeRole::Subtitle),
        (4, TypeRole::Subtitle),
        (9, TypeRole::Subtitle),
    ];
    for (name, palette) in every_palette() {
        let t = tokens_for(&palette);
        let s = t.semantic().clone();
        let (body, secondary) = (t.text(false), t.text(true));
        assert_eq!((body.colour, body.role), (s.text.primary, TypeRole::Body));
        assert_eq!(
            (secondary.colour, secondary.role),
            (s.text.secondary, TypeRole::Body),
            "{name}"
        );
        for (level, role) in levels {
            let h = t.heading(level);
            assert_eq!((h.colour, h.role), (s.text.primary, role), "{name} {level}");
        }
    }
    let rows = tokens("nord").diagnostics(&ResolveOptions::default());
    for (component, variant, role) in [
        ("text", "primary", TypeRole::Body),
        ("text", "secondary", TypeRole::Body),
        ("heading", "1", TypeRole::Title),
        ("heading", "2", TypeRole::Heading),
        ("heading", "3", TypeRole::Subtitle),
    ] {
        let row = rows
            .iter()
            .find(|r| r.component == component && r.variant == variant)
            .unwrap_or_else(|| panic!("no diagnostics for {component} {variant}"));
        assert_eq!((row.type_role, row.density), (Some(role), None));
        assert_eq!(row.contrast[0].measure, Measure::Text);
    }
}

// Roles.

#[test]
fn every_role_resolves_for_badges_alerts_toasts_and_status() {
    let t = tokens("catppuccin");
    let s = t.semantic().clone();
    for role in Role::ALL {
        for emphasis in Emphasis::ALL {
            let badge = t.badge(component::BadgeAppearance::new(role, emphasis));
            assert_eq!(badge.colours, *s.treatment(role, emphasis, State::Default));
            let alert = t.alert(role, emphasis);
            if emphasis == Emphasis::Subtle {
                assert_eq!(alert.surface.foreground, s.text.primary);
                assert_eq!(alert.surface.border, s.role(role).focus);
            } else {
                assert_eq!(
                    alert.description, alert.surface.foreground,
                    "text inside a fill is its own"
                );
            }
        }
        assert_eq!(t.toast(role).mark, s.role(role).subtle.default.foreground);
        assert_eq!(
            t.status(role).colour,
            s.role(role).subtle.default.foreground
        );
    }
}

// Navigation and feedback.

/// A sidebar's item is a control's height at each density, with no border; the current page
/// has the neutral muted fill and the accent indicator, which shows on every surface; hover,
/// focus and the current page compose as a tab's do.
#[test]
fn nav_items_follow_density_and_mark_the_current_page() {
    for (name, palette) in every_palette() {
        let t = tokens_for(&palette);
        let s = t.semantic().clone();
        for d in Density::ALL {
            let g = t.nav_item(d, States::NONE).geometry;
            assert_eq!(g.height, d.metrics().height, "{name}");
            assert_eq!((g.border_width, g.padding_x), (0.0, d.metrics().padding_x));
            assert_eq!(g.icon_size, d.metrics().icon_size);
            assert_eq!(t.nav_list(d).header_padding_x, g.padding_x);
        }
        let current = t.nav_item(Density::Default, States::NONE.selected(true));
        let other = t.nav_item(Density::Default, States::NONE);
        assert_eq!(current.indicator, Some(s.roles.accent.focus), "{name}");
        assert_eq!(other.indicator, None);
        assert_eq!(
            current.current.background,
            s.roles.neutral.muted.hover.background
        );
        assert_eq!(other.current.background.a, 0.0, "{name}: no fill at rest");
        for g in s.surface.grounds() {
            let mark = drawn_contrast(s.roles.accent.focus, current.current.background, g);
            assert!(mark >= 3.0, "{name}: indicator {mark:.2}:1");
        }
        // Focus is drawn over any state and changes no colour.
        let hovered = t.nav_item(Density::Default, States::NONE.selected(true).hovered(true));
        let focused = t.nav_item(
            Density::Default,
            States::NONE.selected(true).hovered(true).focused(true),
        );
        assert_eq!(focused.colours, hovered.colours);
        assert!(focused.focus_visible && !hovered.focus_visible);
        assert_eq!(
            hovered.indicator, current.indicator,
            "selected stays under hover"
        );
        let off = t.nav_item(Density::Default, States::NONE.selected(true).disabled(true));
        assert_eq!(off.indicator, Some(s.text.disabled));
        assert!(!off.responds && !off.focus_visible);
    }
}

/// Selected, hovered and pressed stay apart in a segmented control and a sidebar: the selected
/// fill is the neutral muted fill under the pointer, 5 JND from the canvas, so an unselected item
/// under the pointer (2 JND) never passes for it, in every theme.
#[test]
fn a_selected_segment_or_row_reads_apart_from_a_hovered_one() {
    for (name, palette) in every_palette() {
        let t = tokens_for(&palette);
        let s = t.semantic().clone();
        let ground = s.surface.canvas;
        let fill = |c: odk_theme::Color| c.over(ground);
        let d = Density::Default;
        let pairs = [
            (
                t.segment(d, States::NONE.selected(true)).current,
                t.segment(d, States::NONE.hovered(true)).current,
            ),
            (
                t.nav_item(d, States::NONE.selected(true)).current,
                t.nav_item(d, States::NONE.hovered(true)).current,
            ),
        ];
        for (on, hovered) in pairs {
            let apart = fill(on.background).apparent_distance(fill(hovered.background));
            assert!(apart >= 2.0 * JND, "{name}: {:.1} JND", apart / JND);
        }
        // Selected stays selected under the pointer.
        let on = t.segment(d, States::NONE.selected(true));
        assert_eq!(on.colours.hover, on.colours.default, "{name}");
        assert_eq!(
            on.current.background,
            s.roles.neutral.muted.hover.background
        );
    }
}

/// A progress bar is the role's focus colour on the role's own tint, at least 3:1 against it over
/// every surface in every theme; disabled, both are grey.
#[test]
fn progress_bars_show_on_their_track_in_every_theme() {
    for (name, palette) in every_palette() {
        let t = tokens_for(&palette);
        let s = t.semantic().clone();
        for role in Role::ALL {
            let p = t.progress(role, States::NONE);
            assert_eq!(p.bar, s.role(role).focus, "{name} {role}");
            assert_eq!(p.track, s.role(role).muted.default.background);
            for g in s.surface.grounds() {
                let bar = drawn_contrast(p.bar, p.track, g);
                assert!(bar >= 3.0, "{name} {role}: bar {bar:.2}:1");
            }
        }
        let off = t.progress(Role::Accent, States::NONE.disabled(true));
        assert_eq!((off.bar, off.label), (s.text.disabled, s.text.disabled));
    }
    let t = tokens("nord");
    let p = t.progress(Role::Accent, States::NONE);
    assert_eq!((p.height, p.type_role), (4.0, TypeRole::Small));
}

/// A status dot is round, filled in its role's colour, or a hollow grey ring for the neutral
/// role, so on and off differ in shape too; its label is readable text.
#[test]
fn status_dots_differ_in_shape_and_carry_a_label() {
    let t = tokens("vantablack");
    let s = t.semantic().clone();
    for role in Role::ALL {
        let dot = t.status_dot(role);
        assert_eq!((dot.shape, dot.size), (Shape::Circle, 8.0));
        assert_eq!(dot.filled, role != Role::Neutral, "{role}");
        assert_eq!(dot.label, s.text.secondary);
    }
    assert_eq!(t.status_dot(Role::Neutral).colour, s.text.tertiary);
    assert_eq!(
        t.status_dot(Role::Danger).colour,
        t.status(Role::Danger).colour
    );
}

/// A skeleton is the neutral muted tint in lines of body text; a spinner turns once per cycle
/// and stands still with reduced motion; a separator is subtle and insets by a control's padding.
#[test]
fn skeletons_spinners_and_separators() {
    let t = tokens("catppuccin-latte");
    let s = t.semantic().clone();
    let k = t.skeleton();
    assert_eq!(k.fill, s.roles.neutral.muted.default.background);
    assert_eq!((k.line_height, k.bar_height), (20.0, 13.0));
    let spinner = t.spinner(Density::Compact);
    assert_eq!((spinner.size, spinner.turn.as_millis()), (14.0, 800));
    assert!(!spinner.still);
    let reduced = t.with_foundation(Arc::new(foundation(|c| c.reduced_motion = true)));
    assert!(reduced.spinner(Density::Default).still);
    let sep = t.separator();
    assert_eq!((sep.colour, sep.thickness), (s.border.subtle, 1.0));
    for d in Density::ALL {
        let at = t.with_foundation(Arc::new(foundation(|c| c.density = d)));
        assert_eq!(at.separator().inset, d.metrics().padding_x);
    }
    let compact = t.alert(Role::Info, Emphasis::Muted).compact;
    let standard = t.alert(Role::Info, Emphasis::Muted).geometry;
    assert!(compact.padding_x < standard.padding_x && compact.padding_y < standard.padding_y);
}

/// A status reads without its hue: each status role has a mark of its own.
#[test]
fn status_is_never_colour_alone() {
    let marks: Vec<_> = [Role::Info, Role::Success, Role::Warning, Role::Danger]
        .map(|r| status_mark(r).expect("a status has a mark"))
        .into();
    for (i, a) in marks.iter().enumerate() {
        for b in &marks[i + 1..] {
            assert_ne!(a, b);
        }
    }
    assert_eq!(status_mark(Role::Accent), None);
    let t = tokens("vantablack");
    assert_eq!(t.status(Role::Danger).mark, Some("×"));
}

/// Every colour a component draws is the semantic colour its source names: the component layer
/// makes none of its own.
#[test]
fn components_draw_only_semantic_colours() {
    for (name, palette) in every_palette() {
        let t = tokens_for(&palette);
        let s = t.semantic();
        for row in t.diagnostics(&ResolveOptions::default()) {
            assert_eq!(
                row.colours.background,
                s.colour(row.sources.background),
                "{name}: {row}"
            );
            assert_eq!(
                row.colours.foreground,
                s.colour(row.sources.foreground),
                "{name}: {row}"
            );
            assert_eq!(
                row.colours.border,
                s.colour(row.sources.border),
                "{name}: {row}"
            );
        }
    }
    // And no semantic name is left over for a colour of their own: `none` is the canvas,
    // transparent.
    let s = semantic(&fixture("nord"));
    assert_eq!(s.colour(ColourRef::None), s.surface.canvas.with_alpha(0.0));
    assert_eq!(
        s.colour(ColourRef::Surface(SurfaceToken::Scrim)),
        s.surface.scrim
    );
    assert_eq!(s.colour(ColourRef::Text(TextToken::Link)), s.text.link);
    assert_eq!(
        s.colour(ColourRef::Border(BorderToken::Danger)),
        s.border.danger
    );
}

#[test]
fn monochrome_themes_stay_monochrome() {
    let mut themes = vec![("vantablack".to_owned(), fixture("vantablack"))];
    themes.extend(
        themes_in(Path::new("/usr/share/omarchy/themes"))
            .into_iter()
            .filter(|(n, _)| n == "white"),
    );
    themes.push((
        "emergency".to_owned(),
        (*ThemeSnapshot::emergency().palette).clone(),
    ));
    for (name, palette) in themes {
        let t = tokens_for(&palette);
        for row in t.diagnostics(&ResolveOptions::default()) {
            for c in [
                row.colours.background,
                row.colours.foreground,
                row.colours.border,
            ] {
                assert!(c.to_oklch().c <= JND, "{name}: {row}");
            }
        }
    }
}

// Themes and contrast.

/// Every theme ODK may meet passes every component check, so the checks never refuse a theme
/// the semantic layer accepts: the fixtures, the emergency palette and every stock theme.
#[test]
fn every_theme_passes_every_component_check() {
    for (name, palette) in every_palette() {
        let s = semantic(&palette);
        component::validate(&s, &ResolveOptions::default())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let rows = tokens_for(&palette).diagnostics(&ResolveOptions::default());
        assert!(rows.len() > 500, "{name}");
        for row in rows {
            for r in &row.contrast {
                assert!(r.passes(), "{name}: {row}");
            }
        }
    }
}

/// Final pairs are measured as drawn, over each surface beneath: a muted button on a raised
/// panel is its tint over that panel.
#[test]
fn final_pairs_are_measured_over_their_actual_surfaces() {
    let t = tokens("nord");
    let s = t.semantic().clone();
    let secondary = t.button(
        ButtonVariant::Secondary,
        Density::Default,
        ButtonLayout::Text,
        States::NONE.hovered(true),
    );
    assert!(secondary.current.background.a < 1.0, "a translucent fill");
    let rows = t.diagnostics(&ResolveOptions::default());
    let row = rows
        .iter()
        .find(|r| r.component == "button" && r.variant == "secondary" && r.state == "hover")
        .unwrap();
    let text = row
        .contrast
        .iter()
        .find(|r| r.measure == Measure::Text)
        .unwrap();
    let least = s
        .surface
        .grounds()
        .into_iter()
        .map(|g| {
            drawn_contrast(
                secondary.current.foreground,
                secondary.current.background,
                g,
            )
        })
        .fold(f32::INFINITY, f32::min);
    assert_eq!(text.found, least);
    assert!(text.found >= 4.5);
    // Menu items are measured over the overlay they are drawn on.
    let item = rows
        .iter()
        .find(|r| r.component == "menu item" && r.variant == "highlighted" && r.state == "default")
        .unwrap();
    assert_eq!(item.contrast[0].against, "overlay");
}

/// The button's own pairs, as `Button` draws them: every label reads over every surface it may
/// sit on, in every variant, at every emphasis and in every pointer state, in every fixture. A
/// disabled button's label is `text.disabled`, plainly fainter than active text and not held to
/// text's 4.5:1, as WCAG exempts inactive controls; its label still says what it is.
#[test]
fn button_labels_read_in_every_variant_emphasis_and_state() {
    for name in FIXTURES {
        let t = tokens(name);
        let s = t.semantic().clone();
        for variant in ButtonVariant::ALL {
            let own = variant.appearance();
            let appearances = [own]
                .into_iter()
                .chain(Emphasis::ALL.map(|e| own.with_emphasis(e)));
            for a in appearances {
                let b = t.button(a, Density::Default, ButtonLayout::Text, States::NONE);
                for state in RESTING {
                    let c = b.colours.get(state);
                    for ground in s.surface.grounds() {
                        let found = drawn_contrast(c.foreground, c.background, ground);
                        assert!(
                            found >= 4.5,
                            "{name} {variant:?} as {} {state:?}: {found:.2}:1 over {ground:?}",
                            a.name()
                        );
                    }
                }
                let off = States::NONE.disabled(true);
                let disabled = t.button(a, Density::Default, ButtonLayout::Text, off);
                assert_eq!(disabled.current.foreground, s.text.disabled, "{name}");
            }
        }
        let faint = s.text.disabled.contrast(s.surface.canvas);
        let active = s.text.primary.contrast(s.surface.canvas);
        assert!(
            faint < 3.0 && faint < active,
            "{name}: disabled text at {faint:.2}:1"
        );
    }
}

/// Every button appearance can be found in the diagnostics under its name, with the colours a
/// button of it draws, so a tool such as the gallery can show where each colour came from.
#[test]
fn every_button_appearance_is_in_the_diagnostics_under_its_name() {
    let t = tokens("tokyo-night");
    let rows = t.diagnostics(&ResolveOptions::default());
    let find = |name: &str, state: &str| {
        rows.iter()
            .find(|r| {
                r.component == "button"
                    && r.variant == name
                    && r.state == state
                    && r.density == Some(Density::Default)
            })
            .unwrap_or_else(|| panic!("no diagnostics for button {name} {state}"))
    };
    for variant in ButtonVariant::ALL {
        let own = variant.appearance();
        for a in [own]
            .into_iter()
            .chain(Emphasis::ALL.map(|e| own.with_emphasis(e)))
        {
            let b = t.button(a, Density::Default, ButtonLayout::Text, States::NONE);
            for state in RESTING {
                assert_eq!(find(&a.name(), state.name()).colours, b.colours.get(state));
            }
            let off = States::NONE.disabled(true);
            let disabled = t.button(a, Density::Default, ButtonLayout::Text, off);
            assert_eq!(find(&a.name(), "disabled").colours, disabled.current);
            assert_eq!(find(&a.name(), "focus").colours.foreground, b.focus.colour);
            assert_eq!(find(&a.name(), "default").height, Some(34.0));
        }
        assert_eq!(
            find(variant.name(), "default").colours,
            find(&own.name(), "default").colours,
            "a variant is listed under its own name too"
        );
    }
    assert_eq!(
        ButtonVariant::Destructive
            .appearance()
            .with_emphasis(Emphasis::Muted)
            .name(),
        "danger muted"
    );
    assert_eq!(
        ButtonVariant::Outline.appearance().name(),
        "neutral subtle outline"
    );
    assert_eq!(
        ButtonVariant::Link
            .appearance()
            .with_emphasis(Emphasis::Solid)
            .name(),
        "accent link"
    );
}

/// The user's own themes are reported, not asserted: the tests must not depend on them.
#[test]
fn the_user_s_own_themes_are_reported() {
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let themes = themes_in(&PathBuf::from(home).join(".config/omarchy/themes"));
    let mut passing = 0;
    for (name, palette) in &themes {
        let Ok(r) = resolve(palette, &ResolveOptions::default()) else {
            eprintln!("{name}: does not resolve");
            continue;
        };
        match component::validate(&r.colours, &ResolveOptions::default()) {
            Ok(()) => passing += 1,
            Err(e) => eprintln!("{name}: {e}"),
        }
    }
    eprintln!(
        "{passing} of the user's {} themes pass every component check",
        themes.len()
    );
}

/// A component pair that falls short is an error naming it, never a colour.
#[test]
fn a_pair_that_falls_short_is_an_error() {
    let mut s = semantic(&fixture("tokyo-night"));
    s.text.on_selection = s.surface.canvas;
    let e = component::validate(&s, &ResolveOptions::default()).unwrap_err();
    assert_eq!(
        (e.component, e.state, e.what),
        ("field", "selection", "text")
    );
    assert!(e.to_string().contains("below the 4.5:1"), "{e}");
    let stricter = ResolveOptions {
        text_contrast: 21.0,
        ui_contrast: 21.0,
        ..Default::default()
    };
    assert!(component::validate(&semantic(&fixture("nord")), &stricter).is_err());
}

// Snapshots.

#[test]
fn a_density_or_radius_change_keeps_the_very_same_colours() {
    let base = tokens("tokyo-night");
    for change in [
        |c: &mut FoundationConfig| c.density = Density::Compact,
        |c: &mut FoundationConfig| c.corner_radius = CornerRadius::Square,
    ] {
        let next = base.with_foundation(Arc::new(foundation(change)));
        assert!(
            Arc::ptr_eq(next.semantic(), base.semantic()),
            "no colour resolved again"
        );
        assert_eq!(next.generation(), base.generation());
        // Each at its own application density.
        let (a, b) = (
            base.button(
                ButtonVariant::Primary,
                base.density(None),
                ButtonLayout::Text,
                States::NONE,
            ),
            next.button(
                ButtonVariant::Primary,
                next.density(None),
                ButtonLayout::Text,
                States::NONE,
            ),
        );
        assert_eq!(a.colours, b.colours);
        assert_ne!(a.geometry, b.geometry);
    }
    assert_eq!(
        base.with_foundation(Arc::new(foundation(|c| c.density = Density::Compact)))
            .density(None),
        Density::Compact
    );
}

/// The matrix of Phase 12: every fixture (Tokyo Night, Nord, Catppuccin Mocha and Latte,
/// Vantablack, the incomplete theme) at every density and both radii. Geometry follows the
/// foundation alone, whatever the theme; colours follow the theme alone, whatever the density
/// and radius; and a ring is seen against every surface at every setting. Each failure names
/// the theme, density, radius, component and state. The contrast of every pair in every theme is
/// `every_theme_passes_every_component_check`, and labels in every variant, emphasis and state
/// `button_labels_read_in_every_variant_emphasis_and_state`; this test is what joins them.
#[test]
fn theme_by_density_by_radius_matrix() {
    use odk_theme::JND;
    for name in FIXTURES {
        let palette = fixture(name);
        let base = tokens_for(&palette);
        let grounds = base.semantic().surface.grounds();
        for radius in [CornerRadius::Square, CornerRadius::Subtle] {
            for density in Density::ALL {
                let at = format!("{name} / {density:?} / {radius:?}");
                let f = foundation(|c| {
                    c.density = density;
                    c.corner_radius = radius;
                });
                let height = f.controls.for_density(density).height;
                let t = base.with_foundation(Arc::new(f));
                assert!(
                    Arc::ptr_eq(t.semantic(), base.semantic()),
                    "{at}: a setting resolved the colours again"
                );
                for variant in ButtonVariant::ALL {
                    for (state, states) in [
                        ("rest", States::NONE),
                        ("hover", States::NONE.hovered(true)),
                        ("pressed", States::NONE.pressed(true)),
                        ("disabled", States::NONE.disabled(true)),
                        ("loading", States::NONE.loading(true)),
                    ] {
                        let what = format!("{at}: button {variant:?} {state}");
                        let b = t.button(variant, density, ButtonLayout::Text, states);
                        let reference = base.button(variant, density, ButtonLayout::Text, states);
                        assert_eq!(b.geometry.height, height, "{what}: height");
                        assert_eq!(b.geometry.radius, radius.px(), "{what}: radius");
                        assert!(b.geometry.radius <= 2.0, "{what}: rounder than 2 px");
                        assert_eq!(b.geometry.border_width, 1.0, "{what}: border");
                        assert_eq!(b.focus.width, 2.0, "{what}: focus ring width");
                        assert_eq!(b.current, reference.current, "{what}: colours moved");
                        for ground in grounds {
                            let seen = drawn_contrast(b.focus.colour, ground, ground);
                            assert!(
                                seen >= 3.0,
                                "{what}: focus ring {seen:.2}:1 against {ground:?}"
                            );
                        }
                    }
                }
                for (component, g) in [
                    ("field", t.field(density, States::NONE).geometry),
                    ("select", t.select(density, States::NONE).trigger.geometry),
                    ("segmented", t.segmented(density).geometry),
                    ("tab", t.tab(NavLook::Text, density, States::NONE).geometry),
                    (
                        "menu item",
                        t.menu_item(density, States::NONE, false).geometry,
                    ),
                ] {
                    assert_eq!(g.height, height, "{at}: {component} height");
                    assert!(
                        g.radius <= radius.px(),
                        "{at}: {component} radius {}",
                        g.radius
                    );
                    assert_eq!(g.border_width.max(1.0), 1.0, "{at}: {component} border");
                }
                // Hover and press stay apart from rest by a visible step, as drawn over the
                // canvas: the fills are translucent.
                let canvas = base.semantic().surface.canvas;
                let drawn = |states| {
                    t.button(
                        ButtonVariant::Secondary,
                        density,
                        ButtonLayout::Text,
                        states,
                    )
                    .current
                    .background
                    .over(canvas)
                };
                let rest = drawn(States::NONE);
                for (state, states) in [
                    ("hover", States::NONE.hovered(true)),
                    ("pressed", States::NONE.pressed(true)),
                ] {
                    let moved = drawn(states).apparent_distance(rest);
                    assert!(
                        moved >= 2.0 * JND - 0.005,
                        "{at}: secondary {state} moved {moved:.3}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_theme_change_keeps_the_foundation_and_changes_the_colours() {
    let base = tokens("tokyo-night");
    let next = base.with_theme(Arc::new(semantic(&fixture("catppuccin-latte"))), 7);
    assert!(Arc::ptr_eq(next.foundation(), base.foundation()));
    assert_eq!(next.generation(), 7);
    let (a, b) = (
        base.button(
            ButtonVariant::Primary,
            Density::Default,
            ButtonLayout::Text,
            States::NONE,
        ),
        next.button(
            ButtonVariant::Primary,
            Density::Default,
            ButtonLayout::Text,
            States::NONE,
        ),
    );
    assert_eq!(a.geometry, b.geometry);
    assert_ne!(a.current, b.current);
}

#[test]
fn identical_inputs_give_identical_tokens() {
    for name in FIXTURES {
        let (a, b) = (tokens(name), tokens(name));
        assert_eq!(a, b, "{name}");
        let options = ResolveOptions::default();
        assert_eq!(a.diagnostics(&options), b.diagnostics(&options), "{name}");
        let text: Vec<String> = a
            .diagnostics(&options)
            .iter()
            .map(|r| r.to_string())
            .collect();
        let again: Vec<String> = b
            .diagnostics(&options)
            .iter()
            .map(|r| r.to_string())
            .collect();
        assert_eq!(text, again, "in the same order");
    }
}

/// A theme the manager publishes has colours every component check passed, and a failed
/// resolution keeps the snapshot as it was. The failure here is the semantic layer's: no theme
/// fails only the component checks, since they use pairs it guarantees, so the component branch
/// is covered by `a_pair_that_falls_short_is_an_error` on tampered colours instead.
#[test]
fn only_checked_themes_are_published() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("colors.toml"),
        fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/nord/colors.toml"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut manager = ThemeManager::new(ThemeSource::Dir(dir.path().to_owned()));
    manager.reload();
    let snapshot = manager.snapshot();
    assert!(!snapshot.emergency);
    component::validate(&snapshot.semantic, &snapshot.resolution.options).unwrap();
    let before = manager.snapshot();
    let impossible = ResolveOptions {
        text_contrast: 21.0,
        ..Default::default()
    };
    manager.set_options(impossible).unwrap();
    assert!(
        Arc::ptr_eq(&before, &manager.snapshot()),
        "nothing partial is published"
    );
}

/// How long a theme takes at each step, for reading (`cargo test --release -p odk-theme --test
/// components print_timings -- --ignored --nocapture`). Nothing asserts: a time is a fact about
/// one machine. What the tests do assert is that none of this happens while drawing.
#[test]
#[ignore = "prints how long each step of a theme change takes, for reading"]
fn print_timings() {
    use std::time::Instant;
    const RUNS: u32 = 200;
    fn mean(mut step: impl FnMut()) -> f64 {
        step();
        let start = Instant::now();
        for _ in 0..RUNS {
            step();
        }
        start.elapsed().as_secs_f64() * 1000.0 / f64::from(RUNS)
    }
    for name in FIXTURES {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name);
        let colors = fs::read_to_string(dir.join("colors.toml")).unwrap();
        let palette = fixture(name);
        let semantic_colours = semantic(&palette);
        let foundation = FoundationTokens::default();
        let parse = mean(|| {
            Palette::parse(&colors).unwrap();
        });
        let resolve_colours = mean(|| {
            resolve(&palette, &ResolveOptions::default()).unwrap();
        });
        let components = mean(|| {
            component::resolve(&foundation, &semantic_colours, &ResolveOptions::default()).unwrap();
        });
        let snapshot = mean(|| {
            ThemeSnapshot::from_toml(&colors, None).unwrap();
        });
        let density = tokens_for(&palette);
        let regenerate = mean(|| {
            std::hint::black_box(density.with_foundation(Arc::new(foundation.clone())));
        });
        println!(
            "{name:18} parse {parse:6.3} ms  resolve {resolve_colours:6.3} ms  component tokens {components:6.3} ms  whole snapshot {snapshot:6.3} ms  density or radius change {:6.3} us",
            regenerate * 1000.0
        );
    }
}

#[test]
#[ignore = "prints the component styles of Tokyo Night, for reading"]
fn print_diagnostics() {
    for row in tokens("tokyo-night").diagnostics(&ResolveOptions::default()) {
        eprintln!("{row}");
    }
}

/// The contrast pairings that interaction and accessibility rest on are in the catalogue that
/// every resolution checks (`docs/spec/odk-component-tokens.md`), so a theme that cannot reach
/// them is a failed reload and no component needs a check of its own. This pins the list, by
/// variant: the mark of a check box is on its checked variant, a radio's dot on its selected
/// one, and a pairing dropped from the catalogue fails here, not only a whole component.
/// `every_palette` resolving without error is the proof that each reaches its minimum on every
/// fixture and installed theme.
#[test]
fn the_pairings_that_interaction_rests_on_are_all_measured() {
    let measured: Vec<(String, String, &str, Measure)> = tokens("tokyo-night")
        .diagnostics(&ResolveOptions::default())
        .iter()
        .flat_map(|d| {
            d.contrast
                .iter()
                .map(|c| {
                    (
                        d.component.to_owned(),
                        d.variant.clone(),
                        d.state,
                        c.measure,
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let has = |component: &str, variant: &str, state: &str, measure: Measure| {
        measured
            .iter()
            .any(|(c, v, s, m)| c == component && v == variant && *s == state && *m == measure)
    };
    // Focus rings, against every surface the control can be drawn on, at 3:1, for every kind.
    for (component, variant) in [
        ("button", "primary"),
        ("button", "secondary"),
        ("button", "outline"),
        ("button", "destructive"),
        ("field", "normal"),
        ("field", "invalid"),
        ("field", "read-only"),
        ("field action", "ghost"),
        ("segment", "off"),
        ("segment", "on"),
        ("nav link", "link"),
        ("tab", "text selected"),
        ("checkbox", "unchecked"),
        ("checkbox", "checked"),
        ("radio", "unselected"),
        ("radio", "selected"),
        ("switch", "off"),
        ("switch", "on"),
        ("tab", "text unselected"),
        ("nav item", "unselected"),
        ("nav item", "current page"),
    ] {
        assert!(
            has(component, variant, "focus", Measure::Mark),
            "{component} {variant}: the focus ring is measured"
        );
    }
    // The marks that show a choice, on the variant that shows it: a check box's tick and dash, a
    // radio's dot, a switch's thumb on and off, a tab's and a sidebar page's indicator.
    for (component, variant, state, measure) in [
        ("checkbox", "checked", "default", Measure::Text),
        ("checkbox", "indeterminate", "default", Measure::Text),
        ("radio", "selected", "default", Measure::Mark),
        ("switch", "on", "default", Measure::Mark),
        ("switch", "off", "default", Measure::Mark),
        ("tab", "text selected", "indicator", Measure::Mark),
        ("tab", "surface selected", "indicator", Measure::Mark),
        ("nav item", "current page", "indicator", Measure::Mark),
    ] {
        assert!(
            has(component, variant, state, measure),
            "{component} {variant} {state}: the {} is measured",
            measure.name()
        );
    }
    // The edge that shows where a control is, in each state it is chosen or not.
    for (component, variant) in [
        ("checkbox", "unchecked"),
        ("checkbox", "checked"),
        ("radio", "unselected"),
        ("radio", "selected"),
        ("switch", "off"),
        ("switch", "on"),
    ] {
        assert!(
            has(component, variant, "default", Measure::Edge),
            "{component} {variant}: the edge is measured"
        );
    }
    // Progress, a status dot and a field's marks and placeholder, by component and state: each
    // has one variant.
    let any = |component: &str, state: &str, measure: Measure| {
        measured
            .iter()
            .any(|(c, _, s, m)| c == component && *s == state && *m == measure)
    };
    for (component, state, measure) in [
        ("progress", "bar", Measure::Mark),
        ("status dot", "dot", Measure::Mark),
        ("select", "default", Measure::Mark),
        ("field", "placeholder", Measure::Text),
        ("field", "required", Measure::Text),
        ("field", "invalid mark", Measure::Text),
        ("tooltip", "default", Measure::Text),
        ("toast", "default", Measure::Text),
        ("toast", "secondary", Measure::Text),
        ("menu item", "hover", Measure::Text),
        ("menu", "secondary", Measure::Text),
        ("alert", "mark", Measure::Text),
    ] {
        assert!(
            any(component, state, measure),
            "{component} {state}: the {} is measured",
            measure.name()
        );
    }
}
