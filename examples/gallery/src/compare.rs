//! The Theme comparison page. Two installed themes are loaded as separate snapshots, each
//! resolved by the same resolver with the same options, and compared as tokens side by side:
//! the surfaces, text, borders and roles, and the component tokens of every button variant
//! built for each. Real components, which draw the one theme the process publishes, are shown
//! one theme after the other: GPUI components read a process-wide store, so two themes cannot
//! be drawn by real components at once without scoping that store (`docs/gallery.md`,
//! "Limits"). Nothing here touches the system's theme.

use std::sync::Arc;

use odk::gpui::{Div, px, rgba};
use odk::prelude::*;
use odk::theme::component::{ButtonLayout, ComponentTokens, States};
use odk::theme::semantic::{Role, State};
use odk::theme::{Color, Mode};

use crate::buttons::choice;
use crate::present::{edge, flag, group, page, section, swatch, wrap};
use crate::registry::Page;
use crate::themes::{Loaded, Themes, pretty};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    A,
    B,
}

pub struct ComparePage {
    themes: Entity<Themes>,
    a: Option<Arc<Loaded>>,
    b: Option<Arc<Loaded>>,
    error: Option<String>,
    differences_only: bool,
    group: Group,
}

impl ComparePage {
    pub fn new(themes: Entity<Themes>) -> Self {
        Self {
            themes,
            a: None,
            b: None,
            error: None,
            differences_only: false,
            group: Group::Neutral,
        }
    }

    /// Loads the two themes the page starts on: the system's and another. Done when the page is
    /// shown, never while it is drawn.
    pub fn ensure_loaded(&mut self, cx: &mut Context<Self>) {
        if self.a.is_some() || self.b.is_some() {
            return;
        }
        let names = self.themes.read(cx).names().to_vec();
        let system = odk::ui::followed_theme(cx)
            .name
            .clone()
            .filter(|n| names.contains(n))
            .or_else(|| names.first().cloned());
        let other = names.iter().find(|n| Some(*n) != system.as_ref()).cloned();
        if let Some(name) = system {
            self.pick(Side::A, &name, cx);
        }
        if let Some(name) = other {
            self.pick(Side::B, &name, cx);
        }
    }

    /// Loads `name` as one side of the comparison.
    pub fn pick(&mut self, side: Side, name: &str, cx: &mut Context<Self>) {
        let loaded = self.themes.update(cx, |themes, cx| themes.reload(name, cx));
        match loaded {
            Ok(loaded) => {
                match side {
                    Side::A => self.a = Some(loaded),
                    Side::B => self.b = Some(loaded),
                }
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
        cx.notify();
    }

    /// The two themes, if both are chosen. For the tests.
    pub fn sides(&self) -> (Option<Arc<Loaded>>, Option<Arc<Loaded>>) {
        (self.a.clone(), self.b.clone())
    }

    /// How many of the compared tokens differ between the two themes. For the tests.
    pub fn differences(&self) -> Option<usize> {
        let (a, b) = (self.a.as_ref()?, self.b.as_ref()?);
        Some(token_rows(a, b).iter().filter(|r| r.differs()).count())
    }

    fn show(&mut self, side: Side, cx: &mut Context<Self>) {
        let loaded = match side {
            Side::A => self.a.clone(),
            Side::B => self.b.clone(),
        };
        if let Some(loaded) = loaded {
            let name = loaded.name.clone();
            self.themes.update(cx, |themes, cx| {
                themes.preview(&name, cx);
            });
        }
    }
}

/// Which tokens of the comparison are listed: the neutral surfaces, text and borders, or one
/// role's colours. The count of differences is always over all of them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Neutral,
    Role(Role),
}

/// One token in both themes.
pub struct TokenRow {
    pub group: Group,
    pub name: String,
    pub a: Color,
    pub b: Color,
}

impl TokenRow {
    pub fn differs(&self) -> bool {
        !self.a.same_rgba8(self.b)
    }
}

/// The tokens compared: surfaces, text, borders, and each role's three colours at each emphasis
/// in its default state, all read from the two snapshots' semantic layers.
pub fn token_rows(a: &Loaded, b: &Loaded) -> Vec<TokenRow> {
    let (x, y) = (&a.snapshot.semantic, &b.snapshot.semantic);
    let mut rows = Vec::new();
    for t in crate::colors::SURFACES {
        rows.push(TokenRow {
            group: Group::Neutral,
            name: format!("surface.{}", t.name()),
            a: x.surface.get(t),
            b: y.surface.get(t),
        });
    }
    for t in crate::colors::TEXTS {
        rows.push(TokenRow {
            group: Group::Neutral,
            name: format!("text.{}", t.name()),
            a: x.text.get(t),
            b: y.text.get(t),
        });
    }
    for t in crate::colors::BORDERS {
        rows.push(TokenRow {
            group: Group::Neutral,
            name: format!("border.{}", t.name()),
            a: x.border.get(t),
            b: y.border.get(t),
        });
    }
    for role in Role::ALL {
        for emphasis in odk::theme::semantic::Emphasis::ALL {
            let (p, q) = (
                x.treatment(role, emphasis, State::Default),
                y.treatment(role, emphasis, State::Default),
            );
            for (part, c, d) in [
                ("background", p.background, q.background),
                ("foreground", p.foreground, q.foreground),
                ("border", p.border, q.border),
            ] {
                rows.push(TokenRow {
                    group: Group::Role(role),
                    name: format!("roles.{role}.{}.default.{part}", emphasis.name()),
                    a: c,
                    b: d,
                });
            }
        }
        rows.push(TokenRow {
            group: Group::Role(role),
            name: format!("roles.{role}.focus"),
            a: x.role(role).focus,
            b: y.role(role).focus,
        });
    }
    rows
}

fn mode(l: &Loaded) -> &'static str {
    match l.snapshot.palette.mode {
        Mode::Light => "light",
        _ => "dark",
    }
}

/// A button variant's colours as one theme's component tokens give them: a token sample, not a
/// button.
fn chip(l: &Loaded, variant: ButtonVariant) -> Div {
    let components = ComponentTokens::new(
        odk::ui::foundation(),
        l.snapshot.semantic.clone(),
        l.snapshot.generation,
    );
    let style = components.button(variant, Density::Default, ButtonLayout::Text, States::NONE);
    let t = style.current;
    div()
        .px(Space::Sm)
        .py(Space::Xs)
        .bg(rgba(t.background.to_rgba_u32()))
        .text_color(rgba(t.foreground.to_rgba_u32()))
        .border_1()
        .border_color(rgba(t.border.to_rgba_u32()))
        .child(variant.name())
}

impl Render for ComparePage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let names = self.themes.read(cx).names().to_vec();
        let current = self.themes.read(cx).choice().map(str::to_owned);
        let picker = |side: Side,
                      id: &'static str,
                      label: &'static str,
                      loaded: &Option<Arc<Loaded>>,
                      cx: &mut Context<Self>| {
            let mut select = Select::new(id, label).placeholder("Choose a theme");
            for name in &names {
                select = select.item(name.clone(), pretty(name));
            }
            if let Some(l) = loaded {
                select = select.value(l.name.clone());
            }
            Row::new()
                .gap(Space::Md)
                .child(Text::new(label).muted())
                .child(
                    div()
                        .w(px(crate::layout::THEME_SELECT_WIDTH))
                        .child(select.on_change(
                            cx.listener(move |this, value: &str, _, cx| this.pick(side, value, cx)),
                        )),
                )
        };
        let pickers = wrap()
            .gap(Space::Xl)
            .child(picker(Side::A, "compare-a", "Theme A", &self.a.clone(), cx))
            .child(picker(Side::B, "compare-b", "Theme B", &self.b.clone(), cx));

        let mut body = Stack::new().gap(Space::Lg).child(pickers);
        if let Some(error) = &self.error {
            body = body.child(
                Alert::new("compare-error", "A theme could not be loaded")
                    .variant(Tone::Danger)
                    .description(error.clone()),
            );
        }
        if let (Some(a), Some(b)) = (self.a.clone(), self.b.clone()) {
            let rows = token_rows(&a, &b);
            let differing = rows.iter().filter(|r| r.differs()).count();
            body = body
                .child(Text::new(format!(
                    "A is {} ({}), B is {} ({}). Both are resolved the same way, with the same density and corners; {differing} of {} compared tokens differ.",
                    pretty(&a.name),
                    mode(&a),
                    pretty(&b.name),
                    mode(&b),
                    rows.len()
                )))
                .child(section("Component tokens, by theme"))
                .child(Text::new("Every button variant at rest, from each theme's own component tokens. These are token samples, not buttons; the real ones are below.").muted());
            let mut chips = Stack::new().gap(Space::Sm);
            for (label, l) in [("A", &a), ("B", &b)] {
                chips = chips.child(
                    Row::new()
                        .flex_wrap()
                        .gap(Space::Sm)
                        .child(div().w(px(24.0)).child(Text::new(label).muted()))
                        .children(ButtonVariant::ALL.into_iter().map(|v| chip(l, v))),
                );
            }
            body = body.child(chips).child(section("Tokens"));
            let mut table = Stack::new().gap(Space::Xs).child(
                Checkbox::new("compare-only", "Show only differences")
                    .checked(self.differences_only)
                    .on_change(flag(cx, |this: &mut Self, on, cx| {
                        this.differences_only = on;
                        cx.notify();
                    })),
            );
            let mut groups = vec![(Group::Neutral, "Surfaces, text, borders")];
            groups.extend(Role::ALL.map(|r| {
                (
                    Group::Role(r),
                    match r {
                        Role::Accent => "Accent",
                        Role::Neutral => "Neutral",
                        Role::Danger => "Danger",
                        Role::Success => "Success",
                        Role::Warning => "Warning",
                        Role::Info => "Info",
                    },
                )
            }));
            table = table.child(choice(
                "compare-group",
                "Show",
                groups,
                self.group,
                |this: &mut Self, g, _| this.group = g,
                cx,
            ));
            for row in rows
                .iter()
                .filter(|r| r.group == self.group && (!self.differences_only || r.differs()))
            {
                table = table.child(
                    Row::new()
                        .flex_wrap()
                        .gap(Space::Md)
                        .child(
                            div()
                                .w(px(crate::layout::LABEL_WIDTH * 1.6))
                                .child(Text::new(row.name.clone()).muted()),
                        )
                        .child(wrap().child(swatch(row.a)).child(Text::new(row.a.to_hex())))
                        .child(wrap().child(swatch(row.b)).child(Text::new(row.b.to_hex())))
                        .child(Text::new(if row.differs() {
                            "≠ differs"
                        } else {
                            "= same"
                        })),
                );
            }
            body = body.child(table);
        } else {
            body = body.child(Text::new("Choose two installed themes to compare."));
        }

        let showing = current
            .as_deref()
            .map_or_else(|| "the system's theme".to_owned(), pretty);
        body = body
            .child(section("Real components, one theme after the other"))
            .child(Text::new(format!("Drawing {showing}. Choosing A or B draws that theme in the whole gallery without changing Omarchy's; “System theme” returns.")).muted())
            .child(
                wrap()
                    .child(Button::new("compare-show-a").label("Show A").variant(ButtonVariant::Outline).on_click(cx.listener(|this, _, _, cx| this.show(Side::A, cx))))
                    .child(Button::new("compare-show-b").label("Show B").variant(ButtonVariant::Outline).on_click(cx.listener(|this, _, _, cx| this.show(Side::B, cx))))
                    .child(Button::new("compare-system").label("System theme").variant(ButtonVariant::Ghost).on_click(cx.listener(|this, _, _, cx| {
                        this.themes.update(cx, |themes, cx| themes.follow(cx));
                        cx.notify();
                    }))),
            )
            .child(group("Components"))
            .child(
                wrap()
                    .children(ButtonVariant::ALL.into_iter().enumerate().map(|(n, v)| {
                        Button::new(("compare-real", n)).label(v.name()).variant(v)
                    }))
                    .child(Badge::new("Info").tone(Tone::Info).marked())
                    .child(Switch::new("compare-switch", "Switch").default_checked(true)),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(edge())
                    .pt(Space::Sm)
                    .child(Text::new("Limit: real components read one process-wide theme, so two themes are never drawn by real components at the same time. That would need the store scoped to a subtree (docs/spec/odk-source-palette.md, “Previewing a theme”).").muted()),
            );

        page(Page::Compare, body)
    }
}
