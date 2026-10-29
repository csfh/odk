//! An image drawn from bytes the application holds, never from a path (ADR 0072).

use std::sync::Arc;

use gpui::{
    App, ElementId, ImageFormat, InteractiveElement, IntoElement, ObjectFit, ParentElement,
    RenderOnce, SharedString, Styled, StyledImage, Window, div, img, px,
};
use odk_a11y::{Accessible, Describe, Role};

use crate::Icon;
use crate::components;
use crate::focus::{child_id, part_name};
use crate::icon;
use crate::theme::colour;

/// How an image fills its box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageFit {
    /// Cropped to fill the whole box.
    #[default]
    Cover,
    /// All of it shown, inside the box.
    Contain,
}

/// The format of an encoded image, from its first bytes: PNG, JPEG, WebP, GIF or BMP.
fn sniff(bytes: &[u8]) -> Option<ImageFormat> {
    Some(match bytes {
        [0x89, b'P', b'N', b'G', ..] => ImageFormat::Png,
        [0xFF, 0xD8, 0xFF, ..] => ImageFormat::Jpeg,
        [
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
            ..,
        ] => ImageFormat::Webp,
        [b'G', b'I', b'F', b'8', ..] => ImageFormat::Gif,
        [b'B', b'M', ..] => ImageFormat::Bmp,
        _ => return None,
    })
}

/// An image the application gives as the bytes of a PNG, JPEG, WebP, GIF or BMP file, which it
/// read or was sent, such as by its daemon outside the sandbox (ADR 0072). It fills the box its
/// parent gives it, cropped to fill or shown whole (`fit`), with the application's corners;
/// `Image::avatar` is the one round image, a circle of the foundation's `s12` across. Before its bytes
/// arrive it is a still skeleton; bytes that are not such an image draw a placeholder with `×`,
/// and it says so. It takes no focus and no keys.
///
/// ```ignore
/// Image::avatar("me", "Your avatar").source(avatar_bytes.clone())
/// Image::new("wallpaper", "Current background").fit(ImageFit::Contain).source(bytes)
/// ```
#[derive(IntoElement)]
pub struct Image {
    id: ElementId,
    name: SharedString,
    source: Option<Arc<[u8]>>,
    fit: ImageFit,
    avatar: bool,
}

impl Image {
    /// An image named `name`, which says what it shows ("Current background"), never a file's
    /// name.
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            source: None,
            fit: ImageFit::Cover,
            avatar: false,
        }
    }

    /// A circle of the foundation's `s12` across (48 px at the default text size), cropped to
    /// fill: a person's picture.
    pub fn avatar(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self {
            avatar: true,
            ..Self::new(id, name)
        }
    }

    /// The encoded image. Pass the same `Arc` again while it has not changed: the decoded image
    /// is kept for it, so a redraw neither hashes nor decodes the bytes again.
    pub fn source(mut self, bytes: impl Into<Arc<[u8]>>) -> Self {
        self.source = Some(bytes.into());
        self
    }

    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }

    fn readable(&self) -> Option<bool> {
        self.source.as_ref().map(|b| sniff(b).is_some())
    }
}

impl Describe for Image {
    fn accessible(&self) -> Accessible {
        let a = Accessible::new(Role::Image, self.name.to_string()).busy(self.source.is_none());
        match self.readable() {
            Some(false) => a.description("The image could not be shown"),
            _ => a,
        }
    }
}

#[cfg(test)]
thread_local! {
    /// How many times bytes were made into an image on this thread, for the checklist.
    pub(crate) static DECODED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The decoded image kept for the bytes it was made from.
#[derive(Default)]
struct Decoded {
    from: Option<Arc<[u8]>>,
    image: Option<Arc<gpui::Image>>,
}

impl RenderOnce for Image {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = components().image();
        // An avatar's size is known, so its circle is a Shape at that size.
        let radius = if self.avatar {
            s.avatar_shape.corner_radius(s.avatar, s.avatar)
        } else {
            s.radius
        };
        let mut frame = div()
            .id(self.id.clone())
            .debug_selector(|| part_name(&self.id, "image"))
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .rounded(px(radius))
            .flex_shrink_0();
        frame = if self.avatar {
            frame.size(px(s.avatar))
        } else {
            frame.size_full()
        };
        #[cfg(test)]
        let drawn = crate::focus::id_path(&self.id);
        let Some(bytes) = self.source.clone() else {
            #[cfg(test)]
            crate::drawn::record(&drawn, "waiting");
            return frame.bg(colour(s.placeholder));
        };
        let Some(format) = sniff(&bytes) else {
            #[cfg(test)]
            crate::drawn::record(&drawn, "broken");
            // An image that cannot be shown: ODK's image icon (ADR 0083), so it reads as one.
            let size = crate::foundation().controls.metrics().icon_size;
            return frame
                .bg(colour(s.placeholder))
                .child(icon::still(Icon::Image, size, s.mark));
        };
        let decoded =
            window.use_keyed_state(child_id(&self.id, "decoded"), cx, |_, _| Decoded::default());
        let image = decoded.update(cx, |d, _| {
            let same = d.from.as_ref().is_some_and(|f| Arc::ptr_eq(f, &bytes));
            if !same || d.image.is_none() {
                #[cfg(test)]
                DECODED.with(|n| n.set(n.get() + 1));
                d.image = Some(Arc::new(gpui::Image::from_bytes(format, bytes.to_vec())));
                d.from = Some(bytes.clone());
            }
            d.image.clone().expect("decoded just now")
        });
        #[cfg(test)]
        crate::drawn::record(&drawn, "shown");
        let fit = match self.fit {
            ImageFit::Cover => ObjectFit::Cover,
            ImageFit::Contain => ObjectFit::Contain,
        };
        let placeholder = s.placeholder;
        frame.child(
            // GPUI clips a child to a rectangle, so the picture takes the corners itself.
            img(image)
                .size_full()
                .rounded(px(radius))
                .object_fit(fit)
                .with_fallback(move || {
                    div().size_full().bg(colour(placeholder)).into_any_element()
                }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_format_is_read_from_the_first_bytes() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n...."), Some(ImageFormat::Png));
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some(ImageFormat::Jpeg));
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some(ImageFormat::Webp));
        assert_eq!(sniff(b"GIF89a"), Some(ImageFormat::Gif));
        assert_eq!(sniff(b"BM...."), Some(ImageFormat::Bmp));
        assert_eq!(sniff(b"<svg"), None, "no vector images from bytes");
        assert_eq!(sniff(b""), None);
    }

    #[test]
    fn reports_an_image_and_whether_it_could_be_shown() {
        let waiting = Image::new("i", "Your avatar").accessible();
        assert_eq!((waiting.role, waiting.busy), (Role::Image, true));
        let broken = Image::new("i", "Your avatar")
            .source(b"not an image".to_vec())
            .accessible();
        assert!(!broken.busy);
        assert_eq!(
            broken.description.as_deref(),
            Some("The image could not be shown")
        );
        let png = Image::new("i", "x")
            .source(b"\x89PNG....".to_vec())
            .accessible();
        assert_eq!(png.description, None);
    }
}
