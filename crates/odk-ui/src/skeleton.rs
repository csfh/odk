//! A skeleton, on component tokens: the shape of content still loading, in a quiet tint
//! (`docs/spec/odk-component-tokens.md`).

use gpui::{
    App, DefiniteLength, Div, InteractiveElement, IntoElement, ParentElement, RenderOnce, Styled,
    Window, div, px, relative,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::SkeletonStyle;

use crate::components;
use crate::layout::v_flex;
use crate::theme::colour;

/// How much of its column the last line of a text skeleton takes, so the block reads as a
/// paragraph rather than a box. ODK's own.
const LAST_LINE: f32 = 0.6;

/// What a skeleton stands in for.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Shape {
    /// Lines of body text, each as high as a line of text, the last shorter.
    Text(usize),
    /// A box of a given size, such as an image or an avatar.
    Block { width: f32, height: f32 },
}

/// A placeholder in the shape of content that is still loading: lines of text, or a box, in the
/// neutral tint a secondary button rests on, with the application's corners. It is exactly the
/// size of what it stands in for, so nothing moves when the content arrives. It stands still:
/// the tint says something is coming, and a `Spinner` or a `Progress` says that work goes on.
/// Its accessible name, "Loading" by default, says what is coming.
#[derive(IntoElement)]
pub struct Skeleton {
    shape: Shape,
    label: String,
}

impl Skeleton {
    /// One line of body text, as wide as its column.
    pub fn line() -> Self {
        Self::text(1)
    }

    /// `lines` lines of body text (at least one), as wide as their column, the last shorter: a
    /// paragraph, or a list of rows when given several in a column.
    pub fn text(lines: usize) -> Self {
        Self {
            shape: Shape::Text(lines.max(1)),
            label: "Loading".into(),
        }
    }

    /// A box `width` by `height` logical pixels: what an image, a chart or an avatar will fill.
    /// A size that is negative or not a number is no size: zero.
    pub fn block(width: f32, height: f32) -> Self {
        let size = |v: f32| if v.is_finite() { v.max(0.0) } else { 0.0 };
        Self {
            shape: Shape::Block {
                width: size(width),
                height: size(height),
            },
            label: "Loading".into(),
        }
    }

    /// The accessible name: what is loading.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }
}

impl Describe for Skeleton {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Progress, self.label.clone()).busy(true)
    }
}

/// One line: a bar as high as the text's letters, centred in a line of text, `width` of the
/// column.
fn line(s: &SkeletonStyle, n: usize, width: DefiniteLength) -> Div {
    div()
        .debug_selector(|| format!("odk-skeleton/line-{n}"))
        .flex()
        .items_center()
        .h(px(s.line_height))
        .child(
            div()
                .h(px(s.bar_height))
                .w(width)
                .rounded(px(s.radius))
                .bg(colour(s.fill)),
        )
}

impl RenderOnce for Skeleton {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let s = components().skeleton();
        #[cfg(test)]
        crate::drawn::record("odk-skeleton", s);
        match self.shape {
            Shape::Text(lines) => v_flex()
                .debug_selector(|| "odk-skeleton".into())
                .w_full()
                .flex_shrink_0()
                .children((0..lines).map(|n| {
                    let last = lines > 1 && n + 1 == lines;
                    line(&s, n, relative(if last { LAST_LINE } else { 1.0 }))
                })),
            Shape::Block { width, height } => div()
                .debug_selector(|| "odk-skeleton".into())
                .flex_shrink_0()
                .w(px(width))
                .h(px(height))
                .max_w_full()
                .rounded(px(s.radius))
                .bg(colour(s.fill)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_of_no_size_is_zero_not_a_layout_error() {
        for (w, h) in [(-4.0, 10.0), (f32::NAN, 10.0), (10.0, f32::INFINITY)] {
            let Shape::Block { width, height } = Skeleton::block(w, h).shape else {
                unreachable!()
            };
            assert!(width.is_finite() && width >= 0.0, "{w}");
            assert!(height.is_finite() && height >= 0.0, "{h}");
        }
    }
}
