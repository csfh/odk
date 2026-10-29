//! The Navigation page: tabs, segmented controls, a sidebar's navigation and breadcrumbs, at
//! each density and in every state, and the diagnostics of their
//! component tokens. Everything here is a real control in the active Omarchy theme; Tab to see
//! its ring, and hover and press it with the pointer.

use odk::gpui::{Div, px};
use odk::prelude::*;

use crate::buttons::{caption, heights, row};
use crate::content::Content;
use crate::{Settings, diagnostics};

/// The Navigation page: its own state, and the Content switch it follows.
pub struct NavigationPage {
    worst_case: bool,
    alignment: Vec<SharedString>,
    /// The mail tabs' selection, the chosen period, and the sidebar's page.
    mail_tab: SharedString,
    period: Vec<SharedString>,
    nav_page: SharedString,
    inspector: diagnostics::Inspector,
    diagnostics: diagnostics::Rows,
}

impl NavigationPage {
    pub fn new(settings: &Entity<Settings>, cx: &mut Context<Self>) -> Self {
        cx.observe(settings, |this, settings, cx| {
            this.worst_case = settings.read(cx).worst_case;
            cx.notify();
        })
        .detach();
        Self {
            worst_case: settings.read(cx).worst_case,
            alignment: vec!["left".into()],
            mail_tab: "inbox".into(),
            period: vec!["week".into()],
            nav_page: "general".into(),
            inspector: diagnostics::Inspector::new("nav item"),
            diagnostics: diagnostics::Rows::default(),
        }
    }
}

impl Render for NavigationPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = Content::get(self.worst_case);
        crate::page(crate::Page::Navigation, section(self, &c, cx))
    }
}

/// The components the diagnostics panel offers.
const COMPONENTS: [&str; 4] = ["tab", "segment", "nav item", "nav link"];

fn group(title: &'static str) -> impl IntoElement {
    Heading::new(title).level(3)
}

/// The section.
pub fn section(g: &mut NavigationPage, c: &Content, cx: &mut Context<NavigationPage>) -> Div {
    // Uncontrolled controls keep their own state, so the Content switch gives them new ids.
    let k = usize::from(g.worst_case);
    v_flex()
        .flex_shrink_0()
        .gap(Space::Lg)
        .child(caption(
            "Tab reaches each control and shows its ring; the arrow keys move inside tabs and segmented controls. Hover and press with the pointer.",
        ))
        .child(group("Tabs"))
        .child(tabs(g, c, k, cx))
        .child(group("Segmented controls"))
        .child(segments(g, c, k, cx))
        .child(group("Sidebar navigation"))
        .child(sidebar(g, c, k, cx))
        .child(group("Breadcrumbs"))
        .child(breadcrumbs(c))
        .child(group("Densities"))
        .child(caption(format!(
            "Tabs, segmented controls and sidebar rows are a control's height: {}.",
            heights()
        )))
        .children(Density::ALL.iter().enumerate().map(|(n, &d)| density_row(n, d, c)))
        .child(group("Diagnostics"))
        .child(caption(
            "The tokens one navigation component draws with, where each came from, and its contrast.",
        ))
        .child(diagnostics::panel(
            g,
            "nav",
            &COMPONENTS,
            |g| &mut g.diagnostics,
            |g| &mut g.inspector,
            cx,
        ))
}

fn tabs(g: &mut NavigationPage, c: &Content, k: usize, cx: &mut Context<NavigationPage>) -> Div {
    let mut text = Tabs::new(("tabs", k), "Settings sections");
    for (n, label) in c.tabs.iter().enumerate() {
        let body = format!("The {label} section.");
        text = text.tab(
            SharedString::from(format!("tab-{n}")),
            *label,
            move |_, _| Text::new(body),
        );
    }
    let text = text.disable("tab-2");

    let mail = Tabs::new(("mail", k), "Mailboxes")
        .manual_activation(true)
        .value(g.mail_tab.clone())
        .tab("inbox", "Inbox", |_, _| Text::new("12 unread messages."))
        .icon("inbox", Icon::Inbox)
        .counter("inbox", "12")
        .tab("sent", "Sent", |_, _| Text::new("Nothing waiting to send."))
        .icon("sent", Icon::Send)
        .tab("drafts", "Drafts", |_, _| Text::new("3 drafts."))
        .icon("drafts", Icon::Draft)
        .counter("drafts", "3")
        .tab("spam", "Spam", |_, _| Text::new("Spam is off."))
        .icon("spam", Icon::Spam)
        .disable("spam")
        .on_change(cx.listener(|this, value: &SharedString, _, cx| {
            this.mail_tab = value.clone();
            cx.notify();
        }));
    v_flex()
        .gap(Space::Lg)
        .child(caption(
            "Automatic: the arrows select as they move. The third tab is disabled.",
        ))
        .child(text)
        .child(caption(
            "Manual, with icons and counters: the arrows move focus, Enter or Space selects. Spam is disabled.",
        ))
        .child(mail)
}

fn segments(
    g: &mut NavigationPage,
    c: &Content,
    k: usize,
    cx: &mut Context<NavigationPage>,
) -> Div {
    let [day, week, month] = c.periods;
    let [left, center, right, justify] = c.alignment;
    v_flex()
        .gap(Space::Md)
        .child(
            row()
                .child(
                    ToggleGroup::new("period", "Period")
                        .item("day", day)
                        .item("week", week)
                        .item("month", month)
                        .value(g.period.clone())
                        .on_change(cx.listener(|this, value: &[SharedString], _, cx| {
                            // Choosing the chosen one again keeps it.
                            if !value.is_empty() {
                                this.period = value.to_vec();
                            }
                            cx.notify();
                        })),
                )
                .child(
                    ToggleGroup::new(("view", k), "View")
                        .icon_item("list", Icon::List, "List")
                        .icon_item("grid", Icon::Grid, "Grid")
                        .icon_item("columns", Icon::Columns, "Columns")
                        .default_value(["grid"]),
                )
                .child(
                    ToggleGroup::new(("layout", k), "Layout")
                        .item("list", "List")
                        .icon("list", Icon::List)
                        .item("grid", "Grid")
                        .icon("grid", Icon::Grid)
                        .default_value(["list"]),
                ),
        )
        .child(caption(format!(
            "Period: {}. A text, an icon-only and an icon-and-text control.",
            g.period.join(", ")
        )))
        .child(
            row()
                .child(
                    ToggleGroup::new("align", "Alignment")
                        .item("left", left)
                        .item("center", center)
                        .item("right", right)
                        .item("justify", justify)
                        .disable("justify")
                        .value(g.alignment.clone())
                        .on_change(cx.listener(|this, value: &[SharedString], _, cx| {
                            this.alignment = value.to_vec();
                            cx.notify();
                        })),
                )
                .child(
                    ToggleGroup::new(("style", k), "Text style")
                        .multiple(true)
                        .default_value(["bold"])
                        .icon_item("bold", Icon::Bold, "Bold")
                        .icon_item("italic", Icon::Italic, "Italic")
                        .icon_item("under", Icon::Underline, "Underline"),
                )
                .child(
                    ToggleGroup::new(("locked", k), "Locked")
                        .item("on", "On")
                        .item("off", "Off")
                        .default_value(["on"])
                        .disabled(true),
                ),
        )
        .child(caption(
            "Alignment has a disabled option; Text style takes several; the last is disabled with its choice kept.",
        ))
}

fn sidebar(g: &mut NavigationPage, c: &Content, k: usize, cx: &mut Context<NavigationPage>) -> Div {
    let ([workspace, account], [general, members, billing, profile, notifications]) = c.nav;
    let current = g.nav_page.clone();
    h_flex()
        .items_start()
        .gap(Space::Lg)
        .child(
            div()
                .w(px(240.0))
                .flex_shrink_0()
                .child(
                    NavList::new(("sidebar", k), "Settings")
                        .group(workspace)
                        .item("general", general)
                        .icon("general", Icon::Settings)
                        .item("members", members)
                        .icon("members", Icon::Team)
                        .counter("members", "4")
                        .item("billing", billing)
                        .icon("billing", Icon::BankCard)
                        .disable("billing")
                        .group(account)
                        .item("profile", profile)
                        .icon("profile", Icon::User)
                        .item("notifications", notifications)
                        .icon("notifications", Icon::Notification)
                        .counter("notifications", "12")
                        .value(current.clone())
                        .on_change(cx.listener(|this, value: &SharedString, _, cx| {
                            this.nav_page = value.clone();
                            cx.notify();
                        })),
                ),
        )
        .child(Separator::vertical())
        .child(
            v_flex()
                .gap(Space::Sm)
                .min_w_0()
                .child(Heading::new(format!("Page: {current}")).level(3))
                .child(caption(
                    "One tab stop: Up and Down move, Enter, Space or a click opens. Moving focus opens nothing. Billing is disabled.",
                )),
        )
}

fn breadcrumbs(c: &Content) -> Div {
    let mut short_path = Breadcrumb::new("short");
    for (n, label) in c.short_path.iter().enumerate() {
        short_path = if n + 1 == c.short_path.len() {
            short_path.current(*label)
        } else {
            short_path.item(*label, |_, _| {})
        };
    }
    let mut long_path = Breadcrumb::new("long").max_items(3);
    for (n, label) in c.long_path.iter().enumerate() {
        long_path = if n + 1 == c.long_path.len() {
            long_path.current(*label)
        } else {
            long_path.item(*label, |_, _| {})
        };
    }
    v_flex()
        .gap(Space::Sm)
        .child(short_path)
        .child(long_path.separator(Icon::ArrowRight))
        .child(caption(
            "The second folds its middle into a menu and has its own separator.",
        ))
}

fn density_row(n: usize, d: Density, c: &Content) -> Div {
    let [day, week, month] = c.periods;
    row()
        .items_center()
        .child(Button::new(("nd-button", n)).label("Save").density(d))
        .child(
            ToggleGroup::new(("nd-period", n), "Period")
                .item("day", day)
                .item("week", week)
                .item("month", month)
                .default_value(["week"])
                .density(d),
        )
        .child(
            div().w(px(220.0)).child(
                Tabs::new(("nd-tabs", n), "Sections")
                    .density(d)
                    .tab("a", "General", |_, _| div())
                    .tab("b", "Display", |_, _| div()),
            ),
        )
        .child(
            div().w(px(160.0)).child(
                NavList::new(("nd-nav", n), "Settings")
                    .density(d)
                    .item("a", "General")
                    .icon("a", Icon::Settings)
                    .default_value("a"),
            ),
        )
        .child(caption(format!("{} {} px", d.name(), d.metrics().height)))
}
