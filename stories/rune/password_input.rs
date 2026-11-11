//! The PasswordInput story on Rune: masked text, a show button, invalid, required and disabled.
//! Run it with `tools/story rune-password_input`.

use odk_view::{Hosted, Layout, PasswordInput, Space};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{caption, page, section};

#[derive(Default)]
struct Story {
    length: usize,
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let on_change = |s: &mut Story, text: String| s.length = text.chars().count();
    let said = format!("{} characters typed.", hosted.app.length);
    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Password",
                "Dots, no copy or primary selection. The eye shows the text and keeps the caret.",
                layout
                    .stack(Space::Sm)
                    .child(
                        PasswordInput::new("pw", "Password", on_change, |_: &mut Story| {})
                            .placeholder("Password")
                            .show(hosted),
                    )
                    .child(caption(hosted, &said)),
            ),
            section(
                hosted,
                "States",
                "Invalid, required, without the show button, and disabled.",
                layout
                    .stack(Space::Sm)
                    .child(
                        PasswordInput::new("bad", "Wrong password", on_change, |_: &mut Story| {})
                            .invalid(true)
                            .show(hosted),
                    )
                    .child(
                        PasswordInput::new(
                            "req",
                            "Required password",
                            on_change,
                            |_: &mut Story| {},
                        )
                        .required(true)
                        .show(hosted),
                    )
                    .child(
                        PasswordInput::new(
                            "fixed",
                            "No show button",
                            on_change,
                            |_: &mut Story| {},
                        )
                        .revealable(false)
                        .show(hosted),
                    )
                    .child(
                        PasswordInput::new(
                            "off",
                            "Disabled password",
                            on_change,
                            |_: &mut Story| {},
                        )
                        .disabled(true)
                        .show(hosted),
                    ),
            ),
        ],
        [],
    )
}

fn main() {
    support::run(Story::default(), view);
}
