use gpui::{ParentElement, TestAppContext, px};
use odk_a11y::{Describe, Role};
use odk_theme::component::{Shape, StatusDotStyle};
use odk_theme::semantic::Role as Intent;

use super::{FIXTURES, open, publish_fixture, redraw};
use crate::{Status, StatusIndicator, v_flex};

const ALL: [(Status, &str); 6] = [
    (Status::Active, "Online"),
    (Status::Inactive, "Offline"),
    (Status::Success, "Synced"),
    (Status::Warning, "Degraded"),
    (Status::Error, "Failed"),
    (Status::Info, "Updating"),
];

/// Items 14, 15: a status indicator is text named by its label, which is never empty.
#[test]
fn reports_its_label() {
    for (status, label) in ALL {
        let a = StatusIndicator::new(status, label).accessible();
        assert_eq!((a.role, a.name.as_str()), (Role::Text, label));
    }
}

/// Items 1, 18, 34: each status draws its Role's dot with its label in every fixture; inactive is
/// a hollow ring, the rest filled, so on and off differ in shape; in a monochrome theme every
/// dot is grey and the label still says what it is.
#[gpui::test]
fn every_status_draws_its_dot_and_label(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex().children(ALL.map(|(status, label)| StatusIndicator::new(status, label)))
    });
    for (theme, colors) in FIXTURES {
        publish_fixture(colors);
        redraw(cx);
        let c = crate::components();
        for (status, label) in ALL {
            let s = crate::drawn::last::<StatusDotStyle>(&format!("odk-status/{label}"));
            assert_eq!(s, c.status_dot(Intent::from(status)), "{theme} {label}");
            assert_eq!(s.filled, status != Status::Inactive);
            assert_eq!(s.shape, Shape::Circle);
            if theme == "vantablack" {
                assert!(s.colour.to_oklch().c <= odk_theme::JND, "{label}");
            }
        }
    }
    let dot = cx.debug_bounds("odk-status/dot").unwrap();
    assert_eq!((dot.size.width, dot.size.height), (px(8.0), px(8.0)));
}
