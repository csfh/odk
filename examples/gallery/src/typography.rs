//! The Typography page: every type role as a live sample with the foundation's values for it,
//! and text on the surfaces it is read on, wrapped, truncated and technical. Sizes are read from
//! the foundation inside `render`, so the user's text size shows.

use odk::gpui::px;
use odk::prelude::*;
use odk::theme::foundation::{FontFamily, FontWeight, TypeRole};

use crate::present::{group, kv, page, section};
use crate::registry::Page;

pub struct TypographyPage;

fn weight(w: FontWeight) -> &'static str {
    match w {
        FontWeight::Regular => "regular (400)",
        FontWeight::Medium => "medium (500)",
        FontWeight::Semibold => "semibold (600)",
    }
}

impl Render for TypographyPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let f = odk::ui::foundation();
        let theme = odk::ui::theme_snapshot();
        let t = &f.typography;

        let mut roles = v_flex().gap(Space::Lg);
        for role in TypeRole::ALL {
            let s = t.style(role);
            let family = match (s.family, t.family(s.family)) {
                (_, Some(name)) => format!("{name} (the application's)"),
                (FontFamily::Ui, None) => "Omarchy's font".to_owned(),
                (FontFamily::Mono, None) => "Omarchy's monospace font".to_owned(),
            };
            roles = roles.child(
                v_flex()
                    .gap(Space::Xs)
                    .child(Text::new("The quick brown fox jumps over the lazy dog").role(role))
                    .child(
                        Text::new(format!(
                            "type.{} · {family} · {} px / {} px · {} · letter spacing {} px",
                            role.name(),
                            s.size,
                            s.line_height,
                            weight(s.weight),
                            s.letter_spacing
                        ))
                        .muted(),
                    ),
            );
        }

        let mut surfaces = v_flex().gap(Space::Sm);
        for (name, fill, ink) in [
            (
                "surface.canvas",
                theme.semantic.surface.canvas,
                theme.semantic.text.primary,
            ),
            (
                "surface.recessed",
                theme.semantic.surface.recessed,
                theme.semantic.text.primary,
            ),
            (
                "surface.raised",
                theme.semantic.surface.raised,
                theme.semantic.text.secondary,
            ),
            (
                "surface.selection",
                theme.semantic.surface.selection,
                theme.semantic.text.on_selection,
            ),
        ] {
            surfaces = surfaces.child(
                div()
                    .p(Space::Md)
                    .bg(odk::gpui::rgba(fill.to_rgba_u32()))
                    .text_color(odk::gpui::rgba(ink.to_rgba_u32()))
                    .child(Text::new(format!("Body text on {name}")).role(TypeRole::Body))
                    .child(Text::new("A caption beneath it").role(TypeRole::Caption)),
            );
        }

        page(
            Page::Typography,
            v_flex()
                .gap(Space::Lg)
                .child(section("Roles"))
                .child(Text::new(format!(
                    "Nine roles. Text follows the user's text size ({} px base) and Omarchy's font; an application may name its own families. Letter spacing is zero for every role because GPUI 0.2.2 cannot apply any.",
                    f.text_scale.base_size()
                )).muted())
                .child(roles)
                .child(section("On surfaces"))
                .child(surfaces)
                .child(section("Long text, wrapping and truncation"))
                .child(
                    v_flex()
                        .gap(Space::Md)
                        .child(group("Wraps at its container"))
                        .child(div().max_w(px(360.0)).child(Text::new("Long lines wrap at the width they are given and keep the role's line height, so a paragraph in a narrow panel is as legible as one in a wide window. Nothing here is clipped.")))
                        .child(group("Truncates to one line"))
                        .child(
                            div().w(px(220.0)).child(
                                Text::new("A file name that is much too long for its column.tar.gz")
                                    .decorate(EndEllipsis),
                            ),
                        )
                        .child(group("Shortens the middle"))
                        .child(
                            div().w(px(220.0)).child(
                                Text::new(
                                    "https://example.com/users/ada/projects/odk/docs/adr/0058-truncation.md",
                                )
                                .decorate(MiddleEllipsis),
                            ),
                        )
                        .child(group("Technical values"))
                        .child(Text::new("0x7fff_ffff · #1e1e2e · 12.5px/18px · ~/.config/omarchy/shell.toml").mono())
                        .child(
                            div().w(px(220.0)).child(
                                Text::new("/usr/lib/systemd/systemd --user --deserialize=42")
                                    .mono()
                                    .muted()
                                    .decorate(EndEllipsis),
                            ),
                        )
                        .child(kv("Monospace family", t.family(FontFamily::Mono).unwrap_or("Omarchy's monospace font").to_owned())),
                ),
        )
    }
}
