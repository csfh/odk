//! `oma-consent`: shows what an application asks for and prints the user's answer.
//!
//! It decides nothing and keeps nothing (ADR 0025). The request is TOML on standard input and
//! the answer is one word on standard output, as in `docs/spec/consent-dialog.md`. It uses the
//! pinned GPUI directly and depends on neither `oma` nor ODK (ADR 0016).

mod hyprland;
mod theme;

use std::cell::RefCell;
use std::io::Read;
use std::process::ExitCode;
use std::rc::Rc;

use gpui::{
    App, Application, Bounds, Context, Div, FocusHandle, KeyBinding, KeyDownEvent, SharedString,
    Stateful, TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div, point,
    prelude::*, px, rgba, size,
};
use serde::Deserialize;

use theme::Theme;

actions!(oma_consent, [Cancel, FocusNext, FocusPrevious]);

#[derive(Debug, Deserialize)]
struct Request {
    name: String,
    identifier: String,
    version: String,
    #[serde(default)]
    buttons: Vec<String>,
    #[serde(default)]
    lines: Vec<String>,
    #[serde(default)]
    notes: Vec<String>,
}

const KNOWN: &[(&str, &str)] = &[
    ("install", "Install"),
    ("run-once", "Run once"),
    ("allow", "Allow"),
    ("keep-previous", "Keep previous version"),
    ("cancel", "Cancel"),
];

fn label(word: &str) -> Option<&'static str> {
    KNOWN.iter().find(|(w, _)| *w == word).map(|(_, l)| *l)
}

/// The answer, shared between the window and `main`.
type Answer = Rc<RefCell<Option<String>>>;

struct Dialog {
    request: Request,
    theme: Theme,
    answer: Answer,
    root: FocusHandle,
    buttons: Vec<(String, FocusHandle)>,
}

impl Dialog {
    fn new(
        request: Request,
        theme: Theme,
        answer: Answer,
        cx: &mut Context<Self>,
        window: &mut Window,
    ) -> Self {
        let root = cx.focus_handle();
        // Nothing is focused to begin with, so no key but Escape does anything until the user
        // tabs to a button: Enter can never grant by reflex.
        window.focus(&root);
        window.set_window_title(&format!("Allow {}?", request.name));
        let buttons = request
            .buttons
            .iter()
            .map(|w| (w.clone(), cx.focus_handle().tab_stop(true)))
            .collect();
        Self {
            request,
            theme,
            answer,
            root,
            buttons,
        }
    }

    fn give(&self, word: &str, cx: &mut App) {
        *self.answer.borrow_mut() = Some(word.to_owned());
        cx.quit();
    }
}

impl Render for Dialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let muted = t.muted;
        let buttons = self
            .buttons
            .clone()
            .into_iter()
            .enumerate()
            .map(|(ix, (word, handle))| {
                let primary = word != "cancel";
                let focused = handle.is_focused(window);
                let click_word = word.clone();
                let key_word = word.clone();
                let button: Stateful<Div> = div()
                    .id(("button", ix))
                    .track_focus(&handle)
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .cursor_pointer()
                    .text_size(px(13.0))
                    .border_1()
                    .border_color(if focused {
                        t.foreground
                    } else if primary {
                        t.accent
                    } else {
                        rgba(t.border)
                    })
                    .bg(if primary { t.accent } else { rgba(0) })
                    .text_color(if primary { t.on_accent } else { t.foreground })
                    .hover(move |s| s.opacity(0.85))
                    .on_click(cx.listener(move |this, _, _, cx| this.give(&click_word, cx)))
                    .on_key_down(cx.listener(move |this, ev: &KeyDownEvent, _, cx| {
                        if matches!(ev.keystroke.key.as_str(), "enter" | "space") {
                            this.give(&key_word, cx);
                        }
                    }))
                    .child(label(&word).unwrap_or("?"));
                button
            });

        div()
            .id("dialog")
            .track_focus(&self.root)
            .on_action(cx.listener(|this, _: &Cancel, _, cx| this.give("cancel", cx)))
            .on_action(cx.listener(|_, _: &FocusNext, window, _| window.focus_next()))
            .on_action(cx.listener(|_, _: &FocusPrevious, window, _| window.focus_prev()))
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_5()
            .bg(t.background)
            .text_color(t.foreground)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(20.0))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(self.request.name.clone()),
                            )
                            .child(div().text_size(px(12.0)).text_color(muted).child(
                                SharedString::from(format!("version {}", self.request.version)),
                            )),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(muted)
                            .child(self.request.identifier.clone()),
                    ),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .child("A new application. It asks for these permissions:"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(self.request.lines.iter().map(|line| {
                        div()
                            .flex()
                            .gap_2()
                            .text_size(px(13.0))
                            .child(div().text_color(t.accent).child("•"))
                            .child(line.clone())
                    })),
            )
            .children(self.request.notes.iter().map(|note| {
                div()
                    .text_size(px(12.0))
                    .text_color(muted)
                    .child(note.clone())
            }))
            .child(div().flex_grow())
            .child(div().flex().justify_end().gap_2().children(buttons))
    }
}

fn height(request: &Request) -> f32 {
    214.0 + 24.0 * request.lines.len() as f32 + 20.0 * request.notes.len() as f32
}

fn main() -> ExitCode {
    let mut text = String::new();
    if std::io::stdin().read_to_string(&mut text).is_err() {
        eprintln!("oma-consent: cannot read the request");
        return ExitCode::from(2);
    }
    let request: Request = match toml::from_str(&text) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("oma-consent: bad request: {e}");
            return ExitCode::from(2);
        }
    };
    if request.buttons.is_empty() || request.buttons.iter().any(|b| label(b).is_none()) {
        eprintln!("oma-consent: the request offers no usable buttons");
        return ExitCode::from(2);
    }

    let answer: Answer = Rc::new(RefCell::new(None));
    let result = answer.clone();
    Application::new().run(move |cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("escape", Cancel, None),
            KeyBinding::new("tab", FocusNext, None),
            KeyBinding::new("shift-tab", FocusPrevious, None),
        ]);
        let theme = Theme::load();
        let bounds = Bounds::centered(None, size(px(480.0), px(height(&request))), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some(format!("Allow {}?", request.name).into()),
                appears_transparent: false,
                traffic_light_position: Some(point(px(0.0), px(0.0))),
            }),
            app_id: Some("oma-consent".into()),
            is_resizable: false,
            ..Default::default()
        };
        let answer = result.clone();
        let (width, window_height) = (480, height(&request) as u32);
        cx.open_window(options, move |window, cx| {
            cx.new(|cx| Dialog::new(request, theme, answer, cx, window))
        })
        .expect("cannot open the window");
        hyprland::float_dialog(width, window_height);
        // Closing the window without choosing is Cancel.
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });

    let given = answer
        .borrow()
        .clone()
        .unwrap_or_else(|| "cancel".to_owned());
    println!("{given}");
    ExitCode::SUCCESS
}
