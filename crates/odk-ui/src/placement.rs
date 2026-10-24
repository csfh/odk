//! Where an overlay sits against the thing it is anchored to (ADR 0037). GPUI's `anchored` puts
//! an element's corner at a point and, when it would leave the window, switches to the opposite
//! corner at the same point or shifts it back inside. That keeps one point, so an overlay flipped
//! from above its anchor to below it would start where the anchor ends only if the point is
//! chosen for it. This module chooses the point: [`Placement`] and [`Align`] say what is wanted,
//! [`Placed::wrap`] lays the overlay out so that `anchored`'s own flip lands it on the far side of
//! the anchor instead of over it, and the shifting back inside the window stays `anchored`'s.
//! Nothing here solves collisions of its own.

use gpui::{
    AnyElement, Bounds, Corner, IntoElement, ParentElement, Pixels, Point, Size, Styled, anchored,
    div, point, px,
};

use crate::layout::{h_flex, v_flex};

/// The side of its anchor an overlay prefers. It goes to the opposite side when there is no room
/// on this one, and when there is none there either it is shifted back inside the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Placement {
    Top,
    #[default]
    Bottom,
    Left,
    Right,
}

/// Which edge of the anchor the overlay lines up with across the side it is on: the start (left
/// for an overlay above or below, top for one beside) or the end. The overlay extends the other
/// way from that edge, and is shifted back inside the window if that does not fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Align {
    #[default]
    Start,
    End,
}

/// Where `anchored` puts an overlay, and what goes beside it so that it sits against the anchor
/// and not over it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placed {
    /// The point `anchored` puts [`Placed::corner`] of the overlay at.
    pub origin: Point<Pixels>,
    pub corner: Corner,
    /// The size of the empty box that stands for the anchor, in the overlay's column (above or
    /// below) or row (beside).
    pub spacer: Size<Pixels>,
    /// Whether the spacer comes before the overlay.
    pub spacer_first: bool,
    /// Whether the overlay is laid out in a row, because it is beside its anchor.
    pub beside: bool,
}

/// The placement of an overlay against `anchor`.
///
/// The overlay's column (or row) holds the overlay and a spacer as large as the anchor on the
/// anchor's side, and `anchored` puts one corner of the column on one edge of the anchor: for
/// `Bottom`, the column's top left corner at the anchor's top left, with the spacer first, so the
/// overlay starts where the anchor ends. When `anchored` switches to the opposite corner at the
/// same point, the column ends where the anchor starts and the overlay, last in it, sits above.
pub(crate) fn place(placement: Placement, align: Align, anchor: Bounds<Pixels>) -> Placed {
    let end = align == Align::End;
    match placement {
        Placement::Bottom | Placement::Top => {
            let top = placement == Placement::Top;
            // Above: the column's bottom edge at the anchor's bottom, the spacer last.
            let y = if top { anchor.bottom() } else { anchor.top() };
            let (x, corner) = match (top, end) {
                (false, false) => (anchor.left(), Corner::TopLeft),
                (false, true) => (anchor.right(), Corner::TopRight),
                (true, false) => (anchor.left(), Corner::BottomLeft),
                (true, true) => (anchor.right(), Corner::BottomRight),
            };
            Placed {
                origin: point(x, y),
                corner,
                spacer: Size {
                    width: px(0.0),
                    height: anchor.size.height,
                },
                spacer_first: !top,
                beside: false,
            }
        }
        Placement::Left | Placement::Right => {
            let left = placement == Placement::Left;
            let x = if left { anchor.right() } else { anchor.left() };
            let (y, corner) = match (left, end) {
                (false, false) => (anchor.top(), Corner::TopLeft),
                (false, true) => (anchor.bottom(), Corner::BottomLeft),
                (true, false) => (anchor.top(), Corner::TopRight),
                (true, true) => (anchor.bottom(), Corner::BottomRight),
            };
            Placed {
                origin: point(x, y),
                corner,
                spacer: Size {
                    width: anchor.size.width,
                    height: px(0.0),
                },
                spacer_first: !left,
                beside: true,
            }
        }
    }
}

impl Placed {
    /// `overlay`, kept `gap` from the anchor on whichever side it lands, as an element to hand to
    /// `lift`. The gap is padding on both sides of the overlay along the axis it is placed on,
    /// since `anchored` measures children without margins.
    pub(crate) fn wrap(self, gap: Pixels, overlay: impl IntoElement) -> AnyElement {
        let padded = if self.beside {
            div().px(gap).child(overlay).into_any_element()
        } else {
            div().py(gap).child(overlay).into_any_element()
        };
        let spacer = || {
            div()
                .w(self.spacer.width)
                .h(self.spacer.height)
                .flex_shrink_0()
        };
        let mut line = if self.beside { h_flex() } else { v_flex() };
        // Items at the start, so a column narrower than the overlay beside it does not stretch.
        line = line.items_start();
        let line = if self.spacer_first {
            line.child(spacer()).child(padded)
        } else {
            line.child(padded).child(spacer())
        };
        anchored()
            .anchor(self.corner)
            .position(self.origin)
            .child(line)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use gpui::{bounds, size};

    use super::*;

    fn anchor() -> Bounds<Pixels> {
        bounds(point(px(100.0), px(50.0)), size(px(80.0), px(30.0)))
    }

    /// Where the overlay's own box ends up, given the corner `anchored` used and the placed
    /// column's size, with the overlay `overlay` large: the spacer takes the anchor's side.
    fn overlay_box(p: Placed, overlay: Size<Pixels>, flipped: bool) -> Bounds<Pixels> {
        let (width, height) = if p.beside {
            (
                p.spacer.width + overlay.width,
                overlay.height.max(p.spacer.height),
            )
        } else {
            (
                overlay.width.max(p.spacer.width),
                p.spacer.height + overlay.height,
            )
        };
        let corner = if flipped {
            if p.beside {
                p.corner.other_side_corner_along(gpui::Axis::Horizontal)
            } else {
                p.corner.other_side_corner_along(gpui::Axis::Vertical)
            }
        } else {
            p.corner
        };
        let column = Bounds::from_corner_and_size(corner, p.origin, size(width, height));
        // Flipping moves the column, not what is in it: the overlay keeps its slot.
        let (dx, dy) = match (p.beside, p.spacer_first) {
            (false, true) => (px(0.0), p.spacer.height),
            (true, true) => (p.spacer.width, px(0.0)),
            _ => (px(0.0), px(0.0)),
        };
        Bounds {
            origin: column.origin + point(dx, dy),
            size: overlay,
        }
    }

    #[test]
    fn an_overlay_sits_against_the_side_it_prefers() {
        let a = anchor();
        let o = size(px(120.0), px(60.0));
        let below = overlay_box(place(Placement::Bottom, Align::Start, a), o, false);
        assert_eq!((below.top(), below.left()), (a.bottom(), a.left()));
        let above = overlay_box(place(Placement::Top, Align::Start, a), o, false);
        assert_eq!((above.bottom(), above.left()), (a.top(), a.left()));
        let right = overlay_box(place(Placement::Right, Align::Start, a), o, false);
        assert_eq!((right.left(), right.top()), (a.right(), a.top()));
        let left = overlay_box(place(Placement::Left, Align::Start, a), o, false);
        assert_eq!((left.right(), left.top()), (a.left(), a.top()));
    }

    #[test]
    fn flipping_puts_it_on_the_other_side_and_never_over_the_anchor() {
        let a = anchor();
        let o = size(px(120.0), px(60.0));
        for (placement, side) in [
            (Placement::Bottom, "above"),
            (Placement::Top, "below"),
            (Placement::Right, "left of"),
            (Placement::Left, "right of"),
        ] {
            for align in [Align::Start, Align::End] {
                let b = overlay_box(place(placement, align, a), o, true);
                assert!(
                    !b.intersects(&a),
                    "{placement:?}/{align:?} flipped covers the anchor: {b:?}"
                );
                let touching = match side {
                    "above" => b.bottom() == a.top(),
                    "below" => b.top() == a.bottom(),
                    "left of" => b.right() == a.left(),
                    _ => b.left() == a.right(),
                };
                assert!(
                    touching,
                    "{placement:?}/{align:?} flipped is not {side}: {b:?}"
                );
            }
        }
    }

    #[test]
    fn the_end_aligns_with_the_far_edge_of_the_anchor() {
        let a = anchor();
        let o = size(px(120.0), px(60.0));
        let below = overlay_box(place(Placement::Bottom, Align::End, a), o, false);
        assert_eq!(below.right(), a.right());
        let beside = overlay_box(place(Placement::Right, Align::End, a), o, false);
        assert_eq!(beside.bottom(), a.bottom());
    }
}
