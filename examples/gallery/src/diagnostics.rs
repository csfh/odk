//! The diagnostics panel the Button, Forms, Navigation and Feedback sections share: every style of
//! one component at a density, with its colours and where each came from, its geometry and its
//! contrast, and the theme they were resolved for. Read from the component tokens' diagnostics,
//! which are built on request, never while drawing, once for every panel, and kept until the
//! tokens are published again.

use std::rc::Rc;
use std::sync::Arc;

use odk::gpui::{Div, px};
use odk::prelude::*;
use odk::theme::component::{ComponentTokens, Shape, StyleDiagnostic};

use crate::buttons::{choice, colour_line};

/// What one panel shows.
pub struct Inspector {
    component: &'static str,
    density: Density,
}

impl Inspector {
    pub fn new(component: &'static str) -> Self {
        Self {
            component,
            density: Density::Default,
        }
    }
}

/// Every style's diagnostics, built once for the tokens they describe, for one page's panel.
#[derive(Default)]
pub struct Rows(Option<(Arc<ComponentTokens>, Rc<Vec<StyleDiagnostic>>)>);

impl Rows {
    pub(crate) fn get(&mut self, tokens: &Arc<ComponentTokens>) -> Rc<Vec<StyleDiagnostic>> {
        match &self.0 {
            Some((at, rows)) if Arc::ptr_eq(at, tokens) => rows.clone(),
            _ => {
                let options = odk::ui::theme_snapshot().resolution.options.clone();
                let rows = Rc::new(tokens.diagnostics(&options));
                self.0 = Some((tokens.clone(), rows.clone()));
                rows
            }
        }
    }
}

/// One style's line and its three colours with where each came from, as the panels show it.
pub(crate) fn row_lines(row: &StyleDiagnostic) -> Div {
    let shape = match row.shape {
        Shape::Rect { radius } => format!("radius {radius} px"),
        Shape::Pill => "a pill".into(),
        Shape::Circle => "a circle".into(),
    };
    let height = row
        .height
        .map_or_else(|| "its content's height".into(), |h| format!("{h} px high"));
    let role = match (row.role, row.emphasis) {
        (Some(r), Some(e)) => format!(" ({r} {})", e.name()),
        (Some(r), None) => format!(" ({r})"),
        _ => String::new(),
    };
    let type_role = row
        .type_role
        .map_or_else(String::new, |t| format!(", type {}", t.name()));
    let contrast = match row.contrast.first() {
        Some(r) => format!(
            "{} {:.2}:1, needs {}:1, least on surface.{}",
            r.measure.name(),
            r.found,
            r.required,
            r.against
        ),
        None => "not held to a minimum".into(),
    };
    v_flex()
        .gap(Space::Xs)
        .child(Text::new(format!(
            "{} {}{role}: {height}, {shape}{type_role}; {contrast}",
            row.variant, row.state
        )))
        .child(colour_line(
            "Background",
            row.colours.background,
            row.sources.background,
        ))
        .child(colour_line(
            "Foreground",
            row.colours.foreground,
            row.sources.foreground,
        ))
        .child(colour_line(
            "Border",
            row.colours.border,
            row.sources.border,
        ))
}
/// The panel for `components`, whose state `pick` finds in the page and whose rows `rows` does.
/// `id` keeps its controls' ids apart from another panel's.
pub fn panel<V: 'static>(
    g: &mut V,
    id: &'static str,
    components: &'static [&'static str],
    rows: fn(&mut V) -> &mut Rows,
    pick: fn(&mut V) -> &mut Inspector,
    cx: &mut Context<V>,
) -> Div {
    let tokens = odk::ui::components();
    let theme = odk::ui::theme_diagnostics(cx)
        .and_then(|t| t.name.clone())
        .unwrap_or_else(|| "unnamed".into());
    let rows = rows(g).get(&tokens);
    let i = pick(g);
    let (component, density) = (i.component, i.density);

    let f = odk::ui::foundation();
    let metrics = f.controls.for_density(density);
    let mut lines = v_flex()
        .gap(Space::Xs)
        .child(Text::new(format!(
            "Theme: {theme} (generation {})",
            tokens.generation()
        )))
        .child(Text::new(format!(
            "Foundation: {} density, control padding {} px, icon {} px, gap {} px; border {} px, focus ring {} px at {} px offset; motion fast {} ms, normal {} ms, enter {} ms, exit {} ms{}",
            density.name(),
            metrics.padding_x,
            metrics.icon_size,
            metrics.gap,
            f.geometry.border_width,
            f.geometry.focus_width,
            f.geometry.focus_offset,
            f.motion.fast.as_millis(),
            f.motion.normal.as_millis(),
            f.motion.enter.as_millis(),
            f.motion.exit.as_millis(),
            if f.motion.reduced { " (reduced: transitions run for 0 ms)" } else { "" }
        )));
    for row in rows
        .iter()
        .filter(|r| r.component == component && r.density.is_none_or(|d| d == density))
    {
        lines = lines.child(row_lines(row));
    }
    let ids: (&'static str, &'static str, &'static str) = match id {
        "forms" => (
            "forms-inspect-component",
            "forms-inspect-density",
            "forms-inspect-rows",
        ),
        "nav" => (
            "nav-inspect-component",
            "nav-inspect-density",
            "nav-inspect-rows",
        ),
        "overlays" => (
            "overlays-inspect-component",
            "overlays-inspect-density",
            "overlays-inspect-rows",
        ),
        "fields" => (
            "fields-inspect-component",
            "fields-inspect-density",
            "fields-inspect-rows",
        ),
        "selection" => (
            "selection-inspect-component",
            "selection-inspect-density",
            "selection-inspect-rows",
        ),
        "dialogs" => (
            "dialogs-inspect-component",
            "dialogs-inspect-density",
            "dialogs-inspect-rows",
        ),

        _ => (
            "feedback-inspect-component",
            "feedback-inspect-density",
            "feedback-inspect-rows",
        ),
    };
    v_flex()
        .gap(Space::Sm)
        .child(choice(
            ids.0,
            "Component",
            components.iter().map(|c| (*c, *c)).collect(),
            component,
            move |g, c, _| pick(g).component = c,
            cx,
        ))
        .child(choice(
            ids.1,
            "Density",
            Density::ALL.map(|d| (d, d.name())).to_vec(),
            density,
            move |g, d, _| pick(g).density = d,
            cx,
        ))
        .child(
            div()
                .id(ids.2)
                .max_h(px(480.0))
                .overflow_y_scroll()
                .child(lines),
        )
}
