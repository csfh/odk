//! The dialog's window on Rune: what the application asks for, and the buttons that answer.
//!
//! It decides nothing and keeps nothing (ADR 0025): a press, Enter on a focused button, or
//! Escape sets the answer and ends the program. Nothing is focused to begin with, so no key but
//! Escape does anything until the person tabs to a button: Enter can never grant by reflex.

use std::sync::{Arc, Mutex};

use rune::{Color, Cx, El, Key, KeyEvent, TextStyle, Tx, col, el, row, spacer, text};

use crate::request::{Request, label};
use crate::theme::Theme;

/// The answer, shared between the window and `main`.
pub type Answer = Arc<Mutex<Option<String>>>;

/// The window's width in points.
pub const WIDTH: f32 = 480.0;

pub struct Dialog {
    request: Request,
    theme: Theme,
    answer: Answer,
}

impl Dialog {
    pub fn new(request: Request, theme: Theme, answer: Answer) -> Self {
        Self {
            request,
            theme,
            answer,
        }
    }

    fn give(&self, word: &str, tx: &mut Tx) {
        *self.answer.lock().unwrap_or_else(|e| e.into_inner()) = Some(word.to_owned());
        tx.quit();
    }

    /// Escape, with or without a focused control, is Cancel.
    pub fn on_key(dialog: &mut Dialog, ev: &KeyEvent, tx: &mut Tx) -> bool {
        if ev.key == Key::Escape {
            dialog.give("cancel", tx);
            return true;
        }
        false
    }

    pub fn view(dialog: &Dialog, _: &mut Cx<Dialog>) -> El {
        dialog.build()
    }

    fn build(&self) -> El {
        let t = self.theme;
        let words = |content: String, size: f32, weight: f32, colour: Color| {
            text(content)
                .font(TextStyle::new(size, weight))
                .color(colour)
                .a11y_hidden()
        };
        let mut lines = col().gap(4.0);
        for line in &self.request.lines {
            lines = lines.child(
                row()
                    .gap(8.0)
                    .child(words("•".into(), 13.0, 400.0, t.accent))
                    .child(words(line.clone(), 13.0, 400.0, t.foreground).shrink()),
            );
        }
        let mut page = col()
            .key("dialog")
            .role("dialog")
            .label(format!("Allow {}?", self.request.name))
            .w_full()
            .h_full()
            .padding(20.0)
            .gap(12.0)
            .fill(t.background)
            .child(
                col()
                    .gap(4.0)
                    .child(
                        row()
                            .gap(8.0)
                            .child(words(self.request.name.clone(), 20.0, 700.0, t.foreground))
                            .child(words(
                                format!("version {}", self.request.version),
                                12.0,
                                400.0,
                                t.muted,
                            )),
                    )
                    .child(words(self.request.identifier.clone(), 11.0, 400.0, t.muted)),
            )
            .child(words(
                "A new application. It asks for these permissions:".into(),
                13.0,
                400.0,
                t.foreground,
            ))
            .child(lines);
        for note in &self.request.notes {
            page = page.child(words(note.clone(), 12.0, 400.0, t.muted));
        }
        let mut buttons = row().gap(8.0).child(spacer());
        for word in &self.request.buttons {
            buttons = buttons.child(self.button(word));
        }
        page.child(spacer()).child(buttons)
    }

    fn button(&self, word: &str) -> El {
        let t = self.theme;
        let primary = word != "cancel";
        let shown = label(word).unwrap_or("?");
        let (click, enter) = (word.to_owned(), word.to_owned());
        el().key(format!("button-{word}"))
            .role("button")
            .label(shown)
            .focusable(true)
            .px(16.0)
            .py(8.0)
            .radius(6.0)
            .center()
            .fill(if primary {
                t.accent
            } else {
                Color::TRANSPARENT
            })
            .outline(1.0, -1.0, if primary { t.accent } else { t.border })
            .hover(if primary {
                t.accent.alpha(0.85)
            } else {
                t.border.alpha(0.2)
            })
            // A press, and Space on a focused button, answer; Enter is taken here, since Rune
            // presses a focused control on Space and not on Enter.
            .on_press_tx(move |dialog: &mut Dialog, tx: &mut Tx| dialog.give(&click, tx))
            .on_key(move |dialog: &mut Dialog, ev: &KeyEvent, tx: &mut Tx| {
                if ev.key == Key::Enter {
                    dialog.give(&enter, tx);
                    return true;
                }
                false
            })
            .child(words_in_button(
                shown,
                if primary { t.on_accent } else { t.foreground },
            ))
    }
}

fn words_in_button(shown: &str, colour: Color) -> El {
    text(shown.to_owned())
        .font(TextStyle::new(13.0, 400.0))
        .color(colour)
        .a11y_hidden()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Runtime};

    const REQUEST: &str = r#"
name = "Hello"
identifier = "dev.example.hello"
version = "1.0.0"
buttons = ["install", "run-once", "cancel"]
lines = ["Reads your Documents folder", "Uses the network"]
notes = ["Your Music folder is not available."]
"#;

    fn open() -> (Runtime<Dialog>, Answer) {
        let answer: Answer = Arc::new(Mutex::new(None));
        let request = Request::parse(REQUEST).unwrap();
        let height = request.height();
        let dialog = Dialog::new(request, Theme::load(), answer.clone());
        let mut runtime = Runtime::new(dialog, Dialog::view, (WIDTH, height), Env::new(true, 1.0));
        runtime.on_key = Some(Dialog::on_key);
        settle(&mut runtime);
        (runtime, answer)
    }

    fn settle(runtime: &mut Runtime<Dialog>) {
        for _ in 0..8 {
            runtime.frame(1.0 / 60.0);
        }
    }

    fn given(answer: &Answer) -> Option<String> {
        answer.lock().unwrap().clone()
    }

    fn focused(runtime: &Runtime<Dialog>) -> Option<String> {
        runtime
            .elements()
            .into_iter()
            .find(|e| e.focused)
            .and_then(|e| e.key)
    }

    fn press(runtime: &mut Runtime<Dialog>, key: Key) {
        runtime.key(KeyEvent::from(key.clone()));
        runtime.key_up(KeyEvent::from(key));
        settle(runtime);
    }

    #[test]
    fn nothing_is_focused_at_first_and_enter_grants_nothing() {
        let (mut runtime, answer) = open();
        assert_eq!(focused(&runtime), None);
        press(&mut runtime, Key::Enter);
        press(&mut runtime, Key::Text(" ".into()));
        assert_eq!(given(&answer), None, "no button is the default");
    }

    #[test]
    fn tab_reaches_the_buttons_in_order_and_enter_answers_for_the_focused_one() {
        let (mut runtime, answer) = open();
        press(&mut runtime, Key::Tab);
        assert_eq!(focused(&runtime).as_deref(), Some("button-install"));
        press(&mut runtime, Key::Tab);
        assert_eq!(focused(&runtime).as_deref(), Some("button-run-once"));
        press(&mut runtime, Key::Enter);
        assert_eq!(given(&answer).as_deref(), Some("run-once"));
    }

    #[test]
    fn space_on_a_focused_button_answers_for_it() {
        let (mut runtime, answer) = open();
        press(&mut runtime, Key::Tab);
        press(&mut runtime, Key::Text(" ".into()));
        assert_eq!(given(&answer).as_deref(), Some("install"));
    }

    #[test]
    fn a_click_answers_for_the_button_clicked() {
        let (mut runtime, answer) = open();
        let (x, y) = runtime.find("button-cancel").expect("the cancel button");
        runtime.pointer_down(x, y);
        runtime.pointer_up(x, y);
        settle(&mut runtime);
        assert_eq!(given(&answer).as_deref(), Some("cancel"));
    }

    #[test]
    fn escape_is_cancel_with_nothing_focused_and_with_a_button_focused() {
        let (mut runtime, answer) = open();
        press(&mut runtime, Key::Escape);
        assert_eq!(given(&answer).as_deref(), Some("cancel"));

        let (mut runtime, answer) = open();
        press(&mut runtime, Key::Tab);
        assert_eq!(focused(&runtime).as_deref(), Some("button-install"));
        press(&mut runtime, Key::Escape);
        assert_eq!(given(&answer).as_deref(), Some("cancel"));
    }

    #[test]
    fn the_window_says_what_it_is_and_each_button_is_named() {
        let (runtime, _) = open();
        fn walk(node: &serde_json::Value, into: &mut Vec<serde_json::Value>) {
            into.push(node.clone());
            for child in node["children"].as_array().into_iter().flatten() {
                walk(child, into);
            }
        }
        let mut nodes = Vec::new();
        walk(&runtime.accessibility_json(), &mut nodes);
        let named =
            |role: &str, name: &str| nodes.iter().any(|n| n["role"] == role && n["name"] == name);
        assert!(named("dialog", "Allow Hello?"), "{nodes:?}");
        for label in ["Install", "Run once", "Cancel"] {
            assert!(named("button", label), "no button named {label}");
        }
    }
}
