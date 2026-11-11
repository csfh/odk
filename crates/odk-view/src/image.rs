//! An image drawn from bytes the application holds, never from a path (ADR 0072), on the Rune
//! host.

use std::sync::Arc;

use crate::describe::apply;
use crate::host::Hosted;
use crate::icon::Icon;
use crate::images::Load;
use crate::ink::ink;
use crate::style;
use odk_a11y::{Accessible, Describe, Role};
use rune::{El, SurfaceFit, el};

/// How an image fills its box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageFit {
    /// Cropped to fill the whole box.
    #[default]
    Cover,
    /// All of it shown, inside the box.
    Contain,
}

/// Whether `bytes` begin as a PNG, JPEG, WebP, GIF or BMP file: the formats ADR 0072 names, which
/// the codecs `crates/deps` enables decode. Anything else, SVG included, is not drawn.
fn readable(bytes: &[u8]) -> bool {
    matches!(
        bytes,
        [0x89, b'P', b'N', b'G', ..]
            | [0xFF, 0xD8, 0xFF, ..]
            | [b'G', b'I', b'F', b'8', ..]
            | [b'B', b'M', ..]
            | [
                b'R',
                b'I',
                b'F',
                b'F',
                _,
                _,
                _,
                _,
                b'W',
                b'E',
                b'B',
                b'P',
                ..
            ]
    )
}

/// An image the application gives as the bytes of a PNG, JPEG, WebP, GIF or BMP file, which it
/// read or was sent, such as by its daemon outside the sandbox (ADR 0072). It fills the box its
/// parent gives it, cropped to fill or shown whole (`fit`), with the application's corners;
/// `Image::avatar` is the one round image, a circle of the foundation's `s12` across. Before its
/// bytes arrive, or while Rune decodes them, it is a still placeholder; bytes that are not such
/// an image, or that do not decode, draw the placeholder with a mark, and it says so. It takes no
/// focus and no keys.
///
/// Pass the same `Arc` again while the bytes have not changed: the decoded image is kept for it,
/// so a redraw neither hashes nor decodes the bytes again.
pub struct Image {
    id: String,
    name: String,
    source: Option<Arc<[u8]>>,
    fit: ImageFit,
    avatar: bool,
}

impl Image {
    /// An image named `name`, which says what it shows ("Current background"), never a file's
    /// name.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            source: None,
            fit: ImageFit::Cover,
            avatar: false,
        }
    }

    /// A circle of the foundation's `s12` across, cropped to fill: a person's picture.
    pub fn avatar(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            avatar: true,
            ..Self::new(id, name)
        }
    }

    /// The encoded image.
    pub fn source(mut self, bytes: impl Into<Arc<[u8]>>) -> Self {
        self.source = Some(bytes.into());
        self
    }

    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }

    fn unreadable(&self) -> bool {
        self.source.as_ref().is_some_and(|bytes| !readable(bytes))
    }

    pub fn show<T>(self, hosted: &Hosted<T>) -> El {
        let paint = style::paint(hosted.theme());
        let s = paint.tokens.image();
        let accessible = self.accessible();
        // An avatar's size is known, so its circle is a shape at that size.
        let (size, radius) = if self.avatar {
            (
                Some(s.avatar),
                s.avatar_shape.corner_radius(s.avatar, s.avatar),
            )
        } else {
            (None, s.radius)
        };
        let boxed = |element: El| match size {
            Some(side) => element.size(side, side),
            None => element.w_full().h_full(),
        };
        let base = boxed(apply(el().key(self.id.clone()), &accessible)).radius(radius);
        let placeholder = |marked: bool| {
            let base = base.clone().fill(ink(s.placeholder));
            if marked {
                base.center()
                    .child(Icon::Image.mark(paint.role(s.type_role).size, ink(s.mark)))
            } else {
                base
            }
        };
        let Some(bytes) = &self.source else {
            return placeholder(false);
        };
        if !readable(bytes) {
            return placeholder(true);
        }
        match hosted.ui.images.load(&self.id, bytes) {
            // Rune paints nothing of it yet; the view runs again when it settles.
            Load::Pending => placeholder(false),
            Load::Failed => placeholder(true),
            Load::Ready(image) => base
                .pixels_fit(
                    image.pixels(),
                    match self.fit {
                        ImageFit::Cover => SurfaceFit::Fill,
                        ImageFit::Contain => SurfaceFit::Fit,
                    },
                )
                .clip_rounded(radius),
        }
    }
}

impl Describe for Image {
    fn accessible(&self) -> Accessible {
        let accessible =
            Accessible::new(Role::Image, self.name.clone()).busy(self.source.is_none());
        if self.unreadable() {
            accessible.description("The image could not be shown")
        } else {
            accessible
        }
    }
}
