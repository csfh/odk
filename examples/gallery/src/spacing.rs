//! The Spacing page: the foundation's 4 px scale and the spacing named for a purpose, each with
//! its value at the user's text size and a bar of that length.

use odk::gpui::px;
use odk::prelude::*;
use odk::theme::foundation::{FoundationTokens, LayoutTokens};

use crate::present::{group, kv, page, section};
use crate::registry::Page;

pub struct Step {
    pub name: &'static str,
    pub value: fn(&FoundationTokens) -> f32,
}

pub const STEPS: [Step; 9] = [
    Step {
        name: "0",
        value: |f| f.spacing.s0,
    },
    Step {
        name: "1",
        value: |f| f.spacing.s1,
    },
    Step {
        name: "2",
        value: |f| f.spacing.s2,
    },
    Step {
        name: "3",
        value: |f| f.spacing.s3,
    },
    Step {
        name: "4",
        value: |f| f.spacing.s4,
    },
    Step {
        name: "5",
        value: |f| f.spacing.s5,
    },
    Step {
        name: "6",
        value: |f| f.spacing.s6,
    },
    Step {
        name: "8",
        value: |f| f.spacing.s8,
    },
    Step {
        name: "12",
        value: |f| f.spacing.s12,
    },
];

pub type LayoutRow = (&'static str, fn(&LayoutTokens) -> f32);

pub const LAYOUT: [LayoutRow; 6] = [
    ("panel_padding", |l| l.panel_padding),
    ("dialog_padding", |l| l.dialog_padding),
    ("section_gap", |l| l.section_gap),
    ("form_gap", |l| l.form_gap),
    ("field_gap", |l| l.field_gap),
    ("menu_item_gap", |l| l.menu_item_gap),
];

pub struct SpacingPage;

/// A bar as long as `length`, in the accent's solid fill.
fn bar(length: f32) -> impl IntoElement {
    let fill = odk::ui::theme_snapshot()
        .semantic
        .treatment(
            odk::theme::semantic::Role::Accent,
            odk::theme::semantic::Emphasis::Solid,
            odk::theme::semantic::State::Default,
        )
        .background;
    div()
        .h(px(8.0))
        .w(px(length))
        .flex_shrink_0()
        .bg(odk::gpui::rgba(fill.to_rgba_u32()))
}

impl Render for SpacingPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let f = odk::ui::foundation();
        let mut scale = Stack::new().gap(Space::Sm);
        for step in &STEPS {
            let length = (step.value)(&f);
            scale = scale.child(
                Row::new()
                    .gap(Space::Md)
                    .child(
                        div()
                            .w(px(crate::layout::LABEL_WIDTH))
                            .child(Text::new(format!("space.{}  {length} px", step.name))),
                    )
                    .child(bar(length)),
            );
        }
        let mut layout = Stack::new().gap(Space::Sm);
        for (name, value) in &LAYOUT {
            let length = value(&f.layout);
            layout = layout.child(
                Row::new()
                    .gap(Space::Md)
                    .child(
                        div()
                            .w(px(crate::layout::LABEL_WIDTH))
                            .child(Text::new(format!("layout.{name}  {length} px"))),
                    )
                    .child(bar(length)),
            );
        }
        let scale_note = if f.text_scale.is_default() {
            "At the default text size (12 px).".to_owned()
        } else {
            format!(
                "At the user's text size of {} px, each step scaled and rounded on its own.",
                f.text_scale.base_size()
            )
        };
        page(
            Page::Spacing,
            Stack::new()
                .gap(Space::Lg)
                .child(section("Scale"))
                .child(Text::new(format!("A 4 px grid. {scale_note}")).muted())
                .child(scale)
                .child(section("By purpose"))
                .child(Text::new("A component asks for a dialog's padding rather than a step; each is a step of the scale, except the field gap, which is 6 px on purpose.").muted())
                .child(layout)
                .child(group("In use"))
                .child(
                    Stack::new()
                        .gap(Space::Xs)
                        .child(kv("Gap between sections of a page", format!("{} px (layout.section_gap)", f.layout.section_gap)))
                        .child(kv("A panel's padding", format!("{} px (layout.panel_padding)", f.layout.panel_padding)))
                        .child(kv("The gallery's own gaps", "`Space` steps, which read the foundation's spacing")),
                ),
        )
    }
}
