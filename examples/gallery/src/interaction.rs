//! The Accessibility page (`Page::Interaction`): the controls that act, with a log of what they actually did. Tab through
//! them, press Enter and Space, use the arrow keys, open the dialog and the menu and close them
//! with Escape, and watch the log: every line is an event that reached a handler (a click, with
//! the key or the pointer that made it; a change; an open or a close), a key that was pressed (not Tab and Shift+Tab, which are bound actions that GPUI does not
//! deliver as keys; their effect shows as a move of focus), or a move of keyboard focus read
//! back from the window. Nothing in it is simulated, and nothing is
//! inferred: a component whose handler is not told which key made a change says so by logging
//! the key on its own line. The state panel below reads the tokens being drawn, never the
//! resolver.
//!
//! Change the density, the corners, the theme and the Motion switch in the toolbar while a
//! dialog is open, or mid-transition, to see that nothing is lost.

use std::time::Duration;

use odk::gpui::{ClickEvent, KeyDownEvent, KeyboardButton};
use odk::prelude::*;
use odk::theme::component::{ButtonLayout, ButtonVariant, States, status_mark};
use odk::theme::foundation::MotionDuration;
use odk::ui::{
    Button, Checkbox, Dialog, DropdownMenu, MenuItem, Popover, RadioGroup, Select, Switch, Tabs,
    ToggleGroup, Tooltip, focused_component,
};

use crate::buttons::{caption, row};
use crate::diagnostics::Rows;
use crate::{Settings, page, section};

/// How many lines the log keeps.
const LOG_LINES: usize = 14;

pub struct InteractionPage {
    log: Vec<(usize, String)>,
    /// The number of the next line.
    seq: usize,
    /// Where focus was when the page last drew.
    focus: Option<SharedString>,
    dialog: bool,
    popover: bool,
    working: bool,
    /// Callbacks received, by button: what they count is real handler calls and nothing else.
    saved: usize,
    started: usize,
    disabled_called: usize,
    rows: Rows,
}

impl InteractionPage {
    pub fn new(settings: &Entity<Settings>, cx: &mut Context<Self>) -> Self {
        cx.observe(settings, |_, _, cx| cx.notify()).detach();
        Self {
            log: Vec::new(),
            seq: 1,
            focus: None,
            dialog: false,
            popover: false,
            working: false,
            saved: 0,
            started: 0,
            disabled_called: 0,
            rows: Rows::default(),
        }
    }

    /// The lines the log holds, oldest first, without their numbers. For the tests.
    pub fn lines(&self) -> Vec<String> {
        self.log.iter().map(|(_, line)| line.clone()).collect()
    }

    fn note(&mut self, text: impl Into<String>) {
        self.log.push((self.seq, text.into()));
        self.seq += 1;
        if self.log.len() > LOG_LINES {
            self.log.remove(0);
        }
    }
}

/// The component a focus handle's path names: the application's id for it, without the part.
fn named(path: Option<&SharedString>) -> String {
    path.map_or("nothing".to_owned(), |p| {
        p.split('/').next().unwrap_or(p).to_owned()
    })
}

/// What made a click: the pointer, or the key GPUI says was released on the control.
fn source(event: &ClickEvent) -> &'static str {
    match event {
        ClickEvent::Mouse(_) => "the pointer",
        ClickEvent::Keyboard(key) => match key.button {
            KeyboardButton::Enter => "Enter",
            KeyboardButton::Space => "Space",
        },
    }
}

/// A handler that adds `text` to the page's log, for a component whose handler is told nothing
/// but the value.
fn logger<T: ?Sized>(
    entity: &Entity<InteractionPage>,
    text: impl Fn(&T) -> String + 'static,
) -> impl Fn(&T, &mut Window, &mut App) + 'static {
    let entity = entity.clone();
    move |value, _, cx| {
        let line = text(value);
        entity.update(cx, |this, cx| {
            this.note(line);
            cx.notify();
        });
    }
}

/// [`logger`] for a handler that is given its value, as a check box's and a switch's are.
fn flag_logger(
    entity: &Entity<InteractionPage>,
    text: impl Fn(bool) -> String + 'static,
) -> impl Fn(bool, &mut Window, &mut App) + 'static {
    let log = logger(entity, move |on: &bool| text(*on));
    move |on, window, cx| log(&on, window, cx)
}

impl Render for InteractionPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let config = odk::ui::foundation_config(cx);

        // Focus is read back from the window each time the page is drawn, and a focus move is
        // drawn: GPUI does not redraw on one, and `move_focus` asks for it.
        let focused = focused_component(window, cx);
        if focused != self.focus {
            let line = format!(
                "focus moved from {} to {}",
                named(self.focus.as_ref()),
                named(focused.as_ref())
            );
            self.note(line);
            self.focus = focused.clone();
        }

        // Buttons.
        let buttons = row()
            .child(
                Button::new("ix-save")
                    .label("Save")
                    .variant(ButtonVariant::Primary)
                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                        this.saved += 1;
                        this.note(format!("Save activated using {}", source(event)));
                        cx.notify();
                    })),
            )
            .child(
                Button::new("ix-work")
                    .label(if self.working {
                        "Working…"
                    } else {
                        "Start work"
                    })
                    .loading(self.working)
                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                        this.started += 1;
                        this.working = true;
                        this.note(format!("Work started using {}", source(event)));
                        cx.notify();
                        // Two seconds of work, then done: a timer, not a loop, so the page is
                        // idle again afterwards.
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(Duration::from_secs(2)).await;
                            let _ = this.update(cx, |this, cx| {
                                this.working = false;
                                this.note("Work finished; the button is live again");
                                cx.notify();
                            });
                        })
                        .detach();
                    })),
            )
            .child(
                Button::new("ix-disabled")
                    .label("Disabled")
                    .disabled(true)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.disabled_called += 1;
                        this.note("the disabled button was activated (this is a bug)");
                        cx.notify();
                    })),
            )
            .child(Tooltip::new(
                "ix-tip",
                "Close the panel",
                Button::new("ix-close")
                    .label("×")
                    .variant(ButtonVariant::Ghost)
                    .accessible_name("Close the panel")
                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                        this.note(format!("Close activated using {}", source(event)));
                        cx.notify();
                    })),
            ));
        let counts = caption(format!(
            "Save callbacks: {}. Start work callbacks: {} (a press while it works adds none). Disabled callbacks: {}.",
            self.saved, self.started, self.disabled_called
        ));

        // Selection controls.
        let selection = v_flex()
            .gap(Space::Md)
            .child(
                row()
                    .gap(Space::Xl)
                    .child(
                        Checkbox::new("ix-check", "Wrap lines").on_change(flag_logger(
                            &entity,
                            |on| {
                                format!(
                                    "Checkbox changed to {}",
                                    if on { "checked" } else { "unchecked" }
                                )
                            },
                        )),
                    )
                    .child(
                        Switch::new("ix-switch", "Wi-Fi").on_change(flag_logger(&entity, |on| {
                            format!("Switch changed to {}", if on { "on" } else { "off" })
                        })),
                    ),
            )
            .child(
                RadioGroup::new("ix-radio", "Fruit")
                    .item("apple", "Apple")
                    .item("pear", "Pear")
                    .item("plum", "Plum")
                    .on_change(logger(&entity, |v: &str| format!("Radio group chose {v}"))),
            )
            .child(
                Tabs::new("ix-tabs", "Sections")
                    .tab("general", "General", |_, _| div())
                    .tab("display", "Display", |_, _| div())
                    .tab("about", "About", |_, _| div())
                    .on_change(logger(&entity, |v: &SharedString| {
                        format!("Tab {v} selected")
                    })),
            )
            .child(
                ToggleGroup::new("ix-align", "Align")
                    .item("left", "Left")
                    .item("centre", "Centre")
                    .item("right", "Right")
                    .on_change(logger(&entity, |v: &[SharedString]| {
                        if v.is_empty() {
                            "Segmented control cleared".to_owned()
                        } else {
                            format!("Segmented control chose {}", v[0])
                        }
                    })),
            )
            .child(
                Select::new("ix-select", "Fruit")
                    .placeholder("Choose a fruit")
                    .item("apple", "Apple")
                    .item("banana", "Banana")
                    .item("cherry", "Cherry")
                    .on_change(logger(&entity, |v: &str| format!("Select committed {v}"))),
            );

        // Overlays.
        let e = entity.clone();
        let overlays = row()
            .gap(Space::Md)
            .child(
                Button::new("ix-open-dialog")
                    .label("Open dialog…")
                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                        this.dialog = true;
                        this.note(format!("Dialog opened using {}", source(event)));
                        cx.notify();
                    })),
            )
            .child(DropdownMenu::new("ix-menu", "Actions").menu({
                let e = entity.clone();
                move |m| {
                    let pick = |label: &'static str| {
                        let e = e.clone();
                        move |_: &mut Window, cx: &mut App| {
                            e.update(cx, |this, cx| {
                                this.note(format!("Menu item {label} chosen"));
                                cx.notify();
                            })
                        }
                    };
                    m.item(MenuItem::new("Rename").on_select(pick("Rename")))
                        .item(MenuItem::new("Duplicate").on_select(pick("Duplicate")))
                        .item(
                            MenuItem::new("Archive")
                                .disabled(true)
                                .on_select(pick("Archive (disabled: a bug)")),
                        )
                        .item(MenuItem::new("Delete").danger().on_select(pick("Delete")))
                }
            }))
            .child(
                Popover::new(
                    "ix-popover",
                    "Details",
                    Button::new("ix-popover-button").label("Details"),
                )
                .title("Details")
                .content(|_, _| Text::new("Escape or a click outside closes this.").muted())
                .on_open_change({
                    let log = logger(&e, |open: &bool| {
                        format!("Popover {}", if *open { "opened" } else { "closed" })
                    });
                    let e = e.clone();
                    move |open, window, cx| {
                        e.update(cx, |this, _| this.popover = open);
                        log(&open, window, cx);
                    }
                }),
            );

        // The log: newest last.
        let mut lines = v_flex().gap(Space::Xs);
        if self.log.is_empty() {
            lines = lines.child(Text::new("Nothing has happened yet.").muted());
        }
        for (n, line) in &self.log {
            lines = lines.child(Text::new(format!("{n:>3}  {line}")));
        }
        let log = v_flex()
            .gap(Space::Sm)
            .child(
                row().child(
                    Button::new("ix-clear")
                        .label("Clear the log")
                        .variant(ButtonVariant::Outline)
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.log.clear();
                            cx.notify();
                        })),
                ),
            )
            .child(lines);

        // The state panel, from the tokens being drawn.
        let f = odk::ui::foundation();
        let ms = |d: std::time::Duration| format!("{} ms", d.as_millis());
        let snapshot = odk::ui::theme_snapshot();
        let components = odk::ui::components();
        let ring = components
            .button(
                ButtonVariant::Secondary,
                config.density,
                ButtonLayout::Text,
                States::NONE.focused(true),
            )
            .focus;
        let rows = self.rows.get(&components);
        let ring_contrast = rows
            .iter()
            .find(|r| r.component == "button" && r.state == "focus")
            .and_then(|r| r.contrast.first())
            .map_or("not measured".to_owned(), |c| {
                format!(
                    "{:.2}:1 (needs {}:1, least on surface.{})",
                    c.found, c.required, c.against
                )
            });
        let mut open = Vec::new();
        if self.dialog {
            open.push("dialog");
        }
        if self.popover {
            open.push("popover");
        }
        let state = v_flex()
            .gap(Space::Xs)
            .child(Text::new(format!(
                "Theme: {} (generation {})",
                snapshot.name.as_deref().unwrap_or("unnamed"),
                snapshot.generation
            )))
            .child(Text::new(format!(
                "Density: {}. Corners: {} px.",
                config.density.name(),
                f.geometry.radius.default
            )))
            .child(Text::new(format!(
                "Reduced motion: {} (the application's switch: {}; the user's shell.toml: {})",
                if f.motion.reduced { "on" } else { "off" },
                if config.reduced_motion { "on" } else { "off" },
                if snapshot.reduced_motion { "asks" } else { "silent" },
            )))
            .child(Text::new(format!(
                "Motion: fast {}, normal {}, slow {}, enter {}, exit {}; easing {:?} (cubic-bezier 0.22, 1, 0.36, 1)",
                ms(f.motion.transition(MotionDuration::Fast)),
                ms(f.motion.transition(MotionDuration::Normal)),
                ms(f.motion.transition(MotionDuration::Slow)),
                ms(f.motion.transition(MotionDuration::Enter)),
                ms(f.motion.transition(MotionDuration::Exit)),
                f.motion.easing,
            )))
            .child(Text::new(format!(
                "Focus: {}. Ring: {} px wide, {} px out, {}; {ring_contrast}",
                named(self.focus.as_ref()),
                ring.width,
                ring.offset,
                ring.colour.to_hex(),
            )))
            .child(Text::new(format!(
                "Overlays this page opened and has not seen close: {}",
                if open.is_empty() { "none".to_owned() } else { open.join(", ") }
            )));

        // Status without colour: each tone's mark and word, then the same with the colour taken
        // away, which loses nothing.
        let tones = [
            (Tone::Info, "Info"),
            (Tone::Success, "Saved"),
            (Tone::Warning, "Check"),
            (Tone::Danger, "Failed"),
        ];
        let status = v_flex()
            .gap(Space::Sm)
            .child(caption(
                "Every status says what it is in words and in a mark. The second row has no hue at all and means the same.",
            ))
            .child(row().children(
                tones
                    .iter()
                    .map(|(tone, word)| Badge::new(*word).tone(*tone).marked()),
            ))
            .child(row().children(
                tones
                    .iter()
                    .map(|(tone, word)| Badge::new(format!(
                        "{} {word}",
                        status_mark(odk::theme::semantic::Role::from(*tone)).unwrap_or("")
                    ))),
            ));

        // Contrast, from the component tokens' diagnostics: for each component the pair with the
        // least room over its minimum. Token-level only.
        let mut tightest: Vec<(&str, String, f32, f32, &str)> = Vec::new();
        for r in rows.iter() {
            for c in &r.contrast {
                let at = tightest.iter().position(|t| t.0 == r.component);
                let label = format!("{} {}", r.variant, r.state);
                let margin = c.found - c.required;
                match at {
                    Some(i) if tightest[i].2 - tightest[i].3 <= margin => {}
                    Some(i) => tightest[i] = (r.component, label, c.found, c.required, c.against),
                    None => tightest.push((r.component, label, c.found, c.required, c.against)),
                }
            }
        }
        tightest.sort_by_key(|t| t.0);
        let mut contrast = v_flex().gap(Space::Xs).child(caption(
            "The pair of each component with the least room over its minimum, from the resolved tokens (WCAG 2: 4.5:1 for text, 3:1 for essential non-text). A token-level check, not a test with a screen reader or an accessibility tree, which the gallery has not had.",
        ));
        for (component, label, found, required, against) in tightest {
            contrast = contrast.child(Text::new(format!(
                "{component}: {label} {found:.2}:1, needs {required}:1, on surface.{against}"
            )));
        }

        let wrap = |body: odk::gpui::Div| {
            body.capture_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                this.note(format!(
                    "key {}{}",
                    event.keystroke.unparse(),
                    if event.is_held { " (held)" } else { "" }
                ));
                cx.notify();
            }))
        };

        page(
            crate::Page::Interaction,
            wrap(
                v_flex()
                    .flex_shrink_0()
                    .gap(Space::Lg)
                        .child(caption(
                        "Tab moves between controls, Shift+Tab back. Enter and Space press a button, Space toggles a check box and a switch, the arrow keys move inside radio groups, tabs, segmented controls and lists, Escape closes what is open.",
                    ))
                    .child(section("Buttons"))
                    .child(buttons)
                    .child(counts)
                    .child(section("Selection"))
                    .child(selection)
                    .child(section("Overlays"))
                    .child(caption(
                        "A dialog keeps Tab inside it and gives focus back to the button that opened it. A menu does the same and closes on Escape. Clicking outside either does not press what is under the pointer.",
                    ))
                    .child(overlays)
                    .child(section("Event log"))
                    .child(log)
                    .child(section("State"))
                    .child(state)
                    .child(section("Status without colour"))
                    .child(status)
                    .child(section("Contrast"))
                    .child(contrast)
                    .child(
                        Dialog::new("ix-dialog", "Discard changes?")
                            .open(self.dialog)
                            .description("Your edits will be lost.")
                            .on_close({
                                let e = entity.clone();
                                move |_, cx| {
                                    e.update(cx, |this, cx| {
                                        this.dialog = false;
                                        this.note("Dialog closed (Escape, the backdrop or ×)");
                                        cx.notify();
                                    })
                                }
                            })
                            .action({
                                let e = entity.clone();
                                move |_, _| {
                                    let e = e.clone();
                                    Button::new("ix-dialog-keep")
                                        .label("Keep editing")
                                        .on_click(move |event, _, cx| {
                                            let from = source(event);
                                            e.update(cx, |this, cx| {
                                                this.dialog = false;
                                                this.note(format!(
                                                    "Keep editing activated using {from}; dialog closed"
                                                ));
                                                cx.notify();
                                            })
                                        })
                                }
                            })
                            .action({
                                let e = entity.clone();
                                move |_, _| {
                                    let e = e.clone();
                                    Button::new("ix-dialog-discard")
                                        .label("Discard")
                                        .variant(ButtonVariant::Destructive)
                                        .on_click(move |event, _, cx| {
                                            let from = source(event);
                                            e.update(cx, |this, cx| {
                                                this.dialog = false;
                                                this.note(format!(
                                                    "Discard activated using {from}; dialog closed"
                                                ));
                                                cx.notify();
                                            })
                                        })
                                }
                            }),
                    ),
            ),
        )
    }
}
