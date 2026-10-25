//! The Theme diagnostics page: what the theme manager knows about the theme drawn and the one
//! followed, its resolver's strategy and notes, and the component tokens' contrast. All of it is
//! kept from when the theme was loaded; this page reads it and parses nothing.

use odk::prelude::*;
use odk::theme::ThemeDiagnostics;

use crate::diagnostics::Rows;
use crate::present::{group, kv, page, section};
use crate::registry::Page;
use crate::themes::{Themes, pretty};

pub struct ThemePage {
    themes: Entity<Themes>,
    rows: Rows,
}

impl ThemePage {
    pub fn new(themes: Entity<Themes>) -> Self {
        Self {
            themes,
            rows: Rows::default(),
        }
    }
}

fn describe(d: &ThemeDiagnostics) -> Div {
    let missing = if d.missing.is_empty() {
        "none".to_owned()
    } else {
        d.missing
            .iter()
            .map(|k| k.name())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut lines = v_flex()
        .gap(Space::Xs)
        .child(kv(
            "Name",
            d.name
                .clone()
                .map_or_else(|| "unnamed".to_owned(), |n| pretty(&n)),
        ))
        .child(kv("Mode", d.mode.name()))
        .child(kv(
            "Palette source",
            d.colors.as_ref().map_or_else(
                || "none (the emergency palette)".to_owned(),
                |p| p.display().to_string(),
            ),
        ))
        .child(kv("Generation", d.generation.to_string()))
        .child(kv("Colours defined", format!("{} of 27", d.colours)))
        .child(kv("Optional keys left out", missing))
        .child(kv("Resolver strategy", d.strategy.name()))
        .child(kv(
            "Monochrome",
            if d.monochrome {
                "yes: the resolver adds no hue"
            } else {
                "no"
            },
        ))
        .child(kv("Colours adjusted for contrast", d.adjusted.to_string()))
        .child(kv("Text size", format!("{} px", d.text.base_size())))
        .child(kv(
            "User's request for reduced motion",
            if d.reduced_motion {
                match d.compositor_animations {
                    Some(false) => "yes (the compositor's animations are off)",
                    _ => "yes (oma's copy of the compositor's setting)",
                }
            } else {
                "no"
            },
        ))
        .child(kv(
            "Fallback",
            match d.fallback {
                None => "none: the theme loaded".to_owned(),
                Some(f) => format!("{f:?}"),
            },
        ));
    if let Some(failure) = &d.failure {
        lines = lines.child(kv("Latest failure", failure.error.to_string()));
    }
    lines
}

use odk::gpui::Div;

impl Render for ThemePage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = odk::ui::theme_snapshot();
        let system = odk::ui::theme_diagnostics(cx);
        let preview = {
            let themes = self.themes.read(cx);
            themes.choice().and_then(|name| themes.loaded(name))
        };
        let mut body = v_flex().gap(Space::Lg).child(section("System theme"));
        body = body.child(match &system {
            Some(d) => describe(d),
            None => v_flex().child(Text::new(
                "This gallery is not following Omarchy's theme (no diagnostics were kept).",
            )),
        });
        body = body.child(section("Preview"));
        body = body.child(match &preview {
            Some(loaded) => v_flex()
                .gap(Space::Sm)
                .child(Text::new(format!("{} is drawn in place of the system's. Omarchy's selection and files are untouched.", pretty(&loaded.name))))
                .child(describe(&loaded.diagnostics)),
            None => v_flex().child(Text::new("No theme is being previewed: the gallery draws the system's.")),
        });

        let components = odk::ui::components();
        body = body.child(section("Component tokens")).child(
            v_flex()
                .gap(Space::Xs)
                .child(kv("Generation", components.generation().to_string()))
                .child(kv(
                    "Matches the theme drawn",
                    if components.generation() == theme.generation {
                        "yes"
                    } else {
                        "no"
                    },
                ))
                .child(kv("Density", components.density(None).name())),
        );

        let rows = self.rows.get(&components);
        let mut issues = v_flex().gap(Space::Xs);
        let mut count = 0;
        for row in rows.iter() {
            for result in row.contrast.iter().filter(|r| !r.passes()) {
                count += 1;
                if count <= 20 {
                    issues = issues.child(Text::new(format!(
                        "{} {} {}: {} {:.2}:1, needs {}:1 against surface.{}",
                        row.component,
                        row.variant,
                        row.state,
                        result.measure.name(),
                        result.found,
                        result.required,
                        result.against
                    )));
                }
            }
        }
        body = body
            .child(group("Contrast issues"))
            .child(Text::new(format!(
                "{} style{} checked, {count} below the minimum. A theme whose component colours fail a minimum is refused when it loads (ADR 0041), so this is expected to be 0.",
                rows.len(),
                if rows.len() == 1 { "" } else { "s" }
            )).muted())
            .child(issues);

        let mut notes = v_flex().gap(Space::Xs);
        for note in &theme.resolution.notes {
            notes = notes.child(Text::new(format!("⚠ {note}")));
        }
        if theme.resolution.notes.is_empty() {
            notes = notes.child(Text::new("No warnings."));
        }
        body = body.child(group("Resolver warnings")).child(notes);

        page(Page::Diagnostics, body)
    }
}
