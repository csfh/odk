//! What the gallery shows of the tokens is what the theme holds: the pages read the published
//! snapshot, the resolver's report and the component tokens' diagnostics, and decide nothing of
//! their own. These tests compare what the pages are built from with what the theme crates
//! say, and scan the source for the shortcuts that would let the two drift apart.

mod common;

use std::path::Path;
use std::sync::Arc;

use common::{fixture, open, serial, settle};
use gallery::colors::{GROUPS, treatment_data};
use gallery::compare::{ComparePage, Side, token_rows};
use gallery::registry::{Page, tokens};
use gallery::resolver::{Focus, decisions, explain, notes};
use gallery::spacing::{LAYOUT, STEPS};
use gallery::themes::Loaded;
use odk::gpui::TestAppContext;
use odk::theme::foundation::{FoundationTokens, TypeRole};
use odk::theme::semantic::{Emphasis, Role, State, Subject};
use odk::theme::{PaletteKey, ThemeManager, ThemeSource, drawn_contrast};

fn load(name: &str) -> Arc<odk::theme::ThemeSnapshot> {
    let mut manager = ThemeManager::new(ThemeSource::Dir(fixture(name).into()));
    manager.reload();
    manager.snapshot()
}

/// The page table is consistent: ids are unique, the nine pages the gallery has always had keep
/// their ids, and every page is findable by its own name.
#[test]
fn the_page_table_is_consistent() {
    let ids: Vec<_> = Page::ALL.iter().map(|p| p.id()).collect();
    for (n, id) in ids.iter().enumerate() {
        assert!(!ids[..n].contains(id), "{id} is used twice");
        assert_eq!(Page::from_id(id), Some(Page::ALL[n]));
    }
    for old in [
        "buttons",
        "fields",
        "selection",
        "form",
        "navigation",
        "feedback",
        "overlays",
        "dialogs",
        "interaction",
    ] {
        assert!(ids.contains(&old), "{old} is no longer a page");
    }
    for page in Page::ALL {
        let meta = page.meta();
        assert!(
            !meta.title.is_empty() && !meta.description.is_empty(),
            "{page:?}"
        );
        assert!(
            gallery::search(meta.title).iter().any(|h| h.page == page),
            "{page:?} cannot be found by its own title"
        );
    }
    // Pages that show a component name it, so the Overview can list it.
    for page in [
        Page::Buttons,
        Page::Fields,
        Page::Selection,
        Page::Navigation,
        Page::Feedback,
        Page::Overlays,
        Page::Dialogs,
    ] {
        assert!(!page.meta().components.is_empty(), "{page:?}");
    }
}

/// What the Overview says is not built is what really is not: no page claims a combobox or
/// pagination, and the table says why.
#[test]
fn the_overview_says_what_is_not_built() {
    let missing: Vec<_> = gallery::overview::NOT_BUILT
        .iter()
        .map(|(what, _)| *what)
        .collect();
    for what in ["Combobox", "Pagination", "Button group"] {
        assert!(missing.contains(&what), "{what}");
    }
    for page in Page::ALL {
        for component in page.meta().components {
            assert!(
                !["Combobox", "Pagination"].contains(component),
                "{page:?} lists {component}, which does not exist"
            );
        }
    }
}

/// Each page's usage example is a real source file under `src/snippets/`, compiled with the
/// gallery, and shown from its markers. These are the pages that have one.
#[test]
fn usage_examples_are_the_compiled_snippets() {
    let all = gallery::snippets::all();
    assert!(all.len() >= 12);
    for (page, code) in &all {
        assert_eq!(gallery::snippets::usage(*page), Some(*code));
        assert!(code.starts_with("use odk::"), "{page:?}: {code}");
        assert!(!code.contains("snippet start") && !code.contains("snippet end"));
        assert!(code.lines().count() >= 5, "{page:?} is a stub");
    }
    for page in [
        Page::Overview,
        Page::Resolver,
        Page::Interaction,
        Page::Playground,
        Page::Compare,
        Page::Diagnostics,
    ] {
        assert!(gallery::snippets::usage(page).is_none(), "{page:?}");
    }
}

/// The palette groups cover every key the palette has, once each.
#[test]
fn the_colour_page_shows_every_palette_key_once() {
    let mut seen = Vec::new();
    for (_, keys) in GROUPS {
        for key in keys {
            assert!(!seen.contains(key), "{} is in two groups", key.name());
            seen.push(*key);
        }
    }
    for key in PaletteKey::ALL {
        assert!(seen.contains(&key), "{} is in no group", key.name());
    }
}

/// A role's row on the colour page is the semantic layer's treatment as it is, with the contrast
/// the resolver's own measure gives; a theme's source colours are its palette's.
#[test]
fn the_colour_page_reads_the_snapshot_as_it_is() {
    for name in ["tokyo-night", "catppuccin-latte", "vantablack"] {
        let theme = load(name);
        for role in Role::ALL {
            for emphasis in Emphasis::ALL {
                let row = treatment_data(&theme.semantic, role, emphasis);
                let t = theme.semantic.treatment(role, emphasis, State::Default);
                assert_eq!(
                    (row.background, row.foreground, row.border),
                    (t.background, t.foreground, t.border),
                    "{name} {role} {emphasis:?}"
                );
                assert_eq!(
                    row.ratio,
                    drawn_contrast(t.foreground, t.background, theme.semantic.surface.canvas)
                );
            }
        }
        // Source colours are the palette's, and a key a theme leaves out is reported as left out.
        assert_eq!(
            theme.palette.colours().count() + theme.palette.missing().count(),
            PaletteKey::ALL.len()
        );
    }
    let sparse = {
        let mut manager = ThemeManager::new(ThemeSource::Dir(fixture("custom-incomplete").into()));
        manager.reload();
        manager.snapshot()
    };
    assert!(sparse.palette.missing().count() > 0);
}

/// The resolver inspector answers "why yellow and not bright yellow?" from the report: the
/// decision's source is the chosen candidate, the other is listed as considered with its own
/// contrast, and every number is the report's.
#[test]
fn the_resolver_inspector_explains_the_reports_decision() {
    let theme = load("tokyo-night");
    let report = &theme.resolution;
    let d = report
        .decision(Subject::Fill(Role::Warning))
        .expect("the warning fill was decided");
    let e = explain(d);
    assert_eq!(e.subject, "warning solid fill");
    assert_eq!(e.original, d.original);
    assert_eq!(e.resolved, d.resolved);
    assert_eq!(e.adjusted, d.adjusted());
    assert_eq!(e.candidates.len(), d.considered.len());
    let chosen: Vec<_> = e.candidates.iter().filter(|c| c.chosen).collect();
    assert_eq!(chosen.len(), 1, "exactly one candidate was chosen");
    assert!(
        e.source.contains(&chosen[0].keys),
        "{} in {}",
        chosen[0].keys,
        e.source
    );
    let bright = e
        .candidates
        .iter()
        .find(|c| c.keys == "`bright_yellow`")
        .expect("bright yellow was considered");
    assert!(!bright.chosen);
    for (shown, held) in e.candidates.iter().zip(&d.considered) {
        assert_eq!(shown.colour, held.colour);
        assert_eq!(shown.contrast, held.contrast);
        assert_eq!(shown.passes, held.passes);
        assert_eq!(shown.in_family, held.in_family);
    }
    assert_eq!(e.reason, d.reason.to_string());
    let after = d.contrast_after.unwrap();
    assert!(
        e.contrast.contains(&format!("{after:.2}:1 as resolved")),
        "{}",
        e.contrast
    );

    // A colour that was adjusted says so and shows both colours.
    let adjusted = report
        .adjusted()
        .next()
        .expect("a theme with an adjusted colour");
    let e = explain(adjusted);
    assert!(e.adjusted);
    assert_ne!(e.original, e.resolved);
    assert!(e.contrast.contains("as written"));
}

/// The inspector lists what the report holds for the role, and nothing it invents: with the
/// decisions gone the page has none to show and says so.
#[test]
fn the_resolver_inspector_shows_only_what_the_report_holds() {
    let theme = load("tokyo-night");
    let report = &theme.resolution;
    let warning = decisions(report, Focus::Role(Role::Warning));
    assert!(!warning.is_empty());
    assert!(warning.iter().all(|d| matches!(
        d.subject,
        Subject::Fill(Role::Warning)
            | Subject::Ink(Role::Warning)
            | Subject::Focus(Role::Warning)
            | Subject::MutedBorder(Role::Warning)
            | Subject::Treatment(Role::Warning, _, _)
    )));
    assert!(
        decisions(report, Focus::Surfaces)
            .iter()
            .all(|d| matches!(d.subject, Subject::Surface(_)))
    );
    // Everything the report decided is in some focus.
    let covered: usize = [
        Focus::Role(Role::Accent),
        Focus::Role(Role::Neutral),
        Focus::Role(Role::Danger),
        Focus::Role(Role::Success),
        Focus::Role(Role::Warning),
        Focus::Role(Role::Info),
        Focus::Surfaces,
        Focus::Text,
        Focus::Borders,
    ]
    .into_iter()
    .map(|f| decisions(report, f).len())
    .sum();
    assert_eq!(covered, report.decisions.len());

    let mut empty = (**report).clone();
    empty.decisions.clear();
    empty.notes.clear();
    assert!(decisions(&empty, Focus::Role(Role::Warning)).is_empty());
    assert!(notes(&empty, Focus::Role(Role::Warning)).is_empty());
    // Every note of the report is shown with its role, or with the surfaces if it has none.
    let per_role: usize = Role::ALL
        .into_iter()
        .map(|r| notes(report, Focus::Role(r)).len())
        .sum();
    assert_eq!(
        per_role + notes(report, Focus::Surfaces).len(),
        report.notes.len()
    );
    assert!(!report.notes.is_empty(), "the fixture has notes to show");
}

/// The spacing, layout and motion pages list the foundation's values, whatever the text size.
#[test]
fn the_foundation_pages_list_the_foundation_values() {
    let f = FoundationTokens::default();
    assert_eq!(
        STEPS.iter().map(|s| (s.value)(&f)).collect::<Vec<_>>(),
        [0.0, 4.0, 8.0, 12.0, 16.0, 20.0, 24.0, 32.0, 48.0]
    );
    assert_eq!(LAYOUT.len(), 6);
    assert_eq!(LAYOUT[0].1(&f.layout), f.layout.panel_padding);
    for (name, duration) in gallery::motion::DURATIONS {
        assert!(
            f.motion.duration(duration).as_millis() > 0 || name == "instant",
            "{name}"
        );
    }
    assert_eq!(TypeRole::ALL.len(), 9);
    // Their token names are what the search finds.
    assert!(
        tokens()
            .iter()
            .any(|(n, p)| n == "space.12" && *p == Page::Spacing)
    );
    assert!(
        tokens()
            .iter()
            .any(|(n, p)| n == "layout.dialog_padding" && *p == Page::Spacing)
    );
    assert!(
        tokens()
            .iter()
            .any(|(n, p)| n == "motion.exit" && *p == Page::Motion)
    );
    assert!(
        tokens()
            .iter()
            .any(|(n, p)| n == "type.body_strong" && *p == Page::Typography)
    );
}

/// Two themes are compared as two snapshots of their own, with nothing taken from the system's:
/// the same theme twice differs nowhere, two themes differ, and choosing sides changes no
/// theme that is drawn.
#[test]
fn comparing_two_themes_uses_two_snapshots_and_touches_nothing() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Compare);
    let system = odk::ui::theme_snapshot();
    for name in ["tokyo-night", "nord", "catppuccin-latte"] {
        gallery.update(cx, |g, cx| {
            g.themes().update(cx, |t, _| t.add(&fixture(name)));
        });
    }
    let comparison = gallery.read_with(cx, |g, _| g.comparison().clone());
    comparison.update(cx, |c, cx| {
        c.pick(Side::A, "tokyo-night", cx);
        c.pick(Side::B, "nord", cx);
    });
    settle(cx);
    let (a, b): (Arc<Loaded>, Arc<Loaded>) = {
        let (a, b) = comparison.read_with(cx, |c, _| c.sides());
        (a.unwrap(), b.unwrap())
    };
    assert_eq!((a.name.as_str(), b.name.as_str()), ("tokyo-night", "nord"));
    assert!(!Arc::ptr_eq(&a.snapshot, &b.snapshot));
    assert_ne!(a.snapshot.palette, b.snapshot.palette);

    let rows = token_rows(&a, &b);
    assert!(
        rows.iter().filter(|r| r.differs()).count() > 20,
        "two themes differ"
    );
    for row in &rows {
        // Each side's colour is its own snapshot's token, by name.
        if row.name == "surface.canvas" {
            assert_eq!(row.a, a.snapshot.semantic.surface.canvas);
            assert_eq!(row.b, b.snapshot.semantic.surface.canvas);
        }
    }
    assert!(
        token_rows(&a, &a).iter().all(|r| !r.differs()),
        "a theme equals itself"
    );
    assert_eq!(
        comparison.read_with(cx, |c: &ComparePage, _| c.differences()),
        Some(rows.iter().filter(|r| r.differs()).count())
    );

    // The system's theme is the very snapshot it was, and nothing is being previewed.
    assert!(Arc::ptr_eq(&system, &odk::ui::theme_snapshot()));
    assert!(gallery.read_with(cx, |g, cx| g.previewing(cx)).is_none());

    // Showing A then B draws each in turn, and the system's is still the one followed.
    gallery.update(cx, |g, cx| g.preview_theme(&fixture("tokyo-night"), cx));
    assert_eq!(
        odk::ui::theme_snapshot().name.as_deref(),
        Some("tokyo-night")
    );
    gallery.update(cx, |g, cx| g.preview_theme(&fixture("nord"), cx));
    assert_eq!(odk::ui::theme_snapshot().name.as_deref(), Some("nord"));
    assert!(Arc::ptr_eq(
        &system,
        &cx.update(|_, cx| odk::ui::followed_theme(cx))
    ));
}

/// Drawing reads no file and resolves no colour: after the themes are loaded, redraws leave the
/// number of reads and the very semantic snapshot as they were.
#[test]
fn drawing_reads_no_theme_files_and_resolves_nothing() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Compare);
    gallery.update(cx, |g, cx| g.preview_theme(&fixture("nord"), cx));
    settle(cx);
    let reads = gallery.read_with(cx, |g, cx| g.themes().read(cx).reads());
    assert!(reads >= 1);
    let semantic = odk::ui::theme_snapshot().semantic.clone();
    for page in Page::ALL {
        gallery.update(cx, |g, cx| g.show(page, cx));
        for _ in 0..3 {
            odk::ui::test_support::redraw(cx);
        }
    }
    // Showing the comparison loads its two themes, once, and no drawing loads more.
    let after_pages = gallery.read_with(cx, |g, cx| g.themes().read(cx).reads());
    for _ in 0..5 {
        odk::ui::test_support::redraw(cx);
    }
    assert_eq!(
        gallery.read_with(cx, |g, cx| g.themes().read(cx).reads()),
        after_pages
    );
    assert!(Arc::ptr_eq(&semantic, &odk::ui::theme_snapshot().semantic));
}

/// The gallery's own source takes its colours from the theme and decides none: no colour
/// literal, no call into the colour maths or the resolver, no second reading of a theme file.
#[test]
fn the_gallery_source_holds_no_colours_and_resolves_none() {
    let forbidden = [
        "rgba(0x",
        "rgb(0x",
        "hsla(",
        "hsl(",
        "gpui::red",
        "gpui::green",
        "gpui::blue",
        "gpui::white",
        "gpui::black",
        "opaque_grey",
        "gpui::transparent",
        // The resolver and the colour maths are the theme crate's.
        "semantic::resolve",
        "resolve(",
        ".mix(",
        ".with_alpha(",
        ".over(",
        "to_oklab",
        "to_oklch",
        "Palette::parse",
        "Palette::load",
        "from_toml(",
        "fs::read",
    ];
    let hex = regex_free_hex_literal;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![dir.clone()];
    let mut checked = 0;
    while let Some(d) = files.pop() {
        for entry in std::fs::read_dir(&d).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.push(path);
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for (n, line) in text.lines().enumerate() {
                let code = line.split("//").next().unwrap_or("");
                for pattern in forbidden {
                    assert!(
                        !code.contains(pattern),
                        "{}:{} uses `{pattern}`: {line}",
                        path.display(),
                        n + 1
                    );
                }
                assert!(
                    !hex(code),
                    "{}:{} has a colour literal: {line}",
                    path.display(),
                    n + 1
                );
            }
            checked += 1;
        }
    }
    assert!(checked > 20);
}

/// The scan finds a colour literal and passes numbers that are not one.
#[test]
fn the_colour_scan_finds_what_it_looks_for() {
    assert!(regex_free_hex_literal("rgba(0x1e1e2eff)"));
    assert!(regex_free_hex_literal("let c = 0xff00aa;"));
    assert!(!regex_free_hex_literal("\"0x7fff_ffff\""));
    assert!(!regex_free_hex_literal("0x1f"));
}

/// `0x` followed by six or eight hex digits, as a number and not inside a longer one.
fn regex_free_hex_literal(code: &str) -> bool {
    let bytes = code.as_bytes();
    let mut i = 0;
    while i + 2 < bytes.len() {
        if bytes[i] == b'0' && bytes[i + 1] == b'x' {
            let digits = bytes[i + 2..]
                .iter()
                .take_while(|b| b.is_ascii_hexdigit())
                .count();
            if digits == 6 || digits == 8 {
                return true;
            }
        }
        i += 1;
    }
    false
}
