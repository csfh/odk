//! Component geometry, derived from the foundation tokens and nothing else
//! (`docs/spec/odk-component-tokens.md`, "Geometry"). No length here is a component's own: each
//! is a foundation token or follows from them.

use crate::Color;
use crate::foundation::{Density, FoundationTokens, TypeRole};

/// The outline of a part. A rectangle takes the application's corner radius, which is never
/// more than 2 px (ADR 0042). A part whose roundness says what it is, a switch's track and its thumb, is
/// a `Pill` or a `Circle`: a shape of its own, not a larger radius
/// (`docs/spec/odk-foundation-tokens.md`, "Geometry").
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    Rect {
        radius: f32,
    },
    /// A rectangle whose short ends are half circles.
    Pill,
    Circle,
}

impl Shape {
    /// The corner radius to draw a `width` by `height` box of this shape with: the rectangle's
    /// own, or half the shorter side.
    pub fn corner_radius(self, width: f32, height: f32) -> f32 {
        match self {
            Self::Rect { radius } => radius,
            Self::Pill | Self::Circle => width.min(height) / 2.0,
        }
    }
}

/// The box of one control at one density: a button, a field, a select's box, a menu item, a tab,
/// a segment, a badge, a key.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlGeometry {
    /// The outer height, borders included.
    pub height: f32,
    pub padding_x: f32,
    /// What is left of the height once the line of text and the borders are taken, split
    /// evenly, so the text sits in the middle at every density.
    pub padding_y: f32,
    /// Between an icon and the label, or a leading or trailing icon and a field's text.
    pub gap: f32,
    pub icon_size: f32,
    /// One line of the control's text.
    pub line_height: f32,
    pub radius: f32,
    pub border_width: f32,
    pub type_role: TypeRole,
}

impl ControlGeometry {
    pub fn shape(&self) -> Shape {
        Shape::Rect {
            radius: self.radius,
        }
    }

    /// The height inside the borders and the padding: one line of text.
    pub fn content_height(&self) -> f32 {
        self.height - 2.0 * (self.padding_y + self.border_width)
    }

    /// The height of a box `rows` lines high with this padding and border, as a textarea is.
    /// One row is the control's own height.
    pub fn rows_height(&self, rows: usize) -> f32 {
        rows.max(1) as f32 * self.line_height + 2.0 * (self.padding_y + self.border_width)
    }
}

/// The frame of a surface that holds content rather than being a control: menus, popovers,
/// tooltips, dialogs, sheets, toasts and alerts. It follows layout spacing, not density.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceGeometry {
    pub padding_x: f32,
    pub padding_y: f32,
    /// Between the parts inside it: title, body, actions.
    pub gap: f32,
    pub radius: f32,
    pub border_width: f32,
    pub title: TypeRole,
    pub body: TypeRole,
    /// Its mark or icon, at the application's density.
    pub icon_size: f32,
}

/// A check box at one density.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CheckGeometry {
    /// The box is square: this wide and this high, borders included.
    pub size: f32,
    /// The tick or dash inside it: the box less its borders.
    pub mark: f32,
    pub border_width: f32,
    /// A rectangle with the application's corner radius, like every control.
    pub shape: Shape,
    /// Between the box and its label.
    pub gap: f32,
    pub label: TypeRole,
}

/// A radio at one density: a circle as large as a check box, and the dot of the chosen one,
/// inset from the inside of the border by the thumb gap, as a switch's thumb is in its track.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadioGeometry {
    /// The circle's diameter, borders included.
    pub size: f32,
    pub dot: f32,
    pub border_width: f32,
    pub shape: Shape,
    /// Between the circle and its label.
    pub gap: f32,
    pub label: TypeRole,
}

/// A switch at one density. The thumb moves `travel` from one end of the track to the other.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwitchGeometry {
    pub width: f32,
    pub height: f32,
    /// The thumb's diameter.
    pub thumb: f32,
    /// From the track's outer edge to the thumb: the border and the thumb gap.
    pub inset: f32,
    pub travel: f32,
    pub border_width: f32,
    pub track: Shape,
    pub thumb_shape: Shape,
    /// Between the switch and its label.
    pub gap: f32,
    pub label: TypeRole,
}

/// The keyboard focus indicator: a ring of `colour`, `width` wide, drawn `offset` outside the
/// control, so a gap of the surface always lies between it and the control's own fill. It is
/// drawn over any state, together with hover or press, and takes no space in the layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FocusRing {
    pub colour: Color,
    pub width: f32,
    pub offset: f32,
}

/// A control box: the density's height, padding and icon size, and text in `role` centred in
/// the height.
pub(crate) fn control(f: &FoundationTokens, density: Density, role: TypeRole) -> ControlGeometry {
    let m = f.controls.for_density(density);
    let line_height = f.typography.style(role).line_height;
    let border_width = f.geometry.border_width;
    ControlGeometry {
        height: m.height,
        padding_x: m.padding_x,
        padding_y: centred(m.height, line_height, border_width),
        gap: m.gap,
        icon_size: m.icon_size,
        line_height,
        radius: f.geometry.radius.default,
        border_width,
        type_role: role,
    }
}

/// The same box with no border of its own, for an item drawn inside another's: a menu item, a
/// tab, a segment.
pub(crate) fn item(f: &FoundationTokens, density: Density, height: f32) -> ControlGeometry {
    let line_height = f.typography.style(TypeRole::Body).line_height;
    ControlGeometry {
        height,
        padding_y: centred(height, line_height, 0.0),
        border_width: 0.0,
        radius: f.geometry.radius.none,
        ..control(f, density, TypeRole::Body)
    }
}

/// A box as high as one line of `role` and its borders: a badge or a key, which size to their
/// content rather than to a control's height.
pub(crate) fn inline(f: &FoundationTokens, role: TypeRole, padding_x: f32) -> ControlGeometry {
    let style = f.typography.style(role);
    let border_width = f.geometry.border_width;
    ControlGeometry {
        height: style.line_height + 2.0 * border_width,
        padding_x,
        padding_y: 0.0,
        gap: f.spacing.s1,
        icon_size: style.size,
        line_height: style.line_height,
        radius: f.geometry.radius.small,
        border_width,
        type_role: role,
    }
}

pub(crate) fn surface(
    f: &FoundationTokens,
    padding_x: f32,
    padding_y: f32,
    gap: f32,
    title: TypeRole,
    body: TypeRole,
) -> SurfaceGeometry {
    SurfaceGeometry {
        padding_x,
        padding_y,
        gap,
        radius: f.geometry.radius.overlay,
        border_width: f.geometry.border_width,
        title,
        body,
        icon_size: f.controls.metrics().icon_size,
    }
}

pub(crate) fn check(f: &FoundationTokens, density: Density) -> CheckGeometry {
    let m = f.controls.for_density(density);
    let border_width = f.geometry.border_width;
    CheckGeometry {
        size: m.icon_size,
        mark: m.icon_size - 2.0 * border_width,
        border_width,
        shape: Shape::Rect {
            radius: f.geometry.radius.default,
        },
        gap: m.gap,
        label: TypeRole::Body,
    }
}

pub(crate) fn radio(f: &FoundationTokens, density: Density) -> RadioGeometry {
    let m = f.controls.for_density(density);
    let g = &f.geometry;
    RadioGeometry {
        size: m.icon_size,
        dot: m.icon_size - 2.0 * (g.border_width + g.thumb_gap),
        border_width: g.border_width,
        shape: Shape::Circle,
        gap: m.gap,
        label: TypeRole::Body,
    }
}

/// A track as high as the density's icon, a thumb that fills it less the border and the thumb
/// gap on each side, and room for the thumb to move its own diameter and one more gap.
pub(crate) fn switch(f: &FoundationTokens, density: Density) -> SwitchGeometry {
    let m = f.controls.for_density(density);
    let g = &f.geometry;
    let height = m.icon_size;
    let inset = g.border_width + g.thumb_gap;
    let thumb = height - 2.0 * inset;
    let travel = thumb + g.thumb_gap;
    SwitchGeometry {
        width: thumb + travel + 2.0 * inset,
        height,
        thumb,
        inset,
        travel,
        border_width: g.border_width,
        track: Shape::Pill,
        thumb_shape: Shape::Circle,
        gap: m.gap,
        label: TypeRole::Body,
    }
}

/// The padding above and below one line of text in a box `height` high with `border` on each
/// side.
pub(crate) fn centred(height: f32, line_height: f32, border: f32) -> f32 {
    ((height - line_height - 2.0 * border) / 2.0).max(0.0)
}
