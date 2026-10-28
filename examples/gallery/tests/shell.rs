//! The gallery's shell: the sidebar, the search, the toolbar's appearance controls, the theme
//! preview and the state they leave alone. Each test drives the window as a person does, on
//! GPUI's test platform, and reads what the application then holds.

mod common;

use std::sync::Arc;

use common::{FIXTURES, click, fixture, focused, open, press, serial, settle, tab_to};
use gallery::{Page, SidebarContent, search};
use odk::gpui::{TestAppContext, px, size};
use odk::prelude::Density;
use odk::theme::Mode;
use odk::theme::foundation::CornerRadius;
use odk::ui::test_support::redraw;

/// Every page can be shown and is then drawn.
#[test]
fn every_page_can_be_shown() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Buttons);
    for page in Page::ALL.into_iter().filter(|p| *p != Page::Buttons) {
        let before = gallery::page_renders();
        gallery.update(cx, |g, cx| g.show(page, cx));
        settle(cx);
        assert_eq!(gallery.read_with(cx, |g, _| g.current()), page);
        assert!(
            gallery::page_renders() > before,
            "{page:?} was not drawn when shown"
        );
    }
}

/// A click on a row of the sidebar opens its page; the rows are in the order of `Page::ALL`
/// (the headings take no row).
#[test]
fn a_click_in_the_sidebar_opens_a_page() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Overview);
    let current = |gallery: &odk::gpui::Entity<gallery::Gallery>,
                   cx: &mut odk::gpui::VisualTestContext| {
        gallery.read_with(cx, |g, _| g.current())
    };
    assert_eq!(Page::ALL[1], Page::Buttons);
    click(cx, "pages/item-1");
    assert_eq!(current(&gallery, cx), Page::Buttons);
    assert_eq!(Page::ALL[10], Page::Colors);
    click(cx, "pages/item-10");
    assert_eq!(current(&gallery, cx), Page::Colors);
    click(cx, "pages/item-0");
    assert_eq!(current(&gallery, cx), Page::Overview);
}

/// The sidebar is one Tab stop; the arrow keys move inside it without opening anything, and
/// Enter opens the page the focus is on.
#[test]
fn the_keyboard_walks_the_sidebar() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Overview);
    tab_to(cx, "pages");
    assert!(focused(cx).starts_with("pages"), "{}", focused(cx));
    press(cx, "down down");
    assert_eq!(
        gallery.read_with(cx, |g, _| g.current()),
        Page::Overview,
        "moving the focus opens nothing"
    );
    press(cx, "enter");
    assert_eq!(
        gallery.read_with(cx, |g, _| g.current()),
        Page::Fields,
        "two rows down from Overview is Text fields"
    );
    // One stop: Tab leaves the list instead of walking its rows.
    press(cx, "tab");
    assert!(!focused(cx).starts_with("pages"), "{}", focused(cx));
}

/// Searching: the sidebar shows the results in place of the pages, an empty search says so,
/// choosing a result opens it and puts the pages back.
#[test]
fn search_replaces_the_pages_with_results_and_choosing_one_opens_it() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Overview);
    let content = |gallery: &odk::gpui::Entity<gallery::Gallery>,
                   cx: &mut odk::gpui::VisualTestContext| {
        gallery.read_with(cx, |g, _| g.sidebar_content())
    };
    assert_eq!(content(&gallery, cx), SidebarContent::Pages);

    gallery.update(cx, |g, cx| g.set_query("space.4", cx));
    settle(cx);
    assert!(matches!(content(&gallery, cx), SidebarContent::Results(n) if n >= 1));
    // The results are the list the keyboard reaches.
    tab_to(cx, "results");
    assert_eq!(
        gallery.read_with(cx, |g, _| g.hits()[0].page),
        Page::Spacing
    );
    click(cx, "results/item-0");
    assert_eq!(gallery.read_with(cx, |g, _| g.current()), Page::Spacing);
    assert_eq!(gallery.read_with(cx, |g, _| g.query().to_owned()), "");
    assert_eq!(content(&gallery, cx), SidebarContent::Pages);

    // Nothing matches: no list, and no page changes.
    gallery.update(cx, |g, cx| g.set_query("zzzzzz", cx));
    settle(cx);
    assert_eq!(content(&gallery, cx), SidebarContent::NoMatches);
    assert!(gallery.read_with(cx, |g, _| g.hits().is_empty()));
    assert_eq!(gallery.read_with(cx, |g, _| g.current()), Page::Spacing);
}

/// Typing in the search field and pressing Enter opens the best match.
#[test]
fn enter_in_the_search_field_opens_the_first_result() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Overview);
    click(cx, "search/box");
    cx.simulate_input("tooltip");
    settle(cx);
    assert_eq!(
        gallery.read_with(cx, |g, _| g.query().to_owned()),
        "tooltip"
    );
    press(cx, "enter");
    assert_eq!(gallery.read_with(cx, |g, _| g.current()), Page::Overlays);
    assert_eq!(gallery.read_with(cx, |g, _| g.query().to_owned()), "");
}

/// What the registry's search finds, by component, category, token, role and keyword.
#[test]
fn search_finds_components_categories_tokens_roles_and_keywords() {
    let first = |q: &str| search(q).first().map(|h| h.page);
    assert_eq!(first("button"), Some(Page::Buttons));
    assert_eq!(first("Buttons"), Some(Page::Buttons));
    assert_eq!(
        first("combobox"),
        Some(Page::Selection),
        "the searchable list"
    );
    assert_eq!(first("feedback"), Some(Page::Feedback));
    assert_eq!(first("space.4"), Some(Page::Spacing));
    assert_eq!(first("roles.warning.muted"), Some(Page::Colors));
    assert_eq!(first("warning solid fill"), Some(Page::Resolver));
    assert_eq!(first("palette.bright_yellow"), Some(Page::Colors));
    assert_eq!(first("motion.fast"), Some(Page::Motion));
    assert_eq!(first("type.body"), Some(Page::Typography));
    assert_eq!(first("radius.default"), Some(Page::Geometry));
    assert_eq!(first("tooltip"), Some(Page::Overlays));
    assert_eq!(
        first("escape"),
        Some(Page::Interaction),
        "from its notes' words"
    );
    // A token hit says which token matched.
    let hit = &search("roles.warning.muted")[0];
    assert!(
        hit.matched
            .as_deref()
            .is_some_and(|m| m.contains("roles.warning.muted"))
    );
    // Every word must match; an empty query matches nothing.
    assert!(search("button zzzz").is_empty());
    assert!(search("   ").is_empty());
}

/// The toolbar's density, corners and motion change what every page draws, at once and without
/// moving the page.
#[test]
fn the_toolbar_sets_density_corners_and_motion_for_every_page() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Buttons);
    let config =
        |cx: &mut odk::gpui::VisualTestContext| cx.update(|_, cx| odk::ui::foundation_config(cx));
    let height = |cx: &mut odk::gpui::VisualTestContext| {
        cx.debug_bounds("variant-0/button").unwrap().size.height
    };

    let default = height(cx);
    click(cx, "density/item-0");
    assert_eq!(config(cx).density, Density::Compact);
    let compact = height(cx);
    assert!(compact < default, "{compact:?} before {default:?}");
    click(cx, "density/item-2");
    assert_eq!(config(cx).density, Density::Comfortable);
    assert!(height(cx) > default);
    click(cx, "density/item-1");
    assert_eq!(height(cx), default);

    click(cx, "corners/item-0");
    assert_eq!(config(cx).corner_radius, CornerRadius::Square);
    assert_eq!(odk::ui::foundation().geometry.radius.default, 0.0);
    click(cx, "corners/item-1");
    assert_eq!(config(cx).corner_radius, CornerRadius::Subtle);
    assert_eq!(
        odk::ui::foundation().geometry.radius.default,
        CornerRadius::Subtle.px()
    );

    // Motion: the gallery's own switch, Full then Reduced.
    click(cx, "motion/item-0");
    assert!(!odk::ui::foundation().motion.reduced);
    click(cx, "motion/item-1");
    assert!(odk::ui::foundation().motion.reduced);
    assert!(config(cx).reduced_motion);

    // The content switch is the gallery's, not the foundation's.
    click(cx, "content/item-1");
    click(cx, "content/item-0");
    assert_eq!(gallery.read_with(cx, |g, _| g.current()), Page::Buttons);
}

/// Reset puts theme, density, corners and motion back, and leaves the page where it was.
#[test]
fn reset_restores_the_appearance_and_keeps_the_page() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Fields);
    click(cx, "density/item-2");
    click(cx, "corners/item-0");
    gallery.update(cx, |g, cx| g.preview_theme(&fixture("nord"), cx));
    settle(cx);
    click(cx, "reset-appearance/button");
    let config = cx.update(|_, cx| odk::ui::foundation_config(cx));
    assert_eq!(config.density, Density::Default);
    assert_eq!(config.corner_radius, CornerRadius::default());
    assert!(!config.reduced_motion);
    assert!(gallery.read_with(cx, |g, cx| g.previewing(cx)).is_none());
    assert_eq!(gallery.read_with(cx, |g, _| g.current()), Page::Fields);
}

/// A theme is previewed in place of the system's, and Omarchy's is never touched: the followed
/// theme is the same snapshot before, during and after, and following the system draws it again.
#[test]
fn previewing_a_theme_leaves_the_system_theme_alone() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Buttons);
    let system = cx.update(|_, cx| odk::ui::followed_theme(cx));
    let before = odk::ui::theme_snapshot();
    assert!(Arc::ptr_eq(&system, &before));

    assert!(gallery.update(cx, |g, cx| g.preview_theme(&fixture("nord"), cx)));
    settle(cx);
    assert_eq!(
        gallery.read_with(cx, |g, cx| g.previewing(cx)).as_deref(),
        Some("nord")
    );
    assert_eq!(odk::ui::theme_snapshot().name.as_deref(), Some("nord"));
    assert!(Arc::ptr_eq(
        &system,
        &cx.update(|_, cx| odk::ui::followed_theme(cx))
    ));

    gallery.update(cx, |g, cx| g.follow_system(cx));
    settle(cx);
    assert!(gallery.read_with(cx, |g, cx| g.previewing(cx)).is_none());
    assert!(Arc::ptr_eq(&system, &odk::ui::theme_snapshot()));
}

/// What a person set survives every change of appearance: the page, the page's own state (the
/// playground's variant), the search text and the worst-case content.
#[test]
fn appearance_changes_keep_the_state_of_the_page() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Playground);
    gallery.update(cx, |g, cx| {
        g.playground().update(cx, |p, cx| {
            p.button.variant = odk::ui::ButtonVariant::Destructive;
            cx.notify();
        });
        g.set_worst_case(true, cx);
    });
    settle(cx);

    click(cx, "density/item-0");
    click(cx, "corners/item-0");
    click(cx, "motion/item-0");
    gallery.update(cx, |g, cx| {
        g.preview_theme(&fixture("catppuccin-latte"), cx)
    });
    settle(cx);
    gallery.update(cx, |g, cx| g.follow_system(cx));
    cx.simulate_resize(size(px(900.0), px(700.0)));
    settle(cx);

    assert_eq!(gallery.read_with(cx, |g, _| g.current()), Page::Playground);
    let variant = gallery.read_with(cx, |g, cx| g.playground().read(cx).button.variant);
    assert_eq!(variant, odk::ui::ButtonVariant::Destructive);
}

/// Every page draws with each fixture theme, and the theme's character reaches the tokens: the
/// light theme is light, and the monochrome one stays monochrome in every role.
#[test]
fn every_page_draws_in_every_fixture_theme() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Overview);
    for name in FIXTURES {
        assert!(
            gallery.update(cx, |g, cx| g.preview_theme(&fixture(name), cx)),
            "{name} could not be previewed"
        );
        let theme = odk::ui::theme_snapshot();
        assert_eq!(theme.name.as_deref(), Some(name));
        for page in Page::ALL {
            gallery.update(cx, |g, cx| g.show(page, cx));
            redraw(cx);
        }
        match name {
            "catppuccin-latte" => assert_eq!(theme.palette.mode, Mode::Light),
            "vantablack" => {
                assert!(theme.resolution.monochrome);
                for role in odk::theme::semantic::Role::ALL {
                    let r = theme.semantic.role(role);
                    for c in [
                        r.solid.default.background,
                        r.muted.default.foreground,
                        r.focus,
                    ] {
                        assert!(
                            c.to_oklch().c < 0.02,
                            "{role} has colour in a monochrome theme: {} (chroma {})",
                            c.to_hex(),
                            c.to_oklch().c
                        );
                    }
                }
            }
            _ => assert_eq!(theme.palette.mode, Mode::Dark),
        }
    }
}

/// Resizing keeps the page and the search, and a narrow window gets the narrow sidebar.
#[test]
fn resizing_keeps_the_state_and_narrows_the_sidebar() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Buttons);
    gallery.update(cx, |g, cx| g.set_query("button", cx));
    cx.simulate_resize(size(px(1600.0), px(900.0)));
    settle(cx);
    let wide = cx.debug_bounds("results/list").unwrap().size.width;
    cx.simulate_resize(size(px(800.0), px(600.0)));
    settle(cx);
    let narrow = cx.debug_bounds("results/list").unwrap().size.width;
    assert!(
        narrow < wide,
        "{narrow:?} in a narrow window, {wide:?} in a wide one"
    );
    assert_eq!(gallery.read_with(cx, |g, _| g.current()), Page::Buttons);
    assert_eq!(gallery.read_with(cx, |g, _| g.query().to_owned()), "button");
}
