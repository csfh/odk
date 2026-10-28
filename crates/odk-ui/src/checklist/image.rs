use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{ParentElement, Styled, TestAppContext, px};
use odk_a11y::{Describe, Role};

use super::{open, redraw};
use crate::image::DECODED;
use crate::samples::PIXEL;
use crate::{Image, drawn, v_flex};

fn state(id: &str) -> &'static str {
    drawn::last(id)
}

/// Items 7, 23; ADR 0072: an image with no bytes yet is a skeleton and busy; bytes that are not
/// an image draw a placeholder and say so; a PNG is shown.
#[gpui::test]
fn waiting_broken_and_shown(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .child(
                v_flex()
                    .size(px(40.0))
                    .child(Image::new("waiting", "Your avatar")),
            )
            .child(
                v_flex()
                    .size(px(40.0))
                    .child(Image::new("broken", "Your avatar").source(b"not an image".to_vec())),
            )
            .child(
                v_flex()
                    .size(px(40.0))
                    .child(Image::new("shown", "A pixel").source(PIXEL.to_vec())),
            )
    });
    redraw(cx);
    assert_eq!(state("waiting"), "waiting");
    assert_eq!(state("broken"), "broken");
    assert_eq!(state("shown"), "shown");
    let shown = cx.debug_bounds("shown/image").unwrap();
    assert_eq!(
        (shown.size.width, shown.size.height),
        (px(40.0), px(40.0)),
        "fills its box"
    );
}

/// ADR 0072: an avatar is the foundation's `s12` across, a circle.
#[gpui::test]
fn an_avatar_is_s12_across(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Image::avatar("me", "Your avatar").source(PIXEL.to_vec())
    });
    let b = cx.debug_bounds("me/image").unwrap();
    let s12 = crate::foundation().spacing.s12;
    assert_eq!((b.size.width, b.size.height), (px(s12), px(s12)));
    let s = crate::components().image();
    assert_eq!(s.avatar_shape.corner_radius(s12, s12), s12 / 2.0);
}

/// Item 35; ADR 0072: the same bytes are made into an image once, however often the window is
/// drawn; new bytes are made into one again.
#[gpui::test]
fn the_same_bytes_are_decoded_once(cx: &mut TestAppContext) {
    let bytes: Rc<RefCell<Arc<[u8]>>> = Rc::new(RefCell::new(PIXEL.to_vec().into()));
    let b = bytes.clone();
    DECODED.with(|n| n.set(0));
    let cx = open(cx, move |_, _| {
        v_flex()
            .size(px(40.0))
            .child(Image::new("i", "A pixel").source(b.borrow().clone()))
    });
    redraw(cx);
    redraw(cx);
    assert_eq!(DECODED.with(|n| n.get()), 1);
    *bytes.borrow_mut() = PIXEL.to_vec().into();
    redraw(cx);
    assert_eq!(DECODED.with(|n| n.get()), 2, "new bytes, a new image");
}

/// Items 14, 15, 16.
#[test]
fn reports_an_image() {
    let a = Image::avatar("me", "Your avatar")
        .source(PIXEL.to_vec())
        .accessible();
    assert_eq!(
        (a.role, a.name.as_str(), a.busy),
        (Role::Image, "Your avatar", false)
    );
}
