use gpui::{ParentElement, Styled, TestAppContext, px};
use odk_a11y::{Describe, Role};
use odk_theme::component::SkeletonStyle;
use odk_theme::foundation::CornerRadius;

use super::{FIXTURES, open, publish_fixture, redraw, reduce_motion};
use crate::{Skeleton, v_flex};

/// Items 14, 16: a skeleton is a busy Progress named for what is loading.
#[test]
fn reports_what_is_loading() {
    let a = Skeleton::text(3).label("Loading messages").accessible();
    assert_eq!(
        (a.role, a.name.as_str(), a.busy),
        (Role::Progress, "Loading messages", true)
    );
}

/// A text skeleton is as high as the lines of body text it stands for, each bar as high as the
/// text's letters, the last line shorter; a block is the size it is given.
#[gpui::test]
fn it_is_the_size_of_what_it_stands_for(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| v_flex().w(px(300.0)).child(Skeleton::text(3)));
    let s = crate::drawn::last::<SkeletonStyle>("odk-skeleton");
    let all = cx.debug_bounds("odk-skeleton").unwrap();
    assert_eq!(all.size.height, px(3.0 * s.line_height));
    assert_eq!(all.size.width, px(300.0));
    let (first, last) = (
        cx.debug_bounds("odk-skeleton/line-0").unwrap(),
        cx.debug_bounds("odk-skeleton/line-2").unwrap(),
    );
    assert_eq!(first.size.height, px(s.line_height));
    assert_eq!(last.size.height, px(s.line_height));
}

#[gpui::test]
fn a_block_is_the_size_it_is_given(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| v_flex().child(Skeleton::block(120.0, 80.0)));
    let b = cx.debug_bounds("odk-skeleton").unwrap();
    assert_eq!((b.size.width, b.size.height), (px(120.0), px(80.0)));
}

/// Items 1, 20, 34: its fill is the neutral muted tint in every fixture, grey in a monochrome
/// one; its corners are the application's; it stands still, with or without reduced motion.
#[gpui::test]
fn it_draws_its_tokens_and_stands_still(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| Skeleton::line());
    for (theme, colors) in FIXTURES {
        publish_fixture(colors);
        redraw(cx);
        let s = crate::drawn::last::<SkeletonStyle>("odk-skeleton");
        assert_eq!(s, crate::components().skeleton(), "{theme}");
        if theme == "vantablack" {
            assert!(s.fill.to_oklch().c <= odk_theme::JND);
        }
    }
    for (corner, radius) in [(CornerRadius::Square, 0.0), (CornerRadius::Subtle, 2.0)] {
        cx.update(|_, cx| crate::configure(cx, |c| c.corner_radius = corner));
        redraw(cx);
        assert_eq!(
            crate::drawn::last::<SkeletonStyle>("odk-skeleton").radius,
            radius
        );
    }
    let still = crate::drawn::last::<SkeletonStyle>("odk-skeleton");
    reduce_motion();
    redraw(cx);
    assert_eq!(crate::drawn::last::<SkeletonStyle>("odk-skeleton"), still);
}
