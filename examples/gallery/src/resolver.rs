//! The Colour resolver page: why the Adaptive Colour Resolver chose each colour. Everything is
//! the resolver's own report, kept in the snapshot when the theme was loaded: the candidates it
//! weighed, the contrast each had, the reason it gave, and the notes it made. Nothing is chosen
//! or measured again here, so the explanation cannot disagree with the colours.

use odk::prelude::*;
use odk::theme::Color;
use odk::theme::semantic::{Decision, Note, Reason, Report, Role, Source, Subject};

use crate::buttons::choice;
use crate::present::{edge, group, kv, page, section, swatch};
use crate::registry::Page;

/// What the page explains: one role's colours, or the surfaces, text or borders.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    Role(Role),
    Surfaces,
    Text,
    Borders,
}

impl Focus {
    const ALL: [(Focus, &'static str); 9] = [
        (Focus::Role(Role::Accent), "Accent"),
        (Focus::Role(Role::Neutral), "Neutral"),
        (Focus::Role(Role::Danger), "Danger"),
        (Focus::Role(Role::Success), "Success"),
        (Focus::Role(Role::Warning), "Warning"),
        (Focus::Role(Role::Info), "Info"),
        (Focus::Surfaces, "Surfaces"),
        (Focus::Text, "Text"),
        (Focus::Borders, "Borders"),
    ];

    fn covers(self, subject: Subject) -> bool {
        match (self, subject) {
            (Focus::Role(role), s) => subject_role(s) == Some(role),
            (Focus::Surfaces, Subject::Surface(_))
            | (Focus::Text, Subject::Text(_))
            | (Focus::Borders, Subject::Border(_)) => true,
            _ => false,
        }
    }

    fn note_is_about(self, note: &Note) -> bool {
        match self {
            Focus::Role(role) => note_role(note) == Some(role),
            _ => note_role(note).is_none(),
        }
    }
}

fn subject_role(subject: Subject) -> Option<Role> {
    match subject {
        Subject::Fill(r) | Subject::Ink(r) | Subject::Focus(r) | Subject::MutedBorder(r) => Some(r),
        Subject::Treatment(r, _, _) => Some(r),
        _ => None,
    }
}

fn note_role(note: &Note) -> Option<Role> {
    match note {
        Note::MissingOverrideKey { role, .. }
        | Note::FillNearCanvas { role, .. }
        | Note::FocusNearFill { role, .. }
        | Note::StateUnchanged { role, .. } => Some(*role),
        Note::PreferredFails { subject, .. } | Note::TintTooClose { subject, .. } => {
            subject_role(*subject)
        }
        Note::LinkNearText { .. } => None,
    }
}

pub struct ResolverPage {
    focus: Focus,
}

impl ResolverPage {
    pub fn new() -> Self {
        Self {
            focus: Focus::Role(Role::Warning),
        }
    }

    /// What the page explains. For the tests.
    pub fn focus(&self) -> Focus {
        self.focus
    }

    pub fn set_focus(&mut self, focus: Focus, cx: &mut Context<Self>) {
        self.focus = focus;
        cx.notify();
    }
}

impl Default for ResolverPage {
    fn default() -> Self {
        Self::new()
    }
}

fn lch(o: odk::theme::Oklch) -> String {
    format!("L {:.2}  C {:.3}  h {:.0}°", o.l, o.c, o.h)
}

fn keys(keys: &[odk::theme::PaletteKey]) -> String {
    keys.iter()
        .map(|k| format!("`{}`", k.name()))
        .collect::<Vec<_>>()
        .join(" = ")
}

/// One candidate the resolver weighed, as the page words it.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub keys: String,
    pub colour: Color,
    pub contrast: f32,
    pub chosen: bool,
    pub passes: bool,
    pub in_family: bool,
    /// What happened to it: "passes as written", or what it would have become.
    pub verdict: String,
}

/// What the report says about one decision, worded. Built from the decision only; the page and
/// the tests read this, so what is shown cannot differ from what the resolver recorded.
#[derive(Debug, Clone, PartialEq)]
pub struct Explanation {
    pub subject: String,
    pub source: String,
    pub original: Color,
    pub resolved: Color,
    pub original_lch: String,
    pub resolved_lch: String,
    pub adjusted: bool,
    pub contrast: String,
    pub label: Option<String>,
    pub reason: String,
    pub fallback: Option<&'static str>,
    pub candidates: Vec<Candidate>,
}

pub fn explain(d: &Decision) -> Explanation {
    let source = match &d.source {
        Source::Palette(k) => format!("the palette's {}", keys(k)),
        Source::Rule(rule) => format!("a rule: {rule}"),
    };
    let contrast = match (d.requirement, d.contrast_before, d.contrast_after) {
        (Some(r), Some(before), Some(after)) => format!(
            "needs {}:1 against {}; {before:.2}:1 as written, {after:.2}:1 as resolved",
            r.min, r.against
        ),
        (Some(r), _, _) => format!("needs {}:1 against {}", r.min, r.against),
        _ => "no minimum: a rule makes this colour".to_owned(),
    };
    let candidates = d
        .considered
        .iter()
        .map(|c| Candidate {
            keys: keys(&c.keys),
            colour: c.colour,
            contrast: c.contrast,
            chosen: matches!(&d.source, Source::Palette(k) if *k == c.keys),
            passes: c.passes,
            in_family: c.in_family,
            verdict: if c.passes {
                "passes as written".to_owned()
            } else if let Some((colour, change)) = c.adjusted {
                format!(
                    "fails; would become {} (change {change:.3})",
                    colour.to_hex()
                )
            } else {
                "fails and cannot be adjusted to pass".to_owned()
            },
        })
        .collect();
    Explanation {
        subject: d.subject.to_string(),
        source,
        original: d.original,
        resolved: d.resolved,
        original_lch: lch(d.original_lch()),
        resolved_lch: lch(d.resolved_lch()),
        adjusted: d.adjusted(),
        contrast,
        label: d.label.as_ref().map(|(k, c)| format!("{} {}", keys(k), c.to_hex())),
        reason: d.reason.to_string(),
        fallback: (d.reason == Reason::OutsideFamily).then_some(
            "no candidate of the role's hue family can pass, so the nearest of another family is used",
        ),
        candidates,
    }
}

/// The decisions the report holds about `focus`: none when the resolver made no choice there.
pub fn decisions(report: &Report, focus: Focus) -> Vec<&Decision> {
    report
        .decisions
        .iter()
        .filter(|d| focus.covers(d.subject))
        .collect()
}

/// The notes the report holds about `focus`.
pub fn notes(report: &Report, focus: Focus) -> Vec<&Note> {
    report
        .notes
        .iter()
        .filter(|n| focus.note_is_about(n))
        .collect()
}

/// Everything the report says about one decision.
fn card(d: &Decision) -> impl IntoElement {
    let e = explain(d);
    let mut lines = v_flex().gap(Space::Xs);
    lines = lines
        .child(kv("Selected source token", e.source))
        .child(
            h_flex()
                .gap(Space::Md)
                .child(swatch(e.original))
                .child(Text::new(format!(
                    "original {}  ({})",
                    e.original.to_hex(),
                    e.original_lch
                )))
                .child(swatch(e.resolved))
                .child(Text::new(format!(
                    "resolved {}  ({})",
                    e.resolved.to_hex(),
                    e.resolved_lch
                ))),
        )
        .child(kv(
            "Adjustment",
            if e.adjusted {
                "adjusted for contrast: the resolved colour differs from the palette's"
            } else {
                "none: used as the theme wrote it"
            },
        ))
        .child(kv("Contrast", e.contrast));
    if let Some(label) = e.label {
        lines = lines.child(kv("Label drawn on it", label));
    }
    lines = lines.child(kv("Reason", e.reason));
    if let Some(fallback) = e.fallback {
        lines = lines.child(kv("Fallback", fallback));
    }
    if !e.candidates.is_empty() {
        let mut table = v_flex()
            .gap(Space::Xs)
            .child(Text::new("Candidates, in priority order").muted());
        for c in &e.candidates {
            table = table.child(
                h_flex()
                    .flex_wrap()
                    .gap(Space::Md)
                    .child(Text::new(if c.chosen { "● chosen" } else { "○" }))
                    .child(swatch(c.colour))
                    .child(Text::new(format!(
                        "{}  {}  {:.2}:1  {}{}",
                        c.keys,
                        c.colour.to_hex(),
                        c.contrast,
                        c.verdict,
                        if c.in_family {
                            ""
                        } else {
                            "  (another hue family)"
                        }
                    ))),
            );
        }
        lines = lines.child(table);
    }
    v_flex()
        .gap(Space::Sm)
        .p(Space::Md)
        .border_1()
        .border_color(edge())
        .child(Heading::new(e.subject).level(3))
        .child(lines)
}

impl Render for ResolverPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = odk::ui::theme_snapshot();
        let report = &theme.resolution;
        let focus = self.focus;

        let summary = v_flex()
            .gap(Space::Xs)
            .child(kv("Strategy", report.options.strategy.name()))
            .child(kv(
                "Minimum contrast",
                format!(
                    "text {}:1, essential non-text {}:1",
                    report.options.text_contrast, report.options.ui_contrast
                ),
            ))
            .child(kv(
                "Palette",
                format!(
                    "{} mode{}, highest chroma {:.3}",
                    report.mode.name(),
                    if report.monochrome {
                        ", monochrome: no colour is added"
                    } else {
                        ""
                    },
                    report.max_chroma
                ),
            ))
            .child(kv(
                "Decisions",
                format!(
                    "{} recorded, {} adjusted for contrast, {} notes",
                    report.decisions.len(),
                    report.adjusted().count(),
                    report.notes.len()
                ),
            ));

        let mut cards = v_flex().gap(Space::Md);
        let shown = decisions(report, focus);
        for d in &shown {
            cards = cards.child(card(d));
        }
        if shown.is_empty() {
            cards = cards.child(Text::new(
                "The resolver recorded no decision for these values in this theme: they are derived without a choice among candidates, so there is nothing to explain.",
            ));
        }
        let mut notes = v_flex().gap(Space::Xs);
        let about = self::notes(report, focus);
        for note in &about {
            notes = notes.child(Text::new(format!("⚠ {note}")));
        }
        if about.is_empty() {
            notes = notes.child(Text::new("No warnings."));
        }

        page(
            Page::Resolver,
            v_flex()
                .gap(Space::Lg)
                .child(Text::new("Why the theme's colour was used, adjusted or replaced. Choose a role to see each colour it was given, the candidates the resolver weighed, and the contrast each had. This is the resolver's own report, read from the theme's snapshot: nothing is resolved again here.").muted())
                .child(summary)
                .child(choice(
                    "resolver-focus",
                    "Explain",
                    Focus::ALL.to_vec(),
                    focus,
                    |g: &mut ResolverPage, f, _| g.focus = f,
                    cx,
                ))
                .child(section("Decisions"))
                .child(cards)
                .child(group("Warnings"))
                .child(notes),
        )
    }
}
