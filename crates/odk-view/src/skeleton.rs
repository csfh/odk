//! A skeleton on the Rune host: the shape of content still loading, in a quiet tint
//! (`docs/spec/odk-component-tokens.md`). It stands still: the tint says something is coming.

use crate::describe::apply;
use crate::host::Hosted;
use crate::ink::ink;
use crate::style;
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::SkeletonStyle;
use rune::{El, col, el, row};

/// How much of its column the last line of a text skeleton takes, so the block reads as a
/// paragraph rather than a box. ODK's own.
const LAST_LINE: f32 = 0.6;

/// What a skeleton stands in for.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Shape {
    /// Lines of body text, each as high as a line of text, the last shorter.
    Text(usize),
    /// A box of a given size, such as an image or an avatar, in logical pixels.
    Block { width: f32, height: f32 },
    /// A box where a control will be.
    Control,
}

/// A placeholder in the shape of content that is still loading: lines of text, or a box, in the
/// neutral tint a secondary button rests on, with the application's corners. Its accessible
/// name, "Loading" by default, says what is coming.
pub struct Skeleton {
    shape: Shape,
    label: String,
}

impl Skeleton {
    /// One line of body text, as wide as its column.
    pub fn line() -> Self {
        Self::text(1)
    }

    /// `lines` lines of body text (at least one), as wide as their column, the last shorter.
    pub fn text(lines: usize) -> Self {
        Self::of(Shape::Text(lines.max(1)))
    }

    /// In place of a control while what it shows is read: as high as a control at the
    /// application's density and three times as wide.
    pub fn control() -> Self {
        Self::of(Shape::Control)
    }

    /// A box `width` by `height` logical pixels, which do not follow the text size: what an
    /// image, a chart or an avatar will fill. A size that is negative or not a number is zero.
    pub fn block(width: f32, height: f32) -> Self {
        let size = |v: f32| if v.is_finite() { v.max(0.0) } else { 0.0 };
        Self::of(Shape::Block {
            width: size(width),
            height: size(height),
        })
    }

    fn of(shape: Shape) -> Self {
        Self {
            shape,
            label: "Loading".into(),
        }
    }

    /// The accessible name: what is loading.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn show<T>(self, hosted: &Hosted<T>) -> El {
        let accessible = self.accessible();
        let s = style::paint(hosted).tokens.skeleton();
        let shape = match self.shape {
            Shape::Text(lines) => (0..lines).fold(col().w_full(), |stack, n| {
                let last = lines > 1 && n + 1 == lines;
                stack.child(line(&s, if last { LAST_LINE } else { 1.0 }))
            }),
            Shape::Block { width, height } => block(&s, width, height),
            Shape::Control => block(&s, s.control_width, s.control_height),
        };
        apply(shape, &accessible)
    }
}

impl Describe for Skeleton {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Progress, self.label.clone()).busy(true)
    }
}

/// One line: a bar as high as the text's letters, centred in a line of text, `width` of the
/// column.
fn line(s: &SkeletonStyle, width: f32) -> El {
    row().h(s.line_height).center_y().child(
        el().h(s.bar_height)
            .w_frac(width)
            .radius(s.radius)
            .fill(ink(s.fill)),
    )
}

/// A box `width` by `height`.
fn block(s: &SkeletonStyle, width: f32, height: f32) -> El {
    el().w(width).h(height).radius(s.radius).fill(ink(s.fill))
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
