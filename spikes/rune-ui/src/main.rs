//! Can ODK's components be built on Rune instead of GPUI? A `Button`, a `Select` and an `Input`
//! drawn from what `odkd` would send (palette, text size, font, reduced motion), with no
//! colour, font or motion of Rune's own. `--snap out.png` renders headless (Rune's tooling);
//! never run it without a flag on the owner's desktop (CLAUDE.md, "Windows: headless only").

use rune::{
    Align, AppSpec, Color, Cx, El, Key, KeyEvent, Side, TextEdit, TextStyle, Tx, col, el, row,
    text, text_input,
};

/// What `odkd` tells every application (ADR 0062). A stand-in: the spike does not connect.
#[derive(Clone, Copy)]
pub struct Host {
    pub canvas: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub on_accent: Color,
    pub border: Color,
    pub surface: Color,
    /// `[font] base-size`, which scales type, controls and spacing (ADR 0043).
    pub base_size: f32,
    /// Hyprland's `animations:enabled` inverted, as `odkd` reports it (ADR 0048).
    pub reduced_motion: bool,
}

impl Host {
    /// Tokyo Night-ish values, as a theme would give them.
    pub fn default_host() -> Host {
        Host {
            canvas: hex_const("#1a1b26"),
            text: hex_const("#c0caf5"),
            muted: hex_const("#565f89"),
            accent: hex_const("#7aa2f7"),
            on_accent: hex_const("#1a1b26"),
            border: hex_const("#3b4261"),
            surface: hex_const("#24283b"),
            base_size: 12.0,
            reduced_motion: false,
        }
    }
    fn body(&self) -> TextStyle {
        // Omarchy's font is the fontconfig monospace here; any other family needs its bytes.
        TextStyle::new(self.base_size, 400.0).family(rune::tok::Family::SystemMono)
    }
    fn space(&self, units: f32) -> f32 {
        units * self.base_size / 12.0
    }
}

fn hex_const(s: &str) -> Color {
    rune::color::hex(s).expect("a colour")
}

pub const FRUITS: [&str; 4] = ["Apple", "Banana", "Cherry", "Damson"];

pub struct State {
    pub host: Host,
    pub saves: u32,
    pub fruit: Option<usize>,
    pub open: bool,
    pub highlighted: usize,
    pub name: TextEdit,
}

impl Default for State {
    fn default() -> Self {
        State {
            host: Host::default_host(),
            saves: 0,
            fruit: None,
            open: false,
            highlighted: 0,
            name: TextEdit::new(""),
        }
    }
}

/// `odk::ui::Button`, primary variant: a focus ring, hover and press, Enter and Space.
pub fn button<S: 'static>(
    h: &Host,
    key: &str,
    label: &str,
    on_press: impl Fn(&mut S) + Clone + 'static,
) -> El {
    let hover = h.accent.mix(h.text, 0.15);
    let press = h.accent.mix(h.canvas, 0.2);
    row()
        .key(key)
        .role("button")
        .label(label)
        .focusable(true)
        .focus_ring(2.0, 2.0, h.accent)
        .h(h.space(28.0))
        .px(h.space(12.0))
        .center()
        .radius(2.0)
        .fill(h.accent)
        .hover(hover)
        .press(press)
        .on_press(on_press.clone())
        // Rune presses a focused control on Space's press, and a button not on Enter at all.
        // ODK presses on Enter or Space's release (docs/spec/odk-interaction.md), so the button
        // takes both keys, which `El::on_key` sees before Rune's own activation.
        .on_key(|_: &mut S, k: &KeyEvent, _: &mut Tx| activation(k))
        .on_key_up(move |s: &mut S, k: &KeyEvent, _: &mut Tx| {
            let take = activation(k);
            if take {
                on_press(s);
            }
            take
        })
        .child(text(label).font(h.body()).color(h.on_accent).a11y_hidden())
}

fn activation(k: &KeyEvent) -> bool {
    matches!(&k.key, Key::Enter) || matches!(&k.key, Key::Text(t) if t == " ")
}

/// `odk::ui::Select`: a trigger and a listbox anchored under it, closed by Escape or a press
/// outside, with focus going back to the trigger.
pub fn select(h: &Host, s: &State, layer: &mut Vec<El>) -> El {
    let value = s.fruit.map(|i| FRUITS[i]).unwrap_or("Pick a fruit");
    let mut trigger = row()
        .key("fruit-trigger")
        .role("combobox")
        .label("Fruit")
        .expanded(s.open)
        .focusable(true)
        .focus_ring(2.0, 2.0, h.accent)
        .w(h.space(180.0))
        .h(h.space(28.0))
        .px(h.space(8.0))
        .center_y()
        .radius(2.0)
        .fill(h.surface)
        .outline(1.0, 0.0, h.border)
        .on_press(|s: &mut State| s.open = !s.open)
        .on_key(|s: &mut State, k: &KeyEvent, _: &mut Tx| match k.key {
            Key::Down | Key::Enter if !s.open => {
                s.open = true;
                true
            }
            _ => false,
        })
        .child(
            text(value)
                .font(h.body())
                .color(if s.fruit.is_some() { h.text } else { h.muted })
                .a11y_hidden(),
        );
    if let Some(i) = s.fruit {
        trigger = trigger.a11y_value(FRUITS[i]);
    }
    if !s.open {
        return trigger;
    }
    let mut list = col()
        .key("fruit-listbox")
        .role("listbox")
        .label("Fruit")
        .focus_group()
        .padding(4.0);
    for (i, fruit) in FRUITS.iter().enumerate() {
        let item = row()
            .key(format!("fruit-{i}"))
            .role("option")
            .label(*fruit)
            .focusable(true)
            .focus_selected(s.fruit == Some(i))
            .position_in_set(i + 1, FRUITS.len())
            .h(h.space(24.0))
            .px(h.space(8.0))
            .center_y()
            .radius(2.0)
            .fill(if s.highlighted == i {
                h.border
            } else {
                h.surface
            })
            .on_press(move |s: &mut State| {
                s.fruit = Some(i);
                s.open = false;
            })
            .child(text(*fruit).font(h.body()).color(h.text).a11y_hidden());
        list = list.child(item);
    }
    let popup = col()
        .key("fruit-content")
        .focus_return("fruit-trigger")
        .min_w(h.space(180.0))
        .radius(2.0)
        .fill(h.surface)
        .outline(1.0, 0.0, h.border)
        .modal(true, |s: &mut State| s.open = false)
        .on_key(|s: &mut State, k: &KeyEvent, _: &mut Tx| match k.key {
            Key::Down => {
                s.highlighted = (s.highlighted + 1).min(FRUITS.len() - 1);
                true
            }
            Key::Up => {
                s.highlighted = s.highlighted.saturating_sub(1);
                true
            }
            Key::Enter => {
                s.fruit = Some(s.highlighted);
                s.open = false;
                true
            }
            _ => false,
        })
        .child(list)
        .anchored("fruit-trigger", Side::Bottom, Align::Start, 4.0);
    // A scrim under the popup takes presses outside it.
    layer.push(
        el().key("fruit-scrim")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .on_press(|s: &mut State| s.open = false),
    );
    layer.push(popup);
    trigger
}

/// `odk::ui::Input`: one line, a placeholder, Rune's editor (undo, selection, IME).
pub fn input(h: &Host, s: &State) -> El {
    row()
        .key("name-box")
        .w(h.space(240.0))
        .h(h.space(28.0))
        .px(h.space(8.0))
        .center_y()
        .radius(2.0)
        .fill(h.surface)
        .outline(1.0, 0.0, h.border)
        .child(
            text_input(&s.name, h.body())
                .key("name")
                .label("Name")
                .single_line()
                .placeholder("Your name", h.muted)
                .caret(h.text, h.accent.alpha(0.35))
                .color(h.text)
                .grow(1.0)
                .on_edit(|s: &mut State| &mut s.name),
        )
}

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let h = s.host;
    // ODK's reduced motion comes from odkd alone, never from the host's GNOME or KDE settings.
    cx.reduced_motion(h.reduced_motion);
    let mut layer = Vec::new();
    let body = col()
        .key("page")
        .w_full()
        .h_full()
        .padding(h.space(24.0))
        .gap(h.space(16.0))
        .fill(h.canvas)
        .child(
            text(format!("Saved {} times", s.saves))
                .key("saves")
                .font(h.body())
                .color(h.text),
        )
        .child(button(&h, "save", "Save", |s: &mut State| s.saves += 1))
        .child(select(&h, s, &mut layer))
        .child(input(&h, s));
    el().key("root")
        .w_full()
        .h_full()
        .child(body)
        .children(layer)
}

fn main() {
    rune::run(AppSpec::<State> {
        scenes: &[
            ("open", |s| s.open = true),
            ("large", |s| s.host.base_size = 18.0),
        ],
        ..AppSpec::new("ODK on Rune", (480.0, 360.0), State::default, view)
    });
}

#[cfg(test)]
mod tests;
