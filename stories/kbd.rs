//! The Kbd story: a key and a chord drawn as keycaps, given as one string or key by key, a chord
//! that is long or not Latin, and keycaps inline in text and in a column that grows to hold them.
//! Run it in watch mode with `tools/story kbd`.

use odk::gpui::px;
use odk::prelude::*;
use stories::{row, section};

struct KbdStory;

impl Render for KbdStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let chords = row()
            .child(Kbd::new("Esc"))
            .child(Kbd::new("Ctrl+K"))
            .child(Kbd::new("Ctrl+Shift+K"))
            .child(Kbd::new("Alt+Enter"))
            .child(Kbd::new("Ctrl++"));

        let keys = row()
            .child(Kbd::keys(["Ctrl", "Alt", "Delete"]))
            .child(Kbd::keys(["Super", "Space"]))
            .child(Kbd::keys(["Ctrl", "+"]));

        let inline = Stack::new()
            .gap(Space::Sm)
            .child(
                row()
                    .child(Text::new("Press"))
                    .child(Kbd::new("Ctrl+K"))
                    .child(Text::new("to open the command palette.")),
            )
            .child(
                row()
                    .child(Text::new("Close this window with"))
                    .child(Kbd::new("Ctrl+W"))
                    .child(Text::new("or"))
                    .child(Kbd::new("Alt+F4")),
            );

        // A chord keeps its size and its container grows to hold it: the column is a minimum, not
        // a width, and the rule above the chords reaches the end of the longest one.
        let worst = Row::new().child(
            div().min_w(px(180.0)).child(
                Stack::new()
                    .gap(Space::Sm)
                    .child(Separator::horizontal())
                    .child(
                        Stack::new()
                            .gap(Space::Md)
                            .child(Kbd::new("Ctrl+Alt+Shift+Super+PageDown"))
                            .child(Kbd::keys([
                                "Ctrl", "Shift", "Alt", "Super", "Meta", "Hyper",
                            ]))
                            .child(Kbd::keys(["Ctrl", "あ"]))
                            .child(Kbd::keys(["⌘", "⇧", "K"]))
                            .child(Kbd::new("Ctrl+ArrowLeft")),
                    ),
            ),
        );

        root().child(
            scroll_y("kbd-story").child(
                Stack::page()
                    .child(Heading::new("Kbd").level(1))
                    .child(section(
                        "Chords",
                        "One keycap per key, written with `+` between them: `Ctrl++` is Ctrl and Plus.",
                        chords,
                    ))
                    .child(section(
                        "Key by key",
                        "`Kbd::keys` takes the keys one at a time, for a key whose name holds a `+`.",
                        keys,
                    ))
                    .child(section(
                        "Inline",
                        "Small enough to sit in a line of text.",
                        inline,
                    ))
                    .child(section(
                        "Worst case",
                        "A column of at least 180 px that grows to its longest chord, to the rule: a \
                         long chord, many keys, a key that is not Latin and symbols.",
                        worst,
                    )),
            ),
        )
    }
}

fn main() {
    odk::ui::run(|cx| {
        let window = WindowConfig::new("dev.csfh.odk.stories.kbd")
            .title("Kbd")
            .size(900.0, 640.0);
        odk::ui::open_window(cx, window, |_, cx| cx.new(|_| KbdStory)).unwrap();
    });
}
