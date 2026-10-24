use gpui::{App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px};
use odk_a11y::{Accessible, Describe, Role};

use crate::components;
use crate::layout::{StyledExt, h_flex};

/// A key or chord, such as `Ctrl+K`, drawn as one keycap per key in the monospace face.
/// Small enough to sit inline in text, in tooltips and in command items.
#[derive(IntoElement)]
pub struct Kbd {
    keys: Vec<SharedString>,
}

impl Kbd {
    /// A chord written with `+` between keys: `"Ctrl+Shift+K"`. `"Ctrl++"` is Ctrl and Plus.
    pub fn new(chord: &str) -> Self {
        Self {
            keys: parse_chord(chord),
        }
    }

    /// A chord given key by key.
    pub fn keys<S: Into<SharedString>>(keys: impl IntoIterator<Item = S>) -> Self {
        Self {
            keys: keys.into_iter().map(Into::into).collect(),
        }
    }
}

fn parse_chord(chord: &str) -> Vec<SharedString> {
    let mut keys = Vec::new();
    let mut rest = chord;
    while !rest.is_empty() {
        let (key, tail) = match rest.find('+') {
            // A `+` at the start of a segment is the Plus key itself.
            Some(0) => ("+", &rest[1..]),
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, ""),
        };
        keys.push(SharedString::from(key.trim().to_owned()));
        rest = tail
            .strip_prefix('+')
            .filter(|_| key != "+")
            .unwrap_or(tail);
    }
    keys
}

impl Describe for Kbd {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Keys, self.keys.join("+"))
    }
}

impl RenderOnce for Kbd {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let k = components().kbd();
        let g = k.geometry;
        h_flex()
            .gap(px(g.gap))
            .children(self.keys.into_iter().map(|key| {
                div()
                    .px(px(g.padding_x))
                    .rounded(px(g.radius))
                    .border(px(g.border_width))
                    .treatment(&k.colours)
                    .text_role(g.type_role)
                    .child(key)
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(chord: &str) -> Vec<String> {
        parse_chord(chord).iter().map(|k| k.to_string()).collect()
    }

    #[test]
    fn parses_chords() {
        assert_eq!(keys("Ctrl+Shift+K"), ["Ctrl", "Shift", "K"]);
        assert_eq!(keys("K"), ["K"]);
        assert_eq!(keys("Ctrl++"), ["Ctrl", "+"]);
        assert_eq!(keys("+"), ["+"]);
        assert_eq!(Kbd::new("Ctrl+K").accessible().name, "Ctrl+K");
    }
}
