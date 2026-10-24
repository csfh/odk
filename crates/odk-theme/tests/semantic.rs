//! The semantic layer on real palettes: Omarchy's stock themes as fixtures, two custom themes,
//! every stock theme installed, and palettes made to test one rule each
//! (`docs/spec/odk-semantic-tokens.md`).

use std::fs;
use std::path::{Path, PathBuf};

use odk_theme::semantic::{
    Emphasis, Note, OptionsError, Reason, Resolution, ResolveError, ResolveOptions, Role,
    RoleOverride, SemanticColours, Source, State, Strategy, Subject, TextToken, resolve,
};
use odk_theme::{Color, JND, Mode, Palette, PaletteKey, drawn_contrast, hue_distance};

fn fixture(name: &str) -> Palette {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .join("colors.toml");
    Palette::load(&path).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn stock_themes() -> Vec<(String, Palette)> {
    let Ok(entries) = fs::read_dir("/usr/share/omarchy/themes") else {
        return Vec::new();
    };
    let mut themes: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    themes.sort();
    themes
        .into_iter()
        .filter(|dir| dir.join("colors.toml").is_file())
        .map(|dir| {
            let palette = Palette::load(&dir.join("colors.toml")).unwrap();
            (
                dir.file_name().unwrap().to_string_lossy().into_owned(),
                palette,
            )
        })
        .collect()
}

/// The fixtures the brief names: Tokyo Night, Nord, Catppuccin Mocha, Vantablack, a light
/// theme (Catppuccin Latte) and an incomplete custom theme.
const FIXTURES: [&str; 6] = [
    "tokyo-night",
    "nord",
    "catppuccin",
    "catppuccin-latte",
    "vantablack",
    "custom-incomplete",
];

/// The fixtures, then every stock theme installed (none where Omarchy is not).
fn every_palette() -> Vec<(String, Palette)> {
    let mut all: Vec<_> = FIXTURES
        .iter()
        .map(|name| (format!("fixture {name}"), fixture(name)))
        .collect();
    all.extend(stock_themes());
    all
}

fn defaults(palette: &Palette) -> Resolution {
    resolve(palette, &ResolveOptions::default()).unwrap()
}

fn hex(s: &str) -> Color {
    Color::parse_hex(s).unwrap()
}

/// A dark palette with every key the tests below change, so each test states only the colours
/// its rule is about.
const DARK: &str = r##"
mode = "dark"
accent = "#7aa2f7"
muted = "#414868"
background = "#1a1b26"
dark_background = "#13141c"
lighter_background = "#24283b"
foreground = "#c0caf5"
red = "#f7768e"
yellow = "#e0af68"
green = "#9ece6a"
blue = "#7aa2f7"
"##;

fn with(base: &str, extra: &[(&str, &str)]) -> Palette {
    let mut text: String = base
        .lines()
        .filter(|line| {
            !extra
                .iter()
                .any(|(k, _)| line.starts_with(&format!("{k} ")))
        })
        .map(|line| format!("{line}\n"))
        .collect();
    for (key, value) in extra {
        text.push_str(&format!("{key} = \"{value}\"\n"));
    }
    Palette::parse(&text).unwrap()
}

/// Every place a role's text and focus indicator may be drawn, and every surface text may be.
fn grounds(c: &SemanticColours) -> [Color; 5] {
    c.surface.grounds()
}

/// The least contrast of `fg` on `bg` drawn over every surface.
fn worst_drawn(c: &SemanticColours, fg: Color, bg: Color) -> f32 {
    grounds(c)
        .iter()
        .map(|g| drawn_contrast(fg, bg, *g))
        .fold(f32::INFINITY, f32::min)
}

/// What every resolution must give, checked here independently of the resolver's own
/// validation: every text colour and treatment readable where it is drawn, every focus
/// indicator and essential border visible, every colour in gamut.
fn assert_accessible(name: &str, c: &SemanticColours, options: &ResolveOptions) {
    let (t, ui) = (options.text_contrast, options.ui_contrast);
    let transparent = c.surface.canvas.with_alpha(0.0);
    for (what, colour) in [
        ("primary", c.text.primary),
        ("secondary", c.text.secondary),
        ("tertiary", c.text.tertiary),
        ("placeholder", c.text.placeholder),
        ("link", c.text.link),
    ] {
        for under in [transparent, c.surface.hover, c.surface.pressed] {
            let found = worst_drawn(c, colour, under);
            assert!(found >= t, "{name}: text.{what} is {found:.2}:1");
        }
    }
    let accent = &c.roles.accent.solid;
    for state in [&accent.default, &accent.hover, &accent.pressed] {
        let found = c.text.on_accent.contrast(state.background);
        assert!(found >= t, "{name}: on_accent is {found:.2}:1");
    }
    assert!(
        worst_drawn(c, c.text.on_selection, c.surface.selection) >= t,
        "{name}"
    );
    assert!(c.text.inverse.contrast(c.text.primary) >= t, "{name}");
    let disabled = c.text.disabled.contrast(c.surface.canvas);
    assert!(
        (1.999..=3.0).contains(&disabled),
        "{name}: disabled text is {disabled:.2}:1"
    );

    for role in Role::ALL {
        let colours = c.role(role);
        for emphasis in Emphasis::ALL {
            for state in [State::Default, State::Hover, State::Pressed] {
                let tr = colours.emphasis(emphasis).state(state);
                let found = worst_drawn(c, tr.foreground, tr.background);
                assert!(
                    found >= t,
                    "{name}: {role} {} {} is {found:.2}:1",
                    emphasis.name(),
                    state.name()
                );
                if emphasis != Emphasis::Solid {
                    // The focus indicator coexists with hover and press.
                    let focus = worst_drawn(c, colours.focus, tr.background);
                    assert!(focus >= ui, "{name}: {role} focus is {focus:.2}:1");
                }
            }
        }
    }
    for (what, colour) in [
        ("strong", c.border.strong),
        ("hover", c.border.hover),
        ("focus", c.border.focus),
        ("danger", c.border.danger),
    ] {
        for g in grounds(c) {
            let found = colour.over(g).contrast(g);
            assert!(found >= ui, "{name}: border.{what} is {found:.2}:1");
        }
    }
    for colour in every_colour(c) {
        assert!(colour.is_in_gamut(), "{name}: {colour:?}");
    }
}

fn every_colour(c: &SemanticColours) -> Vec<Color> {
    let s = &c.surface;
    let t = &c.text;
    let b = &c.border;
    let mut all = vec![
        s.canvas,
        s.recessed,
        s.sunken,
        s.raised,
        s.overlay,
        s.selection,
        s.hover,
        s.pressed,
        s.scrim,
        t.primary,
        t.secondary,
        t.tertiary,
        t.disabled,
        t.inverse,
        t.link,
        t.placeholder,
        t.on_accent,
        t.on_selection,
        b.subtle,
        b.default,
        b.strong,
        b.hover,
        b.focus,
        b.disabled,
        b.danger,
    ];
    for role in Role::ALL {
        let colours = c.role(role);
        all.push(colours.focus);
        for emphasis in Emphasis::ALL {
            for state in State::ALL {
                let tr = colours.emphasis(emphasis).state(state);
                all.extend([tr.background, tr.foreground, tr.border]);
            }
        }
    }
    all
}

#[test]
fn every_stock_theme_and_fixture_is_accessible_under_the_defaults() {
    for (name, palette) in every_palette() {
        let resolution =
            resolve(&palette, &ResolveOptions::default()).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_accessible(&name, &resolution.colours, &ResolveOptions::default());
        assert_eq!(resolution.colours.mode, palette.mode, "{name}");
    }
}

/// The plan's "no new hue", made concrete: every colour the resolver emits, ignoring alpha, is
/// a grey or has the hue of a colour the palette has, and is no more colourful than the
/// palette's most colourful colour. Lightness is all it changes.
#[test]
fn no_theme_gains_a_hue() {
    for (name, palette) in every_palette() {
        let hues: Vec<_> = palette.colours().map(|(_, c)| c.to_oklch()).collect();
        let most = hues.iter().map(|o| o.c).fold(0.0, f32::max);
        for colour in every_colour(&defaults(&palette).colours) {
            let o = colour.with_alpha(1.0).to_oklch();
            assert!(
                o.c <= most + 0.005,
                "{name}: {} is too colourful",
                colour.to_hex()
            );
            if o.c > JND {
                assert!(
                    hues.iter()
                        .any(|p| p.c > JND / 2.0 && hue_distance(p.h, o.h) < 3.0),
                    "{name}: {} has a hue the palette does not",
                    colour.to_hex()
                );
            }
        }
    }
}

#[test]
fn monochrome_themes_stay_grey_and_keep_their_roles() {
    for palette in [fixture("vantablack")].into_iter().chain(
        stock_themes()
            .into_iter()
            .filter(|(n, _)| n == "white")
            .map(|(_, p)| p),
    ) {
        let r = defaults(&palette);
        assert!(r.report.monochrome);
        for colour in every_colour(&r.colours) {
            let o = colour.to_oklch();
            assert!(o.c <= JND, "{} is not grey", colour.to_hex());
        }
        // Danger and success may be near-identical greys, but they stay distinct roles.
        let decision = r.report.decision(Subject::Fill(Role::Danger)).unwrap();
        assert!(matches!(&decision.source, Source::Palette(k) if k.contains(&PaletteKey::Red)));
        let decision = r.report.decision(Subject::Fill(Role::Success)).unwrap();
        assert!(matches!(&decision.source, Source::Palette(k) if k.contains(&PaletteKey::Green)));
    }
    assert!(!defaults(&fixture("tokyo-night")).report.monochrome);
}

/// A palette whose greys are a hair off neutral is still monochrome: the tolerance is one JND
/// of chroma, not equal channels.
#[test]
fn near_greys_count_as_monochrome() {
    let greyish = with(
        DARK,
        &[
            ("accent", "#8d8d8f"),
            ("muted", "#7a7a7a"),
            ("background", "#000000"),
            ("dark_background", "#0a0a0b"),
            ("lighter_background", "#1a1a1b"),
            ("foreground", "#fffffe"),
            ("red", "#a4a4a5"),
            ("yellow", "#cecece"),
            ("green", "#b6b6b7"),
            ("blue", "#8d8d8d"),
        ],
    );
    let r = defaults(&greyish);
    assert!(r.report.monochrome, "max chroma {}", r.report.max_chroma);
    for colour in every_colour(&r.colours) {
        assert!(colour.to_oklch().c <= JND, "{}", colour.to_hex());
    }
}

/// The question the brief asks the diagnostics to answer.
#[test]
fn the_report_says_why_tokyo_night_s_warning_is_its_regular_yellow() {
    let r = defaults(&fixture("tokyo-night"));
    let fill = r.report.decision(Subject::Fill(Role::Warning)).unwrap();
    assert_eq!(fill.source, Source::Palette(vec![PaletteKey::Yellow]));
    assert_eq!(fill.reason, Reason::Passes);
    assert!(!fill.adjusted());
    let bright = fill
        .considered
        .iter()
        .find(|c| c.keys == [PaletteKey::BrightYellow])
        .unwrap();
    assert!(bright.passes, "bright_yellow would pass too");
    let orange = fill
        .considered
        .iter()
        .find(|c| c.keys == [PaletteKey::Orange])
        .unwrap();
    assert!(
        !orange.in_family,
        "orange is another hue family than yellow"
    );
    let text = fill.to_string();
    assert!(text.contains("`yellow` #e0af68"), "{text}");
    assert!(text.contains("the first candidate that passes"), "{text}");
    assert!(text.contains("`bright_yellow` #ff9e64"), "{text}");
    assert!(text.contains("passes as written"), "{text}");
    // And the label it was checked with.
    let (keys, label) = fill.label.clone().unwrap();
    assert_eq!(keys, [PaletteKey::Background]);
    assert!(label.contrast(fill.resolved) >= 4.5);
}

/// Bright is a name, not a promise: when the regular colour passes, it is used even where the
/// bright one is more colourful; when only the bright one passes, it is used as written rather
/// than adjusting the regular one.
#[test]
fn regular_and_bright_candidates_are_judged_not_assumed() {
    // Both pass; bright is more saturated and lighter: the regular one wins on priority.
    let p = with(DARK, &[("red", "#e06c75"), ("bright_red", "#ff4060")]);
    let r = defaults(&p);
    let ink = r.report.decision(Subject::Fill(Role::Danger)).unwrap();
    assert_eq!(ink.source, Source::Palette(vec![PaletteKey::Red]));

    // The regular red is too dark for text; the bright one passes as written.
    let p = with(DARK, &[("red", "#8a1c2c"), ("bright_red", "#ff7a93")]);
    let r = defaults(&p);
    let ink = r.report.decision(Subject::Ink(Role::Danger)).unwrap();
    assert_eq!(ink.source, Source::Palette(vec![PaletteKey::BrightRed]));
    assert!(!ink.adjusted());

    // A "bright" red that is darker than the regular one, and fails, is passed over.
    let p = with(DARK, &[("red", "#ff7a93"), ("bright_red", "#8a1c2c")]);
    let r = defaults(&p);
    let ink = r.report.decision(Subject::Ink(Role::Danger)).unwrap();
    assert_eq!(ink.source, Source::Palette(vec![PaletteKey::Red]));
}

#[test]
fn identical_regular_and_bright_colours_are_one_candidate_under_both_names() {
    let r = defaults(&fixture("catppuccin-latte"));
    let fill = r.report.decision(Subject::Fill(Role::Danger)).unwrap();
    assert_eq!(
        fill.source,
        Source::Palette(vec![PaletteKey::Red, PaletteKey::BrightRed])
    );
    assert_eq!(fill.considered.len(), 1);
}

#[test]
fn missing_bright_and_optional_colours_are_skipped() {
    let r = defaults(&fixture("custom-incomplete"));
    let danger = r.report.decision(Subject::Fill(Role::Danger)).unwrap();
    assert_eq!(danger.considered.len(), 1, "only `red`");
    // No cyan at all: info falls to blue.
    let info = r.report.decision(Subject::Fill(Role::Info)).unwrap();
    assert!(matches!(&info.source, Source::Palette(k) if k.contains(&PaletteKey::Blue)));
    // No darker_background: sunken is one more step down from dark_background.
    let sunken = r
        .report
        .decision(Subject::Surface(odk_theme::semantic::SurfaceToken::Sunken))
        .unwrap();
    assert!(matches!(sunken.source, Source::Rule(_)));
    let (canvas, recessed, sunken) = (
        r.colours.surface.canvas.lightness(),
        r.colours.surface.recessed.lightness(),
        r.colours.surface.sunken.lightness(),
    );
    assert!(
        canvas > recessed && recessed > sunken,
        "{canvas} {recessed} {sunken}"
    );
    // No selection key: the selection is the accent, translucent.
    let selection = r.colours.surface.selection;
    assert!(selection.a < 1.0);
    assert!(selection.same_rgba8(fixture("custom-incomplete").accent.with_alpha(selection.a)));
}

/// A colour too faint for text is adjusted, not rejected: only its lightness moves, as little
/// as it takes, and its hue stays.
#[test]
fn low_contrast_colours_are_adjusted_in_lightness_only() {
    let r = defaults(&fixture("catppuccin-latte"));
    let ink = r.report.decision(Subject::Ink(Role::Success)).unwrap();
    assert!(ink.adjusted());
    assert!(ink.contrast_before.unwrap() < 4.5);
    let after = ink.contrast_after.unwrap();
    assert!(
        (4.5..4.8).contains(&after),
        "no further than needed: {after}"
    );
    let (before, resolved) = (ink.original_lch(), ink.resolved_lch());
    assert!(
        hue_distance(before.h, resolved.h) < 2.0,
        "{before:?} {resolved:?}"
    );
    assert!(resolved.c <= before.c + 0.002);
    assert!(resolved.l < before.l, "darker on a light theme");
    assert_eq!(ink.reason, Reason::LeastChange);
}

/// Adaptive keeps a role in its own hue family and adjusts it; strict takes another candidate
/// that passes as written; manual uses only the override.
#[test]
fn strategies_rank_differently() {
    // Cyan is too dark for text; blue, another hue family, passes as written.
    let p = with(DARK, &[("cyan", "#1d6a75"), ("blue", "#a8c4ff")]);
    let adaptive = defaults(&p);
    let ink = adaptive.report.decision(Subject::Ink(Role::Info)).unwrap();
    assert_eq!(ink.source, Source::Palette(vec![PaletteKey::Cyan]));
    assert!(ink.adjusted());
    assert_eq!(ink.reason, Reason::LeastChange);

    let strict = ResolveOptions {
        strategy: Strategy::Strict,
        ..Default::default()
    };
    let r = resolve(&p, &strict).unwrap();
    let ink = r.report.decision(Subject::Ink(Role::Info)).unwrap();
    assert!(matches!(&ink.source, Source::Palette(k) if k.contains(&PaletteKey::Blue)));
    assert!(!ink.adjusted());
    assert_eq!(ink.reason, Reason::Passes);
    assert_accessible("strict", &r.colours, &strict);

    // When nothing passes as written, strict adjusts the first candidate.
    let none = with(DARK, &[("cyan", "#1d6a75"), ("blue", "#2a3f80")]);
    let r = resolve(&none, &strict).unwrap();
    let ink = r.report.decision(Subject::Ink(Role::Info)).unwrap();
    assert_eq!(ink.source, Source::Palette(vec![PaletteKey::Cyan]));
    assert_eq!(ink.reason, Reason::FirstAdjusted);
}

#[test]
fn overrides_change_the_order_candidates_are_tried_in() {
    let tokyo = fixture("tokyo-night");
    let mut options = ResolveOptions::default();
    options.overrides.set(
        Role::Warning,
        RoleOverride {
            preferred: PaletteKey::BrightYellow,
            fallback: vec![PaletteKey::Yellow],
        },
    );
    for strategy in [Strategy::Adaptive, Strategy::Strict, Strategy::Manual] {
        let options = ResolveOptions {
            strategy,
            ..options.clone()
        };
        let r = resolve(&tokyo, &options).unwrap();
        let fill = r.report.decision(Subject::Fill(Role::Warning)).unwrap();
        assert_eq!(
            fill.source,
            Source::Palette(vec![PaletteKey::BrightYellow]),
            "{strategy:?}"
        );
        assert_eq!(
            r.colours.roles.warning.solid.default.background,
            hex("#ff9e64")
        );
        assert_accessible("override", &r.colours, &options);
    }
    // The palette itself is never changed.
    assert_eq!(tokyo, fixture("tokyo-night"));
}

/// An override cannot make a colour unreadable: manual honours the preferred key when it
/// passes, and otherwise says so and falls back, or adjusts it.
#[test]
fn overrides_cannot_switch_off_contrast() {
    let tokyo = fixture("tokyo-night");
    let mut options = ResolveOptions {
        strategy: Strategy::Manual,
        ..Default::default()
    };
    // The canvas's own colour as the danger colour: unreadable as written.
    options.overrides.set(
        Role::Danger,
        RoleOverride {
            preferred: PaletteKey::Background,
            fallback: vec![PaletteKey::Red],
        },
    );
    let r = resolve(&tokyo, &options).unwrap();
    assert_accessible("manual", &r.colours, &options);
    let ink = r.report.decision(Subject::Ink(Role::Danger)).unwrap();
    assert!(
        matches!(&ink.source, Source::Palette(k) if k.contains(&PaletteKey::Red)),
        "the fallback, which passes after adjusting less, or as written"
    );
    let note = r
        .report
        .notes
        .iter()
        .find(|n| {
            matches!(
                n,
                Note::PreferredFails {
                    subject: Subject::Ink(Role::Danger),
                    ..
                }
            )
        })
        .expect("a note saying the preferred key failed");
    let text = note.to_string();
    assert!(
        text.contains("`background`") && text.contains("4.5:1"),
        "{text}"
    );

    // With no fallback, the preferred key itself is adjusted until it passes.
    options.overrides.set(
        Role::Danger,
        RoleOverride {
            preferred: PaletteKey::Muted,
            fallback: vec![],
        },
    );
    let r = resolve(&tokyo, &options).unwrap();
    assert_accessible("manual adjusted", &r.colours, &options);
    let ink = r.report.decision(Subject::Ink(Role::Danger)).unwrap();
    assert_eq!(ink.source, Source::Palette(vec![PaletteKey::Muted]));
    assert!(ink.adjusted());
}

#[test]
fn an_override_naming_a_key_the_theme_lacks_is_a_note_not_an_error() {
    let mut options = ResolveOptions {
        strategy: Strategy::Manual,
        ..Default::default()
    };
    options.overrides.set(
        Role::Danger,
        RoleOverride {
            preferred: PaletteKey::BrightRed,
            fallback: vec![],
        },
    );
    let r = resolve(&fixture("custom-incomplete"), &options).unwrap();
    assert!(r.report.notes.contains(&Note::MissingOverrideKey {
        role: Role::Danger,
        key: PaletteKey::BrightRed
    }));
    let fill = r.report.decision(Subject::Fill(Role::Danger)).unwrap();
    assert_eq!(fill.source, Source::Palette(vec![PaletteKey::Red]));
}

#[test]
fn options_from_toml_reach_the_resolver() {
    let options = ResolveOptions::from_toml(
        "[colors]\nstrategy = \"strict\"\n[colors.info]\npreferred = \"blue\"\n",
    )
    .unwrap();
    let r = resolve(&fixture("tokyo-night"), &options).unwrap();
    let fill = r.report.decision(Subject::Fill(Role::Info)).unwrap();
    assert!(matches!(&fill.source, Source::Palette(k) if k.contains(&PaletteKey::Blue)));
    assert_eq!(r.report.options, options);
}

#[test]
fn invalid_options_and_unreachable_contrast_are_errors_not_colours() {
    let tokyo = fixture("tokyo-night");
    let nan = ResolveOptions {
        text_contrast: f32::NAN,
        ..Default::default()
    };
    assert!(matches!(
        resolve(&tokyo, &nan),
        Err(ResolveError::Options(OptionsError::Contrast { .. }))
    ));
    // 21:1 is black on white, which no text on Tokyo Night's surfaces can reach.
    let max = ResolveOptions {
        text_contrast: 21.0,
        ..Default::default()
    };
    let err = resolve(&tokyo, &max).unwrap_err();
    assert!(
        matches!(err, ResolveError::Unreachable { required: 21.0, .. }),
        "{err}"
    );
    assert!(err.to_string().contains("21:1"), "{err}");
}

/// Stricter options are met too, wherever the theme makes them possible.
#[test]
fn stricter_contrast_is_met_where_it_can_be() {
    let options = ResolveOptions {
        text_contrast: 7.0,
        ui_contrast: 4.5,
        ..Default::default()
    };
    for name in [
        "tokyo-night",
        "catppuccin",
        "vantablack",
        "catppuccin-latte",
        "nord",
    ] {
        let r = resolve(&fixture(name), &options).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_accessible(name, &r.colours, &options);
        let surfaces: Vec<_> = r
            .report
            .adjusted()
            .filter(|d| matches!(d.subject, Subject::Surface(_)))
            .map(|d| d.subject)
            .collect();
        // No text reaches 7:1 in a pressed fill on Nord's raised surface, so that surface
        // gives way; every other theme keeps its surfaces.
        let expected = if name == "nord" {
            use odk_theme::semantic::SurfaceToken as S;
            vec![Subject::Surface(S::Raised), Subject::Surface(S::Overlay)]
        } else {
            vec![]
        };
        assert_eq!(surfaces, expected, "{name}");
    }
}

#[test]
fn identical_inputs_give_identical_results() {
    for (name, palette) in every_palette() {
        let first = defaults(&palette);
        let second = defaults(&palette);
        assert_eq!(first, second, "{name}");
        let reparsed = Palette::parse(&format!(
            "mode = \"{}\"\n{}",
            palette.mode.name(),
            palette
                .colours()
                .map(|(k, c)| format!("{} = \"{}\"\n", k.name(), c.to_hex()))
                .collect::<String>()
        ))
        .unwrap();
        assert_eq!(defaults(&reparsed), first, "{name} from the same values");
    }
}

#[test]
fn light_and_dark_themes_keep_their_direction() {
    let latte = defaults(&fixture("catppuccin-latte")).colours;
    assert_eq!(latte.mode, Mode::Light);
    assert!(latte.text.primary.lightness() < latte.surface.canvas.lightness());
    let mocha = defaults(&fixture("catppuccin")).colours;
    assert_eq!(mocha.mode, Mode::Dark);
    assert!(mocha.text.primary.lightness() > mocha.surface.canvas.lightness());
    // Surfaces are the theme's own, unchanged.
    let p = fixture("catppuccin-latte");
    assert_eq!(latte.surface.canvas, p.background);
    assert_eq!(latte.surface.raised, p.lighter_background);
    assert_eq!(latte.surface.recessed, p.dark_background);
    assert_eq!(Some(latte.surface.sunken), p.darker_background);
    assert_eq!(Some(latte.surface.selection), p.selection);
}

/// Hover is a visible step from the default, pressed a bigger one, and neither changes the hue
/// or costs contrast; disabled is neutral and fainter.
#[test]
fn interaction_states_step_away_from_the_default() {
    for name in FIXTURES {
        let c = defaults(&fixture(name)).colours;
        let canvas = c.surface.canvas;
        for role in Role::ALL {
            for emphasis in Emphasis::ALL {
                let e = c.role(role).emphasis(emphasis);
                let drawn = |s: State| e.state(s).background.over(canvas);
                let hover = drawn(State::Hover).apparent_distance(drawn(State::Default));
                let pressed = drawn(State::Pressed).apparent_distance(drawn(State::Default));
                assert!(
                    hover >= 2.0 * JND - 0.005,
                    "{name} {role} {}: hover moved {hover}",
                    emphasis.name()
                );
                assert!(pressed > hover, "{name} {role} {}", emphasis.name());
                // A subtle treatment has no fill in either state; its text tells them apart.
                let disabled = e.disabled;
                assert_eq!(disabled.foreground, c.text.disabled);
                assert_ne!(disabled, e.default, "{name} {role}");
                assert!(disabled.background.a < 1.0, "never a solid fill");
            }
            // The solid fill's states keep its hue.
            let solid = &c.role(role).solid;
            let (d, p) = (
                solid.default.background.to_oklch(),
                solid.pressed.background.to_oklch(),
            );
            if d.c > JND {
                assert!(hue_distance(d.h, p.h) < 3.0, "{name} {role}");
            }
        }
        // Disabled reads as disabled next to every enabled label.
        assert!(c.text.disabled.contrast(canvas) < c.text.tertiary.contrast(canvas));
    }
}

/// Muted and subtle fills are translucent and checked as drawn over every surface: the same
/// token reads on the canvas and on a raised panel.
#[test]
fn translucent_fills_are_checked_after_compositing() {
    let c = defaults(&fixture("nord")).colours;
    let muted = &c.roles.danger.muted.pressed;
    assert!(muted.background.a < 1.0);
    for g in c.surface.grounds() {
        let drawn = muted.background.over(g);
        assert!(muted.foreground.contrast(drawn) >= 4.5);
        // Measured against the abstract colour instead, the result would differ.
        assert_ne!(drawn, muted.background);
    }
    let subtle = &c.roles.info.subtle.default;
    assert_eq!(
        subtle.background.a, 0.0,
        "a subtle treatment has no fill until hovered"
    );
}

/// Text tokens form a hierarchy without giving up readability.
#[test]
fn text_has_a_readable_hierarchy() {
    for name in ["tokyo-night", "nord", "catppuccin", "vantablack"] {
        let c = defaults(&fixture(name)).colours;
        let canvas = c.surface.canvas;
        let (p, s, t) = (
            c.text.primary.contrast(canvas),
            c.text.secondary.contrast(canvas),
            c.text.tertiary.contrast(canvas),
        );
        assert!(p >= s && s >= t && t >= 4.5, "{name}: {p} {s} {t}");
        assert_eq!(c.text.placeholder, c.text.tertiary);
        assert_eq!(c.text.on_accent, c.roles.accent.solid.default.foreground);
    }
}

#[test]
fn the_link_is_the_accent_and_is_reported_when_colour_alone_does_not_set_it_apart() {
    let r = defaults(&fixture("vantablack"));
    let link = r.report.decision(Subject::Text(TextToken::Link)).unwrap();
    assert!(matches!(&link.source, Source::Palette(k) if k.contains(&PaletteKey::Accent)));
    let r = defaults(&fixture("catppuccin-latte"));
    assert!(
        r.report
            .notes
            .iter()
            .any(|n| matches!(n, Note::LinkNearText { .. }))
    );
}

/// The resolver is a plain function: no window, no file, no theme name. A palette parsed from
/// text resolves the same as one loaded from disk.
#[test]
fn the_resolver_needs_no_window_file_or_name() {
    let text = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/nord/colors.toml"),
    )
    .unwrap();
    let from_text = Palette::parse(&text).unwrap();
    assert_eq!(defaults(&from_text), defaults(&fixture("nord")));
}

/// Some themes write `darker_background` as a mid grey that no text reads on (four of the
/// user's, generated by Aether). That surface alone is moved, as little as it takes for the
/// theme's own foreground; every surface text can already be read on stays as written.
#[test]
fn a_surface_no_text_reads_on_is_moved_just_enough() {
    let light = r##"
mode = "light"
accent = "#3b5bdb"
muted = "#868e96"
background = "#f5f1ea"
dark_background = "#c9c3b8"
darker_background = "#7e786f"
lighter_background = "#ebe6de"
foreground = "#111111"
red = "#c92a2a"
yellow = "#e67700"
green = "#2b8a3e"
blue = "#3b5bdb"
"##;
    let p = Palette::parse(light).unwrap();
    let r = defaults(&p);
    let s = &r.colours.surface;
    assert_eq!(
        (s.canvas, s.recessed, s.raised, s.overlay),
        (
            p.background,
            p.dark_background,
            p.lighter_background,
            p.lighter_background
        )
    );
    let sunken = r
        .report
        .decision(Subject::Surface(odk_theme::semantic::SurfaceToken::Sunken))
        .unwrap();
    assert!(sunken.adjusted());
    assert!(sunken.contrast_before.unwrap() < 4.5);
    let after = sunken.contrast_after.unwrap();
    assert!((4.5..4.75).contains(&after), "just enough: {after}");
    assert!(s.sunken.lightness() > p.darker_background.unwrap().lightness());
    assert!(
        s.sunken.lightness() < s.recessed.lightness(),
        "still the deepest well"
    );
    assert_accessible("aether-like", &r.colours, &ResolveOptions::default());

    // No stock theme has such a surface: all of them keep every surface as written.
    for (name, palette) in stock_themes() {
        let s = defaults(&palette).colours.surface;
        assert_eq!(s.recessed, palette.dark_background, "{name}");
        assert_eq!(s.raised, palette.lighter_background, "{name}");
        assert_eq!(Some(s.sunken), palette.darker_background, "{name}");
    }
}

/// The user's own themes are reported, not asserted, as in `themes.rs`: the tests must not
/// depend on them.
#[test]
fn the_user_s_own_themes_are_reported() {
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let Ok(entries) = fs::read_dir(PathBuf::from(home).join(".config/omarchy/themes")) else {
        return;
    };
    let (mut resolved, mut total) = (0, 0);
    for dir in entries.flatten().map(|e| e.path()) {
        let Ok(palette) = Palette::load(&dir.join("colors.toml")) else {
            continue;
        };
        total += 1;
        match resolve(&palette, &ResolveOptions::default()) {
            Ok(_) => resolved += 1,
            Err(e) => eprintln!("does not resolve: {}: {e}", dir.display()),
        }
    }
    eprintln!("{resolved} of the user's {total} loadable themes resolve");
}

#[test]
#[ignore = "prints the reports of three fixtures, for reading"]
fn print_reports() {
    for name in ["tokyo-night", "catppuccin-latte", "vantablack"] {
        eprintln!("===== {name}\n{}", defaults(&fixture(name)).report);
    }
}
