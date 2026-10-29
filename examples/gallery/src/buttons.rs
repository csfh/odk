//! The Buttons page: every variant, density, state, content layout and emphasis (at the density
//! and corners the toolbar sets), and the diagnostics of one button picked with the controls,
//! read from the component tokens (`docs/spec/odk-component-tokens.md`). Everything here is a real `Button`
//! in the active Omarchy theme; hover and press it with the pointer, and Tab to see its ring.

use std::sync::Arc;

use odk::gpui::{Div, ElementId, px, rgba};
use odk::prelude::*;
use odk::theme::Color;
use odk::theme::component::{ComponentTokens, Shape, StyleDiagnostic};

use crate::Settings;

/// What the diagnostics panel shows, and the rows it reads, kept until the component tokens
/// are published again: they are built on request, never while drawing.
pub struct Inspector {
    variant: ButtonVariant,
    emphasis: Option<Emphasis>,
    density: Density,
    state: &'static str,
    rows: Option<(Arc<ComponentTokens>, Vec<StyleDiagnostic>)>,
}

impl Default for Inspector {
    fn default() -> Self {
        Self {
            variant: ButtonVariant::Primary,
            emphasis: None,
            density: Density::Default,
            state: "default",
            rows: None,
        }
    }
}

/// The Buttons page: its own state, and the Content switch it follows.
pub struct ButtonsPage {
    worst_case: bool,
    saving: bool,
    /// The recorder example: whether it listens, and the last chord it heard.
    listening: bool,
    heard: SharedString,
    inspector: Inspector,
}

impl ButtonsPage {
    pub fn new(settings: &Entity<Settings>, cx: &mut Context<Self>) -> Self {
        cx.observe(settings, |this, settings, cx| {
            this.worst_case = settings.read(cx).worst_case;
            cx.notify();
        })
        .detach();
        Self {
            worst_case: settings.read(cx).worst_case,
            saving: false,
            listening: false,
            heard: SharedString::default(),
            inspector: Inspector::default(),
        }
    }
}

impl Render for ButtonsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = crate::content::Content::get(self.worst_case);
        crate::page(crate::Page::Buttons, section(self, c.long_button, cx))
    }
}

const STATES: [&str; 5] = ["default", "hover", "pressed", "disabled", "focus"];

/// One of a few options, as a toggle group whose values are the options' places.
pub(crate) fn choice<T: Copy + PartialEq + 'static, V: 'static>(
    id: &'static str,
    label: &'static str,
    options: Vec<(T, &'static str)>,
    current: T,
    set: impl Fn(&mut V, T, &mut Context<V>) + 'static,
    cx: &mut Context<V>,
) -> Div {
    choice_labelled(id, label, true, options, current, set, cx)
}

/// [`choice`] with the label shown or not; it names the group for assistive technology either way.
pub(crate) fn choice_labelled<T: Copy + PartialEq + 'static, V: 'static>(
    id: &'static str,
    label: &'static str,
    show_label: bool,
    options: Vec<(T, &'static str)>,
    current: T,
    set: impl Fn(&mut V, T, &mut Context<V>) + 'static,
    cx: &mut Context<V>,
) -> Div {
    // A current value that is not among the options selects none of them, as when the options
    // are one part of a longer list.
    let at = options.iter().position(|(v, _)| *v == current);
    let mut group = ToggleGroup::new(id, label).value(at.map(|n| n.to_string()));
    for (n, (_, name)) in options.iter().enumerate() {
        group = group.item(n.to_string(), *name);
    }
    h_flex()
        .gap(Space::Md)
        .when(show_label, |row| row.child(Text::new(label).muted()))
        .child(
            group.on_change(cx.listener(move |this, value: &[SharedString], _, cx| {
                // Choosing the chosen option again clears the group; keep what was chosen.
                if let Some(n) = value.first().and_then(|v| v.parse::<usize>().ok()) {
                    set(this, options[n].0, cx);
                    cx.notify();
                }
            })),
        )
}

/// A chord as the gallery writes it: its modifiers, then its key, joined by `+`.
fn chord_text(c: &Chord) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (on, name) in [
        (c.platform, "Super"),
        (c.control, "Ctrl"),
        (c.alt, "Alt"),
        (c.shift, "Shift"),
    ] {
        if on {
            parts.push(name.into());
        }
    }
    parts.push(c.key.to_string());
    parts.join("+")
}

pub(crate) fn row() -> Div {
    h_flex().flex_wrap().gap(Space::Sm)
}

/// The three control heights at the user's text size, for a caption: "28, 34 and 40 px" at 12 px
/// text.
pub(crate) fn heights() -> String {
    let f = odk::ui::foundation();
    let [a, b, c] = Density::ALL.map(|d| f.controls.for_density(d).height);
    format!("{a}, {b} and {c} px")
}

pub(crate) fn caption(text: impl Into<SharedString>) -> impl IntoElement {
    Text::new(text).muted()
}

/// A status variant's mark, so its meaning never rests on its colour alone, as in a
/// monochrome theme.
fn mark(variant: ButtonVariant) -> Option<Icon> {
    match variant {
        ButtonVariant::Destructive | ButtonVariant::Success | ButtonVariant::Warning => {
            Icon::status(variant.appearance().role)
        }
        _ => None,
    }
}

fn titled(variant: ButtonVariant) -> &'static str {
    match variant {
        ButtonVariant::Primary => "Primary",
        ButtonVariant::Secondary => "Secondary",
        ButtonVariant::Outline => "Outline",
        ButtonVariant::Ghost => "Ghost",
        ButtonVariant::Destructive => "Destructive",
        ButtonVariant::Success => "Success",
        ButtonVariant::Warning => "Warning",
        _ => "Link",
    }
}

fn labelled(id: impl Into<ElementId>, variant: ButtonVariant) -> Button {
    let button = Button::new(id).label(titled(variant)).variant(variant);
    match mark(variant) {
        Some(m) => button.leading_icon(m),
        None => button,
    }
}

/// A colour as a small square, edged like a field so a colour close to the surface still
/// shows.
fn swatch(c: Color) -> Div {
    let edge = odk::ui::theme_snapshot().semantic.border.strong;
    div()
        .size(px(14.0))
        .flex_shrink_0()
        .border_1()
        .border_color(rgba(edge.to_rgba_u32()))
        .bg(rgba(c.to_rgba_u32()))
}

/// The section. `long` is the Content switch's longest button label.
pub fn section(g: &mut ButtonsPage, long: &'static str, cx: &mut Context<ButtonsPage>) -> Div {
    v_flex()
        .gap(Space::Md)
        .child(caption(
            "Variants, at the application's density. Tab to see the focus ring; hover and press with the pointer.",
        ))
        .child(row().children(
            ButtonVariant::ALL
                .iter()
                .enumerate()
                .map(|(n, v)| labelled(("variant", n), *v)),
        ))
        .child(caption(format!(
            "Each density, with its own height: {}.",
            heights()
        )))
        .children(Density::ALL.iter().enumerate().map(|(n, &d)| {
            row()
                .child(
                    Button::new(("d-text", n))
                        .label("Save changes")
                        .variant(ButtonVariant::Primary)
                        .density(d),
                )
                .child(
                    Button::new(("d-lead", n))
                        .leading_icon(Icon::Check)
                        .label("Apply")
                        .density(d),
                )
                .child(
                    Button::new(("d-trail", n))
                        .label("Continue")
                        .trailing_icon(Icon::ArrowRight)
                        .variant(ButtonVariant::Outline)
                        .density(d),
                )
                .child(
                    Button::icon(("d-icon", n), Icon::Settings, "Settings")
                        .variant(ButtonVariant::Ghost)
                        .density(d),
                )
                .child(
                    Button::new(("d-delete", n))
                        .leading_icon(Icon::Close)
                        .label("Delete")
                        .variant(ButtonVariant::Destructive)
                        .density(d),
                )
                .child(
                    Button::new(("d-busy", n))
                        .leading_icon(Icon::Check)
                        .label("Saving…")
                        .loading(true)
                        .density(d),
                )
                .child(caption(format!(
                    "{} {} px",
                    d.name(),
                    odk::ui::foundation().controls.for_density(d).height
                )))
        }))
        .child(caption("States: default, disabled, loading, and loading with its text kept."))
        .child(
            row()
                .child(Button::new("s-default").label("Default"))
                .child(Button::new("s-disabled").label("Disabled").disabled(true))
                .child(
                    Button::new("s-disabled-primary")
                        .label("Disabled")
                        .variant(ButtonVariant::Primary)
                        .disabled(true),
                )
                .child(
                    Button::new("s-loading")
                        .label("Loading")
                        .variant(ButtonVariant::Primary)
                        .loading(true),
                )
                .child(
                    Button::new("s-saving")
                        .leading_icon(Icon::Check)
                        .label("Saving…")
                        .variant(ButtonVariant::Primary)
                        .loading(true),
                )
                .child(
                    Button::new("s-try")
                        .leading_icon(Icon::Check)
                        .label(if g.saving { "Saving…" } else { "Save" })
                        .variant(ButtonVariant::Primary)
                        .loading(g.saving)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.saving = true;
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("s-stop")
                        .label("Stop")
                        .variant(ButtonVariant::Ghost)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.saving = false;
                            cx.notify();
                        })),
                ),
        )
        .child(caption("Listening: Record shortcut takes the next key press and the modifiers held with it; Escape cancels."))
        .child(
            row()
                .child(
                    Button::new("s-record")
                        .label(if g.listening { "Listening…" } else { "Record shortcut" })
                        .variant(ButtonVariant::Outline)
                        .listening(g.listening)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.listening = true;
                            cx.notify();
                        }))
                        .on_chord({
                            let page = cx.entity();
                            move |chord, _, cx| {
                                page.update(cx, |this, cx| {
                                    this.listening = false;
                                    if let Some(c) = chord {
                                        this.heard = chord_text(c).into();
                                    }
                                    cx.notify();
                                })
                            }
                        }),
                )
                .children((!g.heard.is_empty()).then(|| Kbd::new(&g.heard))),
        )
        .child(caption("Content: text, leading icon, trailing icon, icon alone, a larger target, and a long label in a narrow width."))
        .child(
            row()
                .child(Button::new("c-text").label("Save changes"))
                .child(Button::new("c-lead").leading_icon(Icon::Check).label("Save changes"))
                .child(Button::new("c-trail").label("Continue").trailing_icon(Icon::ArrowRight))
                .child(Button::icon("c-icon", Icon::Settings, "Settings"))
                .child(
                    Button::icon("c-target", Icon::Settings, "Settings")
                        .variant(ButtonVariant::Outline)
                        .density(Density::Compact)
                        .target_size(44.0),
                )
                .child(
                    div().w(px(160.0)).child(
                        Button::new("c-narrow")
                            .label(long)
                            .variant(ButtonVariant::Secondary)
                            .full_width(),
                    ),
                ),
        )
        .child(caption("Emphasis: solid, muted and subtle for each role that acts."))
        .children(
            [
                ButtonVariant::Primary,
                ButtonVariant::Destructive,
                ButtonVariant::Success,
                ButtonVariant::Warning,
            ]
            .iter()
            .enumerate()
            .map(|(n, &v)| {
                row().children(Emphasis::ALL.iter().enumerate().map(move |(m, &e)| {
                    labelled(("emphasis", n * 3 + m), v)
                        .emphasis(e)
                        .label(format!("{} {}", titled(v), e.name()))
                }))
            }),
        )
        .child(caption("Diagnostics: the tokens one button draws with, and where each came from."))
        .child(inspector(g, cx))
}

/// The diagnostics of the button the controls pick: its tokens by source, its geometry, its
/// contrast and the theme it came from.
fn inspector(g: &mut ButtonsPage, cx: &mut Context<ButtonsPage>) -> Div {
    let tokens = odk::ui::components();
    let stale = !g
        .inspector
        .rows
        .as_ref()
        .is_some_and(|(at, _)| Arc::ptr_eq(at, &tokens));
    if stale {
        let options = odk::ui::theme_snapshot().resolution.options.clone();
        let rows = tokens
            .diagnostics(&options)
            .into_iter()
            .filter(|r| r.component == "button")
            .collect();
        g.inspector.rows = Some((tokens.clone(), rows));
    }
    let i = &g.inspector;
    let appearance = match i.emphasis {
        Some(e) => i.variant.appearance().with_emphasis(e),
        None => i.variant.appearance(),
    };
    let name = appearance.name();
    let rows = &i.rows.as_ref().expect("built above").1;
    let find = |state: &str| {
        rows.iter()
            .find(|r| r.variant == name && r.state == state && r.density == Some(i.density))
    };
    let theme = odk::ui::theme_diagnostics(cx);
    let theme_line = format!(
        "Theme: {} (generation {}), {} mode",
        theme
            .as_ref()
            .and_then(|t| t.name.clone())
            .unwrap_or_else(|| "unnamed".into()),
        tokens.generation(),
        odk::ui::theme_snapshot().semantic.mode.name(),
    );

    let mut lines = v_flex().gap(Space::Xs).child(Text::new(theme_line));
    match find(i.state) {
        Some(row) => {
            lines = lines.child(Text::new(format!(
                "Variant {}, drawn as {} at {} density, state {}",
                i.variant.name(),
                row.variant,
                i.density.name(),
                i.state
            )));
            if i.state == "focus" {
                lines = lines.child(colour_line(
                    "Focus ring",
                    row.colours.foreground,
                    row.sources.foreground,
                ));
            } else {
                lines = lines
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
                    ));
                if let Some(ring) = find("focus") {
                    lines = lines.child(colour_line(
                        "Focus ring",
                        ring.colours.foreground,
                        ring.sources.foreground,
                    ));
                }
            }
            let radius = match row.shape {
                Shape::Rect { radius } => format!("{radius} px"),
                Shape::Pill => "a pill".into(),
                Shape::Circle => "a circle".into(),
            };
            lines = lines.child(Text::new(format!(
                "Height {} px, corner radius {radius}",
                row.height.unwrap_or_default()
            )));
            for r in &row.contrast {
                lines = lines.child(Text::new(format!(
                    "Contrast ({}): {:.2}:1, needs {}:1, least on surface.{}",
                    r.measure.name(),
                    r.found,
                    r.required,
                    r.against
                )));
            }
            if row.contrast.is_empty() {
                lines = lines.child(Text::new(
                    "Contrast: not held to a minimum (an inactive control, WCAG 1.4.3)",
                ));
            }
        }
        None => lines = lines.child(Text::new(format!("No diagnostics for {name}"))),
    }

    let mut sample = Button::new("inspected")
        .label(titled(i.variant))
        .variant(i.variant)
        .density(i.density)
        .disabled(i.state == "disabled");
    if let Some(e) = i.emphasis {
        sample = sample.emphasis(e);
    }
    let variants: Vec<_> = ButtonVariant::ALL.map(|v| (v, v.name())).to_vec();
    let mut emphases = vec![(None, "variant's own")];
    emphases.extend(Emphasis::ALL.map(|e| (Some(e), e.name())));
    let densities: Vec<_> = Density::ALL.map(|d| (d, d.name())).to_vec();
    let states: Vec<_> = STATES.map(|s| (s, s)).to_vec();
    let (variant, emphasis, density, state) = (i.variant, i.emphasis, i.density, i.state);
    v_flex()
        .gap(Space::Sm)
        .child(choice(
            "inspect-variant",
            "Variant",
            variants,
            variant,
            |g, v, _| g.inspector.variant = v,
            cx,
        ))
        .child(
            h_flex()
                .flex_wrap()
                .gap(Space::Xl)
                .child(choice(
                    "inspect-emphasis",
                    "Emphasis",
                    emphases,
                    emphasis,
                    |g, e, _| g.inspector.emphasis = e,
                    cx,
                ))
                .child(choice(
                    "inspect-density",
                    "Density",
                    densities,
                    density,
                    |g, d, _| g.inspector.density = d,
                    cx,
                ))
                .child(choice(
                    "inspect-state",
                    "State",
                    states,
                    state,
                    |g, s, _| g.inspector.state = s,
                    cx,
                )),
        )
        .child(
            h_flex()
                .gap(Space::Lg)
                .items_start()
                .child(sample)
                .child(lines),
        )
}

pub(crate) fn colour_line(
    what: &'static str,
    c: Color,
    source: odk::theme::semantic::ColourRef,
) -> Div {
    h_flex()
        .gap(Space::Sm)
        .child(swatch(c))
        .child(Text::new(format!("{what}: {} from {source}", c.to_hex())))
}
