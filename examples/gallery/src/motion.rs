//! The Motion page: the foundation's durations and curve, and the components that use them,
//! each with Play and Reset. Every demonstration is a real component moved by the real motion
//! tokens; nothing here animates by itself, so the page asks for no frame while it is idle.

use odk::gpui::px;
use odk::prelude::*;
use odk::theme::foundation::{CubicBezier, MotionDuration};

use crate::buttons::row;
use crate::present::{flag, group, kv, page, section};
use crate::registry::Page;

pub const DURATIONS: [(&str, MotionDuration); 6] = [
    ("instant", MotionDuration::Instant),
    ("fast", MotionDuration::Fast),
    ("normal", MotionDuration::Normal),
    ("slow", MotionDuration::Slow),
    ("enter", MotionDuration::Enter),
    ("exit", MotionDuration::Exit),
];

/// What the page's demonstrations are doing.
#[derive(Default)]
pub struct MotionPage {
    on: bool,
    tab: usize,
    progress: bool,
    dialog: bool,
}

impl MotionPage {
    pub fn new() -> Self {
        Self::default()
    }

    /// Plays every demonstration: each goes to its other end.
    fn play(&mut self, cx: &mut Context<Self>) {
        self.on = true;
        self.tab = 1;
        self.progress = true;
        cx.notify();
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        *self = Self::default();
        cx.notify();
    }
}

impl Render for MotionPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let f = odk::ui::foundation();
        let m = &f.motion;
        let user = odk::ui::theme_snapshot().reduced_motion;

        let mut table = v_flex().gap(Space::Xs);
        for (name, duration) in DURATIONS {
            table = table.child(kv(
                format!("motion.{name}"),
                format!(
                    "{} ms{}",
                    m.duration(duration).as_millis(),
                    if m.reduced {
                        " (runs for 0 ms while motion is reduced)"
                    } else {
                        ""
                    }
                ),
            ));
        }
        for (name, d) in [
            ("cycle", m.cycle),
            ("hover_open", m.hover_open),
            ("hover_close", m.hover_close),
            ("toast", m.toast),
        ] {
            table = table.child(kv(
                format!("motion.{name}"),
                format!("{} ms (provisional; not a transition)", d.as_millis()),
            ));
        }
        let b = CubicBezier::STANDARD;
        let curve = (0..=10)
            .map(|n| {
                let t = n as f32 / 10.0;
                let y = m.easing.apply(t);
                (t, y)
            })
            .collect::<Vec<_>>();
        let mut plot = h_flex().items_end().gap(px(3.0)).h(px(72.0));
        let ink = odk::ui::theme_snapshot().semantic.text.secondary;
        for (_, y) in &curve {
            plot = plot.child(
                div()
                    .w(px(14.0))
                    .h(px((y * 72.0).max(1.0)))
                    .bg(odk::gpui::rgba(ink.to_rgba_u32())),
            );
        }

        let controls = row()
            .child(
                Button::new("motion-play")
                    .label("Play")
                    .variant(ButtonVariant::Primary)
                    .on_click(cx.listener(|this, _, _, cx| this.play(cx))),
            )
            .child(
                Button::new("motion-reset")
                    .label("Reset")
                    .variant(ButtonVariant::Outline)
                    .on_click(cx.listener(|this, _, _, cx| this.reset(cx))),
            )
            .child(
                Button::new("motion-reduce")
                    .label(if m.reduced {
                        "Motion is reduced"
                    } else {
                        "Reduce motion"
                    })
                    .variant(ButtonVariant::Outline)
                    .disabled(user)
                    .on_click(|_, _, cx| {
                        odk::ui::set_reduced_motion(cx, !odk::ui::foundation().motion.reduced)
                    }),
            );
        let status = Text::new(if user {
            "Reduced by the system's request (Hyprland's animations are off, or [motion] reduced in shell.toml when it does not answer); an application cannot turn that off."
        } else if m.reduced {
            "Reduced by this gallery's switch. The user has not asked for it."
        } else {
            "Full motion. Reduce it here, or in the toolbar, to see every transition land at once."
        })
        .muted();

        let demos = v_flex()
            .gap(Space::Lg)
            .child(
                v_flex()
                    .gap(Space::Sm)
                    .child(group("Switch: normal"))
                    .child(Switch::new("motion-switch", "Switch").checked(self.on).on_change(
                        flag(cx, |this: &mut Self, on, cx| {
                            this.on = on;
                            cx.notify();
                        }),
                    )),
            )
            .child(
                v_flex()
                    .gap(Space::Sm)
                    .child(group("Tab indicator: normal"))
                    .child(
                        Tabs::new("motion-tabs", "Demo")
                            .value(if self.tab == 0 { "one" } else { "two" })
                            .tab("one", "One", |_, _| div())
                            .tab("two", "Two", |_, _| div())
                            .on_change(cx.listener(|this, value: &SharedString, _, cx| {
                                this.tab = usize::from(value.as_ref() == "two");
                                cx.notify();
                            })),
                    ),
            )
            .child(
                v_flex()
                    .gap(Space::Sm)
                    .child(group("Progress: the bar's own change"))
                    .child(
                        Progress::new("motion-progress", "Copying")
                            .value(if self.progress { 100.0 } else { 0.0 })
                            .show_value(true),
                    ),
            )
            .child(
                v_flex()
                    .gap(Space::Sm)
                    .child(group("Popover: enter, then exit"))
                    .child(Text::new("Press the button to open it, Escape to close it. A press outside closes it and goes no further, so Play and Reset do not touch it.").muted())
                    .child(
                        Popover::new(
                            "motion-popover",
                            "Entrance",
                            Button::new("motion-popover-button").label("Open the popover"),
                        )
                        .title("Entrance")
                        .content(|_, _| Text::new("Over `enter`, out over `exit`.")),
                    ),
            )
            .child(
                v_flex()
                    .gap(Space::Sm)
                    .child(group("Dialog: enter, then exit"))
                    .child(
                        Button::new("motion-dialog-open")
                            .label("Open the dialog")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.dialog = true;
                                cx.notify();
                            })),
                    )
                    .child(
                        Dialog::new("motion-dialog", "Entrance")
                            .open(self.dialog)
                            .description("Close it to see it leave.")
                            .on_close({
                                let entity = cx.entity();
                                move |_, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.dialog = false;
                                        cx.notify();
                                    })
                                }
                            }),
                    ),
            );

        page(
            Page::Motion,
            v_flex()
                .gap(Space::Lg)
                .child(section("Durations"))
                .child(table)
                .child(section("Easing"))
                .child(kv(
                    "motion.easing",
                    format!(
                        "cubic-bezier({}, {}, {}, {}) — fast out of the start, settling gently",
                        b.x1, b.y1, b.x2, b.y2
                    ),
                ))
                .child(Text::new("Progress at t = 0, 0.1 … 1:").muted())
                .child(plot)
                .child(section("Components"))
                .child(status)
                .child(controls)
                .child(Text::new("These are the production components. Hover and press change at once: GPUI has no style transitions, so the brief's hover transition is not built (docs/spec/odk-interaction.md).").muted())
                .child(demos),
        )
    }
}
