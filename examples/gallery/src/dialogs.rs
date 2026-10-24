//! The Dialogs page: dialogs, sheets and toasts, the overlays that cover the page or interrupt
//! it. A dialog and a modal sheet block the window behind them and keep Tab inside; a side panel
//! does not; a toast never takes focus. Everything here is a real component in the active Omarchy
//! theme; use the toolbar's density, corner and Motion switches to see each at every setting and
//! with and without motion.

use std::time::Duration;

use odk::prelude::*;
use odk::ui::{
    Alert, Button, ButtonVariant, Dialog, Input, Select, Sheet, Side, Spinner, Toast, ToastId,
    Tone, toast,
};

use crate::buttons::{caption, row};
use crate::content::Content;
use crate::{Settings, page, section};

/// Which dialog is open.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Demo {
    Confirm,
    Destructive,
    Form,
    Saving,
}

pub struct DialogsPage {
    worst_case: bool,
    demo: Option<Demo>,
    /// The side and whether it blocks the window.
    sheet: Option<(Side, bool)>,
    name: SharedString,
    /// What is wrong with the form dialog's name, shown in the dialog.
    error: Option<SharedString>,
    uploading: bool,
    inspector: crate::diagnostics::Inspector,
    diagnostics: crate::diagnostics::Rows,
}

impl DialogsPage {
    pub fn new(settings: &Entity<Settings>, cx: &mut Context<Self>) -> Self {
        cx.observe(settings, |this, settings, cx| {
            this.worst_case = settings.read(cx).worst_case;
            cx.notify();
        })
        .detach();
        Self {
            worst_case: settings.read(cx).worst_case,
            demo: None,
            sheet: None,
            name: "notes.md".into(),
            error: None,
            uploading: false,
            inspector: crate::diagnostics::Inspector::new("dialog"),
            diagnostics: crate::diagnostics::Rows::default(),
        }
    }

    fn open(&mut self, demo: Demo, cx: &mut Context<Self>) {
        self.demo = Some(demo);
        self.error = None;
        if demo == Demo::Saving {
            Self::finish_saving_later(cx);
        }
        cx.notify();
    }

    fn close_dialog(&mut self, cx: &mut Context<Self>) {
        self.demo = None;
        cx.notify();
    }
}

fn group(title: &'static str) -> impl IntoElement {
    Heading::new(title).level(3)
}

impl Render for DialogsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = Content::get(self.worst_case);
        let entity = cx.entity();

        let opener = |id: &'static str, label: &'static str, demo: Demo, cx: &mut Context<Self>| {
            Button::new(id)
                .label(label)
                .on_click(cx.listener(move |this, _, _, cx| this.open(demo, cx)))
        };
        let dialogs = row()
            .child(opener("open-confirm", "Confirm…", Demo::Confirm, cx))
            .child(opener("open-delete", "Delete…", Demo::Destructive, cx))
            .child(opener("open-form", "Rename…", Demo::Form, cx))
            .child(opener("open-saving", "Save…", Demo::Saving, cx));

        let sheet_button = |side: Side, modal: bool, label: String, cx: &mut Context<Self>| {
            Button::new(SharedString::from(format!("sheet-{side:?}-{modal}")))
                .label(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.sheet = Some((side, modal));
                    cx.notify();
                }))
        };
        let sheets = row()
            .child(sheet_button(Side::Right, true, "Right sheet".into(), cx))
            .child(sheet_button(Side::Left, true, "Left sheet".into(), cx))
            .child(sheet_button(Side::Bottom, true, "Bottom sheet".into(), cx))
            .child(sheet_button(
                Side::Right,
                false,
                "Side panel (not modal)".into(),
                cx,
            ));

        let (toast_title, toast_text, toast_action) = c.toast;
        let raise =
            |id: &'static str,
             label: &'static str,
             build: fn(&'static str, &'static str, &'static str) -> Toast| {
                Button::new(id).label(label).on_click(move |_, window, cx| {
                    toast(window, cx, build(toast_title, toast_text, toast_action))
                })
            };
        let toasts = row()
            .child(raise("toast-neutral", "Neutral", |t, d, _| {
                Toast::new(t).neutral().description(d)
            }))
            .child(raise("toast-info", "Info", |t, d, _| {
                Toast::new(t).variant(Tone::Info).description(d)
            }))
            .child(raise("toast-success", "Success", |t, d, _| {
                Toast::new(t).variant(Tone::Success).description(d)
            }))
            .child(raise("toast-warning", "Warning", |t, d, _| {
                Toast::new(t).variant(Tone::Warning).description(d)
            }))
            .child(raise("toast-danger", "Danger", |t, d, _| {
                Toast::new(t).variant(Tone::Danger).description(d)
            }))
            .child(raise("toast-action", "With an action", |t, d, a| {
                Toast::new(t).description(d).action(a, |_, _| {})
            }))
            .child(raise("toast-persistent", "Persistent error", |_, _, _| {
                Toast::new("Could not save notes.md")
                    .variant(Tone::Danger)
                    .description("It stays until you dismiss it.")
                    .persistent()
            }))
            .child(Button::new("toast-many").label("Five at once").on_click(
                move |_, window, cx| {
                    for n in 1..=5 {
                        toast(
                            window,
                            cx,
                            Toast::new(format!("Message {n}"))
                                .description("Three show; the rest wait."),
                        );
                    }
                },
            ))
            .child({
                let uploading = self.uploading;
                Button::new("toast-update")
                    .label(if uploading {
                        "Finish the upload"
                    } else {
                        "Start an upload"
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        // One id for both: the second toast replaces the first in place.
                        let id = ToastId::named("upload");
                        let t = if this.uploading {
                            Toast::new("Upload finished")
                                .variant(Tone::Success)
                                .with_id(id)
                        } else {
                            Toast::new("Uploading…").with_id(id).neutral().persistent()
                        };
                        toast(window, cx, t);
                        this.uploading = !this.uploading;
                        cx.notify();
                    }))
            });

        let (dialog_title, dialog_text, [cancel, rename]) = c.dialog;
        let (sheet_title, sheet_body, [sheet_ok, sheet_cancel]) = c.sheet;

        let shut = |entity: &Entity<Self>| {
            let e = entity.clone();
            move |_: &mut Window, cx: &mut App| e.update(cx, |this, cx| this.close_dialog(cx))
        };

        page(
            crate::Page::Dialogs,
            v_flex()
                .flex_shrink_0()
                .gap(Space::Lg)
                .child(caption(
                    "A dialog blocks the window behind it, keeps Tab inside and gives focus back when it closes. Escape closes it unless the dialog says otherwise.",
                ))
                .child(section("Dialog"))
                .child(group("Confirmation, destructive, form and busy"))
                .child(dialogs)
                .child(section("Sheet"))
                .child(caption(
                    "A modal sheet dims the window and traps focus; the side panel leaves the window live, takes no focus and closes with Escape while focus is in it.",
                ))
                .child(sheets)
                .child(section("Toast"))
                .child(caption(
                    "Toasts never take focus. Tab reaches their buttons; the clock stops while the pointer or focus is on the stack.",
                ))
                .child(toasts)
                .child(section("Diagnostics"))
                .child(caption(
                    "The tokens a dialog, a sheet or a toast draws with, where each came from, and its contrast.",
                ))
                .child(crate::diagnostics::panel(
                    self,
                    "dialogs",
                    &["dialog", "sheet", "toast"],
                    |g| &mut g.diagnostics,
                    |g| &mut g.inspector,
                    cx,
                ))
                // A confirmation: every way out closes it.
                .child(
                    Dialog::new("dlg-confirm", "Discard changes?")
                        .open(self.demo == Some(Demo::Confirm))
                        .description("Your edits to notes.md will be lost.")
                        .on_close(shut(&entity))
                        .action({
                            let close = shut(&entity);
                            move |_, _| {
                                let close = close.clone();
                                Button::new("confirm-keep")
                                    .label("Keep editing")
                                    .on_click(move |_, w, cx| close(w, cx))
                            }
                        })
                        .action({
                            let close = shut(&entity);
                            move |_, _| {
                                let close = close.clone();
                                Button::new("confirm-discard")
                                    .label("Discard")
                                    .variant(ButtonVariant::Primary)
                                    .on_click(move |_, w, cx| close(w, cx))
                            }
                        }),
                )
                // Destructive: a click on the backdrop does not dismiss it.
                .child(
                    Dialog::new("dlg-delete", "Delete 12 files?")
                        .open(self.demo == Some(Demo::Destructive))
                        .description("This cannot be undone. Only the buttons answer it.")
                        .close_on_backdrop(false)
                        .on_close(shut(&entity))
                        .action({
                            let close = shut(&entity);
                            move |_, _| {
                                let close = close.clone();
                                Button::new("delete-cancel")
                                    .label("Cancel")
                                    .on_click(move |_, w, cx| close(w, cx))
                            }
                        })
                        .action({
                            let close = shut(&entity);
                            move |_, _| {
                                let close = close.clone();
                                Button::new("delete-confirm")
                                    .label("Delete")
                                    .variant(ButtonVariant::Destructive)
                                    .on_click(move |_, w, cx| close(w, cx))
                            }
                        }),
                )
                // A form: validation keeps it open and says why.
                .child({
                    let name = self.name.clone();
                    let error = self.error.clone();
                    let (change, submit) = (entity.clone(), entity.clone());
                    Dialog::new("dlg-form", dialog_title)
                        .open(self.demo == Some(Demo::Form))
                        .description(dialog_text)
                        .on_close(shut(&entity))
                        .content(move |_, _| {
                            let change = change.clone();
                            let mut body = v_flex().gap(Space::Sm);
                            if let Some(error) = error.clone() {
                                body = body.child(
                                    Alert::new("form-error", "Check the name")
                                        .variant(Tone::Danger)
                                        .description(error)
                                        .compact(true),
                                );
                            }
                            body.child(
                                Input::new("form-name", "File name")
                                    .value(name.clone())
                                    .invalid(error.is_some())
                                    .on_change(move |text, _, cx| {
                                        let text: SharedString = text.to_owned().into();
                                        change.update(cx, |this, cx| {
                                            this.name = text;
                                            cx.notify();
                                        })
                                    }),
                            )
                            .child(
                                Select::new("form-folder", "Folder")
                                    .item("docs", "Documents")
                                    .item("pics", "Pictures")
                                    .item("music", "Music")
                                    .default_value("docs"),
                            )
                        })
                        .action({
                            let close = shut(&entity);
                            move |_, _| {
                                let close = close.clone();
                                Button::new("form-cancel")
                                    .label(cancel)
                                    .on_click(move |_, w, cx| close(w, cx))
                            }
                        })
                        .action(move |_, _| {
                            let submit = submit.clone();
                            Button::new("form-rename")
                                .label(rename)
                                .variant(ButtonVariant::Primary)
                                .on_click(move |_, _, cx| {
                                    submit.update(cx, |this, cx| {
                                        if this.name.trim().is_empty() {
                                            this.error = Some("A file needs a name.".into());
                                        } else if this.name.contains('/') {
                                            this.error = Some("A name cannot contain a slash.".into());
                                        } else {
                                            this.error = None;
                                            this.demo = None;
                                        }
                                        cx.notify();
                                    })
                                })
                        })
                })
                // Busy: nothing closes it until the work is done.
                .child({
                    let busy = self.demo == Some(Demo::Saving);
                    Dialog::new("dlg-saving", "Saving notes.md")
                        .open(busy)
                        .description("Please wait.")
                        .close_on_escape(false)
                        .close_on_backdrop(false)
                        .close_button(false)
                        .content(|_, _| Spinner::new("saving-spinner").label("Writing the file…"))
                        .action(|_, _| {
                            Button::new("saving-busy")
                                .label("Saving")
                                .loading(true)
                        })
                        .on_close(shut(&entity))
                        .action(|_, _| Button::new("saving-cancel").label("Cancel").disabled(true))
                })
                .child({
                    let (side, modal) = self.sheet.unwrap_or((Side::Right, true));
                    let close = entity.clone();
                    Sheet::new("sheet", sheet_title)
                        .open(self.sheet.is_some())
                        .side(side)
                        .modal(modal)
                        .on_close(move |_, cx| {
                            close.update(cx, |this, cx| {
                                this.sheet = None;
                                cx.notify();
                            })
                        })
                        .content(move |_, _| {
                            v_flex()
                                .gap(Space::Md)
                                .child(Text::new(sheet_body))
                                .child(Select::new("sheet-select", "Monitor").item("a", "DP-1").item("b", "HDMI-A-1").default_value("a"))
                                .child(Button::new("apply").label(sheet_ok).variant(ButtonVariant::Primary))
                                .child(Button::new("cancel").label(sheet_cancel))
                        })
                }),
        )
    }
}

/// How long the "Save…" demo pretends to work.
const SAVING: Duration = Duration::from_millis(1500);

impl DialogsPage {
    /// Starts the busy dialog's work: it closes itself when the time is up.
    pub fn finish_saving_later(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SAVING).await;
            let _ = this.update(cx, |this, cx| {
                if this.demo == Some(Demo::Saving) {
                    this.close_dialog(cx);
                }
            });
        })
        .detach();
    }
}
