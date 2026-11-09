//! A key or chord on the Rune host, such as `Ctrl+K`, drawn as one keycap per key in the
//! monospace face. Small enough to sit inline in text, in tooltips and in command items.

use crate::describe::apply;
use crate::host::Hosted;
use crate::ink::ink;
use crate::style::{self, text_style};
use odk_a11y::{Accessible, Describe, Role};
use rune::{El, row, text};

pub struct Kbd {
    keys: Vec<String>,
}

impl Kbd {
    /// A chord written with `+` between keys: `"Ctrl+Shift+K"`. `"Ctrl++"` is Ctrl and Plus.
    pub fn new(chord: &str) -> Self {
        Self {
            keys: parse_chord(chord),
        }
    }

    /// A chord given key by key.
    pub fn keys<S: Into<String>>(keys: impl IntoIterator<Item = S>) -> Self {
        Self {
            keys: keys.into_iter().map(Into::into).collect(),
        }
    }

    pub fn show<T>(self, hosted: &Hosted<T>) -> El {
        let accessible = self.accessible();
        let paint = style::paint(hosted.theme());
        let kbd = paint.tokens.kbd();
        let geometry = kbd.geometry;
        let caps = self.keys.into_iter().map(|key| {
            row()
                .h(geometry.height)
                .px(geometry.padding_x)
                .center()
                .radius(geometry.radius)
                .fill(ink(kbd.colours.background))
                .outline(geometry.border_width, 0.0, ink(kbd.colours.border))
                .child(
                    text(key)
                        .font(text_style(paint.role(geometry.type_role)))
                        .color(ink(kbd.colours.foreground))
                        .a11y_hidden(),
                )
        });
        apply(row().center_y().gap(geometry.gap), &accessible).children(caps)
    }
}

impl Describe for Kbd {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Keys, self.keys.join("+"))
    }
}

pub(crate) fn parse_chord(chord: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut rest = chord;
    while !rest.is_empty() {
        let (key, tail) = match rest.find('+') {
            // A `+` at the start of a segment is the Plus key itself.
            Some(0) => ("+", &rest[1..]),
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, ""),
        };
        keys.push(key.trim().to_owned());
        rest = tail
            .strip_prefix('+')
            .filter(|_| key != "+")
            .unwrap_or(tail);
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chords() {
        assert_eq!(parse_chord("Ctrl+Shift+K"), ["Ctrl", "Shift", "K"]);
        assert_eq!(parse_chord("K"), ["K"]);
        assert_eq!(parse_chord("Ctrl++"), ["Ctrl", "+"]);
        assert_eq!(parse_chord("+"), ["+"]);
        assert_eq!(Kbd::new("Ctrl+K").accessible().name, "Ctrl+K");
    }
}
