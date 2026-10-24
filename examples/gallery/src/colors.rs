//! The Colors page: the theme's source palette exactly as the theme wrote it, and the semantic
//! colours the Adaptive Colour Resolver made from it. Everything is read from the published
//! snapshot, which holds the resolver's output; nothing is derived here, so what a swatch shows
//! is what a component draws.

use odk::gpui::Div;
use odk::prelude::*;
use odk::theme::Color;
use odk::theme::semantic::{
    BorderToken, Emphasis, Role, SemanticColours, State, SurfaceToken, TextToken,
};
use odk::theme::{PaletteKey, drawn_contrast};

use crate::present::{colour_kv, group, kv, page, section, swatch, wrap};
use crate::registry::Page;

pub const SURFACES: [SurfaceToken; 9] = [
    SurfaceToken::Canvas,
    SurfaceToken::Recessed,
    SurfaceToken::Sunken,
    SurfaceToken::Raised,
    SurfaceToken::Overlay,
    SurfaceToken::Selection,
    SurfaceToken::Hover,
    SurfaceToken::Pressed,
    SurfaceToken::Scrim,
];

pub const TEXTS: [TextToken; 9] = [
    TextToken::Primary,
    TextToken::Secondary,
    TextToken::Tertiary,
    TextToken::Disabled,
    TextToken::Inverse,
    TextToken::Link,
    TextToken::Placeholder,
    TextToken::OnAccent,
    TextToken::OnSelection,
];

pub const BORDERS: [BorderToken; 7] = [
    BorderToken::Subtle,
    BorderToken::Default,
    BorderToken::Strong,
    BorderToken::Hover,
    BorderToken::Focus,
    BorderToken::Disabled,
    BorderToken::Danger,
];

/// The palette's keys in the groups the page shows them in. Every key is in exactly one.
pub const GROUPS: [(&str, &[PaletteKey]); 5] = [
    (
        "Backgrounds",
        &[
            PaletteKey::Background,
            PaletteKey::DarkBackground,
            PaletteKey::DarkerBackground,
            PaletteKey::LighterBackground,
        ],
    ),
    (
        "Foregrounds",
        &[
            PaletteKey::Foreground,
            PaletteKey::DarkForeground,
            PaletteKey::LightForeground,
            PaletteKey::BrightForeground,
        ],
    ),
    (
        "Accent, selection and muted",
        &[
            PaletteKey::Accent,
            PaletteKey::Selection,
            PaletteKey::SelectionBackground,
            PaletteKey::SelectionForeground,
            PaletteKey::Muted,
        ],
    ),
    (
        "Regular colours",
        &[
            PaletteKey::Red,
            PaletteKey::Orange,
            PaletteKey::Yellow,
            PaletteKey::Green,
            PaletteKey::Cyan,
            PaletteKey::Blue,
            PaletteKey::Magenta,
            PaletteKey::Brown,
        ],
    ),
    (
        "Bright colours",
        &[
            PaletteKey::BrightRed,
            PaletteKey::BrightYellow,
            PaletteKey::BrightGreen,
            PaletteKey::BrightCyan,
            PaletteKey::BrightBlue,
            PaletteKey::BrightMagenta,
        ],
    ),
];

pub struct ColorsPage;

impl Render for ColorsPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let theme = odk::ui::theme_snapshot();
        let palette = &theme.palette;
        let semantic = &theme.semantic;

        let mut source = v_flex().gap(Space::Md);
        source = source.child(Text::new(format!(
            "{} · {} colours defined by the theme, {} optional ones left out. A swatch here is the theme's own colour, as written in `colors.toml`.",
            theme
                .name
                .as_deref()
                .map_or_else(|| "unnamed".to_owned(), crate::themes::pretty),
            palette.colours().count(),
            palette.missing().count()
        )).muted());
        for (title, keys) in GROUPS {
            let mut rows = v_flex().gap(Space::Xs);
            for key in keys {
                rows = rows.child(match palette.get(*key) {
                    Some(c) => colour_kv(
                        key.name(),
                        c,
                        if key.is_required() {
                            "source (required)"
                        } else {
                            "source"
                        },
                    ),
                    None => kv(key.name(), "not defined by this theme (optional)"),
                });
            }
            source = source.child(group(title)).child(rows);
        }

        let mut surfaces = v_flex().gap(Space::Xs);
        for token in SURFACES {
            surfaces = surfaces.child(colour_kv(
                format!("surface.{}", token.name()),
                semantic.surface.get(token),
                "derived",
            ));
        }
        for token in TEXTS {
            surfaces = surfaces.child(colour_kv(
                format!("text.{}", token.name()),
                semantic.text.get(token),
                "derived",
            ));
        }
        for token in BORDERS {
            surfaces = surfaces.child(colour_kv(
                format!("border.{}", token.name()),
                semantic.border.get(token),
                "derived",
            ));
        }

        let mut roles = v_flex().gap(Space::Md);
        roles = roles.child(
            Text::new(
                "Each role at each emphasis, in its default state: the sample is the treatment's background and foreground over the canvas, with the border it is drawn with. Contrast is the foreground against the background as drawn over the canvas: text needs 4.5:1.",
            )
            .muted(),
        );
        for role in Role::ALL {
            let mut block = v_flex().gap(Space::Xs).child(group(match role {
                Role::Accent => "Accent",
                Role::Neutral => "Neutral",
                Role::Danger => "Danger",
                Role::Success => "Success",
                Role::Warning => "Warning",
                Role::Info => "Info",
            }));
            for emphasis in Emphasis::ALL {
                block = block.child(treatment_row(role, emphasis));
            }
            block = block.child(colour_kv(
                format!("roles.{role}.focus"),
                semantic.role(role).focus,
                "focus indicator",
            ));
            roles = roles.child(block);
        }

        page(
            Page::Colors,
            v_flex()
                .gap(Space::Lg)
                .child(section("Source palette"))
                .child(source)
                .child(section("Surfaces, text and borders"))
                .child(Text::new("Derived by the resolver from the palette above; see the Colour resolver page for how.").muted())
                .child(surfaces)
                .child(section("Semantic roles"))
                .child(roles),
        )
    }
}

/// A role at an emphasis in its default state, as the page shows it: the resolver's three
/// colours, and the contrast of the foreground against the background as drawn over the canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreatmentRow {
    pub background: Color,
    pub foreground: Color,
    pub border: Color,
    pub ratio: f32,
}

impl TreatmentRow {
    pub fn passes(&self) -> bool {
        self.ratio >= odk::theme::TEXT_CONTRAST
    }
}

pub fn treatment_data(semantic: &SemanticColours, role: Role, emphasis: Emphasis) -> TreatmentRow {
    let t = semantic.treatment(role, emphasis, State::Default);
    TreatmentRow {
        background: t.background,
        foreground: t.foreground,
        border: t.border,
        ratio: drawn_contrast(t.foreground, t.background, semantic.surface.canvas),
    }
}

/// One line: a role at an emphasis, its sample, its three colours and its contrast.
fn treatment_row(role: Role, emphasis: Emphasis) -> Div {
    let theme = odk::ui::theme_snapshot();
    let row = treatment_data(&theme.semantic, role, emphasis);
    let (ratio, passes) = (row.ratio, row.passes());
    let name = format!("roles.{role}.{}", emphasis.name());
    h_flex()
        .flex_wrap()
        .gap(Space::Md)
        .child(
            div()
                .w(odk::gpui::px(crate::layout::LABEL_WIDTH))
                .flex_shrink_0()
                .child(Text::new(name).muted()),
        )
        .child(
            div()
                .px(Space::Md)
                .py(Space::Xs)
                .bg(odk::gpui::rgba(row.background.to_rgba_u32()))
                .text_color(odk::gpui::rgba(row.foreground.to_rgba_u32()))
                .border_1()
                .border_color(odk::gpui::rgba(row.border.to_rgba_u32()))
                .child("Aa sample"),
        )
        .child(
            wrap()
                .child(swatch(row.background))
                .child(Text::new(row.background.to_hex())),
        )
        .child(
            wrap()
                .child(swatch(row.foreground))
                .child(Text::new(row.foreground.to_hex())),
        )
        .child(
            wrap()
                .child(swatch(row.border))
                .child(Text::new(row.border.to_hex())),
        )
        .child(Text::new(format!(
            "{ratio:.2}:1 {}",
            if passes {
                "✓ text"
            } else {
                "✗ below 4.5:1"
            }
        )))
}
