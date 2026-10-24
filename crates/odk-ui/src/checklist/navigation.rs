//! The navigation and feedback components together: live theme changes keep their state, a
//! density change moves their geometry and nothing else, the corner setting reaches every
//! rectangle, and they stay grey in a monochrome theme.

use std::fs;

use gpui::{ParentElement, Styled, TestAppContext, px};
use odk_theme::component::{BadgeStyle, NavItemStyle, ProgressStyle, SegmentStyle, TabStyle};
use odk_theme::foundation::{CornerRadius, Density};
use odk_theme::{ThemeManager, ThemeSource};

use super::{FIXTURES, open, press, redraw};
use crate::{Badge, NavList, Progress, Tabs, Text, ToggleGroup, Tone, v_flex};

fn scene() -> gpui::Div {
    v_flex()
        .w(px(480.0))
        .child(
            Tabs::new("t", "Sections")
                .tab("a", "General", |_, _| Text::new("a"))
                .tab("b", "Display", |_, _| Text::new("b"))
                .tab("c", "About", |_, _| Text::new("c")),
        )
        .child(
            ToggleGroup::new("g", "View")
                .item("list", "List")
                .item("grid", "Grid"),
        )
        .child(
            NavList::new("n", "Settings")
                .item("general", "General")
                .item("members", "Members")
                .default_value("general"),
        )
        .child(Progress::new("p", "Upload").value(40.0))
        .child(Badge::new("Synced").tone(Tone::Success))
}

/// Item 3: a theme change recolours tabs, segments, the sidebar, a progress bar and a badge on
/// the next frame, and keeps the selected tab, the chosen segment, the current page, the
/// progress, where focus is, the density and the corners.
#[gpui::test]
fn a_theme_change_keeps_their_state(cx: &mut TestAppContext) {
    let [(_, dark), .., (_, light)] = FIXTURES;
    let cx = open(cx, |_, _| scene());
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
    // Select Display, choose Grid, open Members, and leave focus on Members.
    press(cx, "tab right tab right space tab down enter");
    redraw(cx);
    let tab = || crate::drawn::last::<(TabStyle, f32)>("t/tab-1").0;
    let grid = || crate::drawn::last::<SegmentStyle>("g/item-1");
    let members = || crate::drawn::last::<NavItemStyle>("n/item-1");
    let progress = || crate::drawn::last::<(ProgressStyle, f32, f32)>("p");
    let before = (tab(), grid(), members(), progress());
    assert!(before.0.indicator.is_some() && before.2.indicator.is_some());
    assert!(before.2.focus_visible);
    let bounds = cx.debug_bounds("n/item-1").unwrap();

    fs::write(dir.path().join("colors.toml"), light).unwrap();
    cx.update(|_, cx| crate::reload_theme(cx));
    cx.run_until_parked();
    let after = (tab(), grid(), members(), progress());
    assert_ne!(after.0.current, before.0.current, "recoloured");
    assert_ne!(after.2.current, before.2.current);
    assert_ne!(after.3.0.bar, before.3.0.bar);
    assert!(after.0.indicator.is_some(), "Display is still selected");
    assert_eq!(
        after.1.current.background,
        after.1.colours.default.background
    );
    assert_ne!(
        after.1.current,
        crate::components()
            .segment(Density::Compact, odk_theme::component::States::NONE)
            .current,
        "Grid is still chosen"
    );
    assert!(
        after.2.indicator.is_some() && after.2.focus_visible,
        "Members, focused"
    );
    assert_eq!((after.3.1, after.3.2), (0.4, 0.4), "the progress is kept");
    assert_eq!(
        after.2.geometry, before.2.geometry,
        "same density and corners"
    );
    assert_eq!(cx.debug_bounds("n/item-1").unwrap(), bounds);
}

/// A density change moves their geometry, and no colour; the corner setting reaches every
/// rectangle: a badge, a sidebar row, a progress bar.
#[gpui::test]
fn density_and_corners_reach_them(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| scene());
    let badge = || crate::drawn::last::<BadgeStyle>("odk-badge/Synced");
    let members = || crate::drawn::last::<NavItemStyle>("n/item-1");
    let mut heights = Vec::new();
    for d in Density::ALL {
        cx.update(|_, cx| crate::configure(cx, |c| c.density = d));
        redraw(cx);
        heights.push(cx.debug_bounds("t/tab-0").unwrap().size.height);
        assert_eq!(
            cx.debug_bounds("g/group").unwrap().size.height,
            px(d.metrics().height)
        );
        assert_eq!(members().geometry.height, d.metrics().height);
    }
    assert_eq!(heights, [px(28.0), px(34.0), px(40.0)]);
    let colours = members().colours;
    for (corner, radius) in [(CornerRadius::Square, 0.0), (CornerRadius::Subtle, 2.0)] {
        cx.update(|_, cx| crate::configure(cx, |c| c.corner_radius = corner));
        redraw(cx);
        assert_eq!(badge().geometry.radius, radius);
        assert_eq!(members().geometry.radius, radius);
        assert_eq!(
            crate::drawn::last::<(ProgressStyle, f32, f32)>("p")
                .0
                .radius,
            radius
        );
        assert_eq!(members().colours, colours, "no colour moves");
    }
}

/// Item 18, in Vantablack: every colour they draw is grey, and selection still reads, by the
/// indicator, the fill and the ring.
#[gpui::test]
fn monochrome_stays_monochrome(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| scene());
    let (_, vantablack) = FIXTURES
        .into_iter()
        .find(|(n, _)| *n == "vantablack")
        .unwrap();
    super::publish_fixture(vantablack);
    press(cx, "tab");
    redraw(cx);
    let tab = crate::drawn::last::<(TabStyle, f32)>("t/tab-0").0;
    let nav = crate::drawn::last::<NavItemStyle>("n/item-0");
    let progress = crate::drawn::last::<(ProgressStyle, f32, f32)>("p").0;
    let grey = |c: odk_theme::Color| c.to_oklch().c <= odk_theme::JND;
    for c in [
        tab.current.foreground,
        tab.indicator.unwrap(),
        tab.focus.colour,
        nav.current.background,
        nav.indicator.unwrap(),
        progress.bar,
        progress.track,
    ] {
        assert!(grey(c), "{c:?}");
    }
}
