//! The Image story on Rune: bytes the application holds, cover and contain in a box that is not the
//! image's shape, an avatar, a file in each format ADR 0072 names, and bytes that are not an
//! image. Run it with `tools/story rune-image`.

use std::sync::Arc;

use odk_view::{Hosted, Image, ImageFit, Layout, Space};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{caption, page, section};

const PNG: &[u8] = include_bytes!("assets/gradient.png");
const WEBP: &[u8] = include_bytes!("assets/gradient.webp");
const GIF: &[u8] = include_bytes!("../../crates/odk-view/tests/fixtures/two.gif");
const BMP: &[u8] = include_bytes!("../../crates/odk-view/tests/fixtures/two.bmp");
const JPEG: &[u8] = include_bytes!("../../crates/odk-view/tests/fixtures/two.jpg");

struct Story {
    png: Arc<[u8]>,
    webp: Arc<[u8]>,
    gif: Arc<[u8]>,
    bmp: Arc<[u8]>,
    jpeg: Arc<[u8]>,
    broken: Arc<[u8]>,
    svg: Arc<[u8]>,
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let story = &hosted.app;
    // The same `Arc` on every build, so the decoded image is kept.
    let boxed = |id: &str, name: &str, fit: ImageFit, bytes: &Arc<[u8]>| {
        col().w(200.0).h(120.0).child(
            Image::new(id, name)
                .fit(fit)
                .source(bytes.clone())
                .show(hosted),
        )
    };
    let fits = layout
        .row(Space::Md)
        .child(
            layout
                .stack(Space::Xs)
                .child(boxed("cover", "Cover", ImageFit::Cover, &story.png))
                .child(caption(hosted, "Cover: crops to fill.")),
        )
        .child(
            layout
                .stack(Space::Xs)
                .child(boxed("contain", "Contain", ImageFit::Contain, &story.png))
                .child(caption(hosted, "Contain: all of it shown.")),
        );
    let avatar = layout
        .row(Space::Md)
        .child(
            Image::avatar("me", "Your avatar")
                .source(story.png.clone())
                .show(hosted),
        )
        .child(Image::avatar("nobody", "No picture yet").show(hosted));
    let formats = [
        ("PNG", &story.png),
        ("JPEG", &story.jpeg),
        ("WebP", &story.webp),
        ("GIF", &story.gif),
        ("BMP", &story.bmp),
    ]
    .into_iter()
    .fold(layout.row(Space::Md), |row, (name, bytes)| {
        row.child(
            layout
                .stack(Space::Xs)
                .child(
                    col().w(96.0).h(60.0).child(
                        Image::new(format!("format-{name}"), name)
                            .source(bytes.clone())
                            .show(hosted),
                    ),
                )
                .child(caption(hosted, name)),
        )
    });
    let wrong = layout
        .row(Space::Md)
        .child(
            layout
                .stack(Space::Xs)
                .child(
                    col().w(96.0).h(60.0).child(
                        Image::new("svg", "An SVG, which is not drawn")
                            .source(story.svg.clone())
                            .show(hosted),
                    ),
                )
                .child(caption(hosted, "SVG")),
        )
        .child(
            layout
                .stack(Space::Xs)
                .child(
                    col().w(96.0).h(60.0).child(
                        Image::new("broken", "A PNG that is cut short")
                            .source(story.broken.clone())
                            .show(hosted),
                    ),
                )
                .child(caption(hosted, "Cut short")),
        )
        .child(
            layout
                .stack(Space::Xs)
                .child(
                    col()
                        .w(96.0)
                        .h(60.0)
                        .child(Image::new("none", "Not here yet").show(hosted)),
                )
                .child(caption(hosted, "No bytes")),
        );

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Fit",
                "A 16:9 picture in a box that is wider than it is high.",
                fits,
            ),
            section(
                hosted,
                "Avatar",
                "A circle of one size; with no bytes, a placeholder.",
                avatar,
            ),
            section(
                hosted,
                "Formats",
                "Each format ADR 0072 names; Rune decodes them off the UI thread.",
                formats,
            ),
            section(
                hosted,
                "Not an image",
                "A placeholder with a mark, and the image says it could not be shown.",
                wrong,
            ),
        ],
        [],
    )
}

fn main() {
    let mut broken = PNG[..24].to_vec();
    broken.extend_from_slice(b"not the rest of a png");
    support::run(
        Story {
            png: Arc::from(PNG),
            webp: Arc::from(WEBP),
            gif: Arc::from(GIF),
            bmp: Arc::from(BMP),
            jpeg: Arc::from(JPEG),
            broken: Arc::from(broken),
            svg: Arc::from(&b"<svg xmlns='http://www.w3.org/2000/svg'/>"[..]),
        },
        view,
    );
}
