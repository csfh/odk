//! The Overview page: what the gallery is for, how to drive it, and what it covers, accurately,
//! including what it does not.

use odk::gpui::WeakEntity;
use odk::prelude::*;

use crate::present::{group, kv, page, section};
use crate::registry::{Category, Page};
use crate::{Gallery, themes};

pub struct OverviewPage {
    gallery: WeakEntity<Gallery>,
}

impl OverviewPage {
    pub fn new(gallery: WeakEntity<Gallery>) -> Self {
        Self { gallery }
    }
}

/// What the gallery does not have, and why, so a missing page is not mistaken for a broken one.
pub const NOT_BUILT: [(&str, &str); 7] = [
    (
        "Combobox",
        "ODK has none: the Command palette (Selection page) is its searchable list, on the same field and menu tokens.",
    ),
    (
        "Pagination",
        "No component exists; nothing in ODK needs one yet (docs/plan.md).",
    ),
    (
        "Button group",
        "The segmented control (ToggleGroup, Navigation page) is the grouped button; there is no separate component.",
    ),
    (
        "Forced hover, press and focus on a live control",
        "Not built: it would need a forcing hook in Button's public API. The Buttons page shows each state's tokens from the component token diagnostics instead, and real controls respond to the real pointer and keyboard.",
    ),
    (
        "Real components in two themes at once",
        "Not possible without scoping the process-wide theme store; the Theme comparison page compares tokens side by side and shows real components one theme after the other.",
    ),
    (
        "Syntax highlighting in code",
        "Code is shown in the monospace role and copied with a button; no highlighter is linked.",
    ),
    (
        "Collapsible sidebar groups and a docked inspector",
        "The sidebar scrolls instead, and each page carries its own token inspector below its examples.",
    ),
];

impl Render for OverviewPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let config = odk::ui::foundation_config(cx);
        let mut coverage = v_flex().gap(Space::Sm);
        for category in Category::ALL {
            let pages: Vec<Page> = Page::ALL
                .into_iter()
                .filter(|p| p.meta().category == category && !p.meta().components.is_empty())
                .collect();
            if pages.is_empty() {
                continue;
            }
            coverage = coverage.child(group(category.title()));
            for page in pages {
                let gallery = self.gallery.clone();
                coverage = coverage.child(
                    h_flex()
                        .flex_wrap()
                        .gap(Space::Md)
                        .child(
                            Button::new(("overview-open", page as usize))
                                .label(page.title())
                                .variant(ButtonVariant::Link)
                                .on_click(move |_, _, cx| {
                                    let _ = gallery.update(cx, |g, cx| g.show(page, cx));
                                }),
                        )
                        .child(Text::new(page.meta().components.join(", ")).muted()),
                );
            }
        }
        let mut missing = v_flex().gap(Space::Sm);
        for (what, why) in NOT_BUILT {
            missing = missing.child(kv(what, why));
        }
        page(
            Page::Overview,
            v_flex()
                .gap(Space::Lg)
                .child(Text::new("Every page uses the real ODK components in the theme Omarchy has selected. Hover, press and Tab through them; they respond as they would in an application. Use the toolbar to preview another installed theme without changing Omarchy's, and to change density, corners and motion for the whole gallery."))
                .child(section("How to use it"))
                .child(
                    v_flex()
                        .gap(Space::Xs)
                        .child(kv("Find a page", "Type in the sidebar's search: a component, a category, a token such as roles.warning.muted, or a word such as “focus”. Tab to the results, then Up and Down move through them and Enter opens one."))
                        .child(kv("Preview a theme", "Choose one in the toolbar. “System theme” returns to Omarchy's. Nothing is written."))
                        .child(kv("Check a state", "Density, corners and motion apply to every page at once; what you typed or chose on a page is kept."))
                        .child(kv("Inspect a value", "Each component page ends with its tokens and where each colour came from; the Colour resolver page says why a colour was chosen."))
                        .child(kv("Try the keyboard", "Tab, arrow keys, Enter, Space and Escape. The Accessibility page logs what they do.")),
                )
                .child(section("Now"))
                .child(
                    v_flex()
                        .gap(Space::Xs)
                        .child(kv("Theme", themes::system_name(cx)))
                        .child(kv("Density", config.density.name()))
                        .child(kv("Corners", format!("{:?} ({} px)", config.corner_radius, config.corner_radius.px()))),
                )
                .child(section("Coverage"))
                .child(coverage)
                .child(section("Not built"))
                .child(missing),
        )
    }
}
