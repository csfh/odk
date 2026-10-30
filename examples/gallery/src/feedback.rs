//! The Feedback page: badges in every Role and Emphasis, status indicators, alerts, progress
//! bars, spinners, skeletons and separators, and the diagnostics of their component tokens. Everything here is a real component in the active
//! Omarchy theme; in a monochrome one every Role is a grey, and the marks and words still say
//! what each means.

use odk::gpui::{Div, px};
use odk::prelude::*;
use odk::theme::semantic::Role as Intent;

use crate::buttons::{caption, row};
use crate::content::Content;
use crate::{Settings, diagnostics};

/// The Feedback page: its own state, and the Content switch it follows.
pub struct FeedbackPage {
    worst_case: bool,
    alert_dismissed: bool,
    saving: bool,
    /// The progress bar's value and whether it is paused, and whether the skeleton's content has
    /// arrived.
    progress: f64,
    progress_paused: bool,
    skeleton_loaded: bool,
    inspector: diagnostics::Inspector,
    diagnostics: diagnostics::Rows,
}

impl FeedbackPage {
    pub fn new(settings: &Entity<Settings>, cx: &mut Context<Self>) -> Self {
        cx.observe(settings, |this, settings, cx| {
            this.worst_case = settings.read(cx).worst_case;
            cx.notify();
        })
        .detach();
        Self {
            worst_case: settings.read(cx).worst_case,
            alert_dismissed: false,
            saving: false,
            progress: 40.0,
            progress_paused: false,
            skeleton_loaded: false,
            inspector: diagnostics::Inspector::new("progress"),
            diagnostics: diagnostics::Rows::default(),
        }
    }
}

impl Render for FeedbackPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = Content::get(self.worst_case);
        crate::page(crate::Page::Feedback, section(self, &c, cx))
    }
}

/// The components the diagnostics panel offers.
const COMPONENTS: [&str; 8] = [
    "badge",
    "status dot",
    "alert",
    "progress",
    "spinner",
    "skeleton",
    "separator",
    "status",
];

/// The six Roles, named for a badge's label.
const ROLES: [(Intent, &str); 6] = [
    (Intent::Neutral, "Neutral"),
    (Intent::Accent, "Accent"),
    (Intent::Success, "Success"),
    (Intent::Warning, "Warning"),
    (Intent::Danger, "Danger"),
    (Intent::Info, "Info"),
];

fn group(title: &'static str) -> impl IntoElement {
    Heading::new(title).level(3)
}

/// The section.
pub fn section(g: &mut FeedbackPage, c: &Content, cx: &mut Context<FeedbackPage>) -> Div {
    v_flex()
        .flex_shrink_0()
        .gap(Space::Lg)
        .child(group("Badges"))
        .child(badges(c))
        .child(group("Status indicators"))
        .child(statuses(c))
        .child(group("Alerts"))
        .child(alerts(g, c, cx))
        .child(group("Progress"))
        .child(progress(g, c, cx))
        .child(group("Spinners"))
        .child(spinners(g, c, cx))
        .child(group("Skeletons"))
        .child(skeletons(g, cx))
        .child(group("Separators"))
        .child(separators())
        .child(group("Diagnostics"))
        .child(caption(
            "The tokens one feedback component draws with, where each came from, and its contrast.",
        ))
        .child(diagnostics::panel(
            g,
            "feedback",
            &COMPONENTS,
            |g| &mut g.diagnostics,
            |g| &mut g.inspector,
            cx,
        ))
}

fn badges(c: &Content) -> Div {
    let matrix = |emphasis: Emphasis, outlined: bool| {
        row().children(ROLES.map(|(role, name)| {
            let badge = Badge::new(name).role(role).emphasis(emphasis);
            if outlined { badge.outlined() } else { badge }
        }))
    };
    let [secondary, primary, ghost, info, success, warning, danger] = c.badges;
    v_flex()
        .gap(Space::Sm)
        .child(caption(
            "Solid, muted (the default for a status) and subtle, in each Role.",
        ))
        .child(matrix(Emphasis::Solid, false))
        .child(matrix(Emphasis::Muted, false))
        .child(matrix(Emphasis::Subtle, false))
        .child(caption("Subtle with an edge."))
        .child(matrix(Emphasis::Subtle, true))
        .child(caption(
            "Status badges with their marks, so they read without colour; an icon; counters.",
        ))
        .child(
            row()
                .child(Badge::new(info).tone(Tone::Info).marked())
                .child(Badge::new(success).tone(Tone::Success).marked())
                .child(Badge::new(warning).tone(Tone::Warning).marked())
                .child(Badge::new(danger).tone(Tone::Danger).marked())
                .child(
                    Badge::new(primary)
                        .role(Intent::Accent)
                        .emphasis(Emphasis::Solid)
                        .icon(Icon::StarFilled),
                )
                .child(Badge::new(secondary))
                .child(
                    Badge::new(ghost).appearance(
                        BadgeAppearance::new(Intent::Neutral, Emphasis::Subtle).outlined(),
                    ),
                )
                .child(Badge::new("3"))
                .child(
                    Badge::new("99+")
                        .role(Intent::Danger)
                        .emphasis(Emphasis::Solid),
                ),
        )
}

fn statuses(c: &Content) -> Div {
    let [active, inactive, success, warning, error, info] = c.statuses;
    v_flex()
        .gap(Space::Sm)
        .child(
            row()
                .gap(Space::Lg)
                .child(StatusIndicator::new(Status::Active, active))
                .child(StatusIndicator::new(Status::Inactive, inactive))
                .child(StatusIndicator::new(Status::Success, success))
                .child(StatusIndicator::new(Status::Warning, warning))
                .child(StatusIndicator::new(Status::Error, error))
                .child(StatusIndicator::new(Status::Info, info)),
        )
        .child(caption(
            "The label always says what the status is; an inactive one is a hollow ring.",
        ))
}

fn alerts(g: &mut FeedbackPage, c: &Content, cx: &mut Context<FeedbackPage>) -> Div {
    let tones = [Tone::Info, Tone::Success, Tone::Warning, Tone::Danger];
    let mut list = v_flex().gap(Space::Sm);
    for (n, ((title, description, action), tone)) in c.alerts.into_iter().zip(tones).enumerate() {
        if tone == Tone::Danger && g.alert_dismissed {
            continue;
        }
        let mut alert = Alert::new(("alert", n), title).variant(tone);
        if !description.is_empty() {
            alert = alert.description(description);
        }
        if !action.is_empty() {
            alert = alert.action(
                Button::new(("alert-action", n))
                    .label(action)
                    .size(Size::Sm),
            );
        }
        if tone == Tone::Danger {
            alert = alert.on_dismiss(cx.listener(|this, _, _, cx| {
                this.alert_dismissed = true;
                cx.notify();
            }));
        }
        list = list.child(alert);
    }
    if g.alert_dismissed {
        list = list.child(
            Button::new("alert-restore")
                .label("Show the dismissed alert")
                .variant(ButtonVariant::Ghost)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.alert_dismissed = false;
                    cx.notify();
                })),
        );
    }
    let (title, description, _) = c.alerts[2];
    list.child(caption("Subtle: no fill, an edge in the tone's colour."))
        .child(
            Alert::new("alert-subtle", title)
                .variant(Tone::Warning)
                .emphasis(Emphasis::Subtle)
                .description(description),
        )
        .child(caption(
            "Compact, for a dense place, and solid, for what must not be missed.",
        ))
        .child(
            Alert::new("alert-compact", "Saved")
                .variant(Tone::Success)
                .compact(true)
                .on_dismiss(|_, _, _| {}),
        )
        .child(
            Alert::new("alert-solid", "Backend unavailable")
                .variant(Tone::Danger)
                .emphasis(Emphasis::Solid),
        )
}

fn progress(g: &mut FeedbackPage, c: &Content, cx: &mut Context<FeedbackPage>) -> Div {
    let value = g.progress;
    let step = |label: &'static str, by: f64| {
        Button::new(SharedString::from(format!("progress-{label}")))
            .label(label)
            .size(Size::Sm)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.progress = (this.progress + by).clamp(0.0, 100.0);
                cx.notify();
            }))
    };
    let pause = Button::new("progress-pause")
        .label(if g.progress_paused { "Resume" } else { "Pause" })
        .size(Size::Sm)
        .on_click(cx.listener(|this, _, _, cx| {
            this.progress_paused = !this.progress_paused;
            cx.notify();
        }));
    v_flex()
        .gap(Space::Md)
        .max_w(px(480.0))
        .child({
            let bar = Progress::new("progress", c.progress)
                .value(value)
                .paused(g.progress_paused)
                .show_label(true)
                .show_value(true);
            if value >= 100.0 {
                bar.tone(Tone::Success)
            } else {
                bar
            }
        })
        .child(
            row()
                .child(step("−10", -10.0))
                .child(step("+10", 10.0))
                .child(pause),
        )
        .child(caption(
            "A new value moves the bar over 180 ms, at once with reduced motion. Done turns it to success.",
        ))
        .child(Progress::new("progress-accent", "Indexing").value(35.0).show_value(true))
        .child(
            Progress::new("progress-files", "Copying files")
                .value(3.0)
                .max(12.0)
                .value_text("3 of 12 files")
                .show_label(true)
                .show_value(true),
        )
        .child(
            Progress::new("progress-failed", "Upload failed")
                .value(60.0)
                .tone(Tone::Danger)
                .show_label(true)
                .show_value(true),
        )
        .child(
            Progress::new("progress-off", "Waiting for the network")
                .value(20.0)
                .disabled(true)
                .show_label(true)
                .show_value(true),
        )
        .child(caption("A bare bar, which says its value only to assistive technology:"))
        .child(Progress::new("progress-bare", "Syncing").value(70.0))
}

fn spinners(g: &mut FeedbackPage, c: &Content, cx: &mut Context<FeedbackPage>) -> Div {
    let [save, delete, reset] = c.buttons;
    row()
        .items_center()
        .child(Spinner::new("sm").size(Size::Sm))
        .child(Spinner::new("md"))
        .child(
            Spinner::new("lg")
                .size(Size::Lg)
                .label("Loading the gallery"),
        )
        .child(
            Button::new("save")
                .label(save)
                .variant(ButtonVariant::Primary)
                .loading(g.saving)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.saving = true;
                    cx.notify();
                })),
        )
        .child(Button::new("off").label(delete).disabled(true))
        .child(
            Button::new("stop")
                .label(reset)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.saving = false;
                    cx.notify();
                })),
        )
        .child(caption(
            "Standing alone in secondary text; in a button in its colour.",
        ))
}

fn skeletons(g: &mut FeedbackPage, cx: &mut Context<FeedbackPage>) -> Div {
    let card = if g.skeleton_loaded {
        h_flex()
            .items_start()
            .gap(Space::Md)
            .child(
                div()
                    .size(px(48.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Heading::new("A").level(2)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(Text::new("Aleksandra Wiśniewska"))
                    .child(Text::new("Joined the Workspace team today.").muted())
                    .child(Text::new("Three files shared.").muted()),
            )
    } else {
        h_flex()
            .items_start()
            .gap(Space::Md)
            .child(Skeleton::block(48.0, 48.0).label("Loading the picture"))
            .child(v_flex().flex_1().min_w_0().child(Skeleton::text(3)))
    };
    v_flex()
        .gap(Space::Md)
        .max_w(px(480.0))
        .child(
            Switch::new("skeleton-loaded", "Loaded")
                .checked(g.skeleton_loaded)
                .on_change({
                    let gallery = cx.entity();
                    move |on, _, cx| {
                        gallery.update(cx, |g, cx| {
                            g.skeleton_loaded = on;
                            cx.notify();
                        })
                    }
                }),
        )
        .child(card)
        .child(
            Item::new("Keyboard layout")
                .description("Read from the compositor, so it waits for it.")
                .action(if g.skeleton_loaded {
                    Select::new("skeleton-layout", "Keyboard layout")
                        .item("us", "English (US)")
                        .item("dk", "Danish")
                        .default_value("us")
                        .into_any_element()
                } else {
                    Skeleton::control()
                        .label("Reading the keyboard layout")
                        .into_any_element()
                }),
        )
        .child(Skeleton::line())
        .child(caption(
            "Exactly the size of what it stands for, so nothing moves when it arrives; a control's stand-in is a control's height and three times as wide, where the control will be. It stands still.",
        ))
}

fn separators() -> Div {
    v_flex()
        .gap(Space::Sm)
        .max_w(px(480.0))
        .child(Text::new("Display"))
        .child(Separator::horizontal())
        .child(
            h_flex()
                .gap(Space::Sm)
                .child(Text::new("Scaling").muted())
                .child(Separator::vertical())
                .child(Text::new("Night light").muted())
                .child(Separator::vertical())
                .child(Text::new("Refresh rate").muted()),
        )
        .child(Separator::horizontal().inset())
        .child(caption("The last one is inset by a control's padding."))
}
