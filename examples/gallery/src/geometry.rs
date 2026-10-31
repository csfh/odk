//! The Geometry page: the two corner settings at the size they are drawn, the border and focus
//! indicator, and each density's control height with real controls at all three. Every number
//! is a foundation token read inside `render`; the corners are not magnified.

use odk::gpui::{px, rgba};
use odk::prelude::*;
use odk::theme::foundation::{CornerRadius, FoundationConfig, FoundationTokens};
use odk::theme::semantic::Role;

use crate::buttons::row;
use crate::present::{group, kv, page, section, wrap};
use crate::registry::Page;

pub struct GeometryPage;

/// A box with the corners and border of `radius`, the size of a small control.
fn corner_sample(radius: f32, border: f32, label: String) -> impl IntoElement {
    let theme = odk::ui::theme_snapshot();
    let line = theme.semantic.border.strong;
    let fill = theme.semantic.surface.raised;
    Stack::new()
        .gap(Space::Xs)
        .child(
            div()
                .w(px(96.0))
                .h(px(48.0))
                .rounded(px(radius))
                .border(px(border))
                .border_color(rgba(line.to_rgba_u32()))
                .bg(rgba(fill.to_rgba_u32())),
        )
        .child(Text::new(label))
}

impl Render for GeometryPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let f = odk::ui::foundation();
        let g = &f.geometry;
        let theme = odk::ui::theme_snapshot();
        let ring = theme.semantic.role(Role::Accent).focus;

        let mut radii = wrap().gap(Space::Xl);
        for setting in [CornerRadius::Square, CornerRadius::Subtle] {
            let tokens = FoundationTokens::from_config(&FoundationConfig {
                corner_radius: setting,
                ..Default::default()
            });
            radii = radii.child(corner_sample(
                tokens.geometry.radius.default,
                g.border_width,
                format!(
                    "{:?}: {} px{}",
                    setting,
                    setting.px(),
                    if setting == g.corner_radius {
                        " (in force)"
                    } else {
                        ""
                    }
                ),
            ));
        }

        let mut radius_rows = Stack::new().gap(Space::Xs);
        for (name, value) in [
            ("radius.none", g.radius.none),
            ("radius.default", g.radius.default),
            ("radius.small", g.radius.small),
            ("radius.overlay", g.radius.overlay),
        ] {
            radius_rows = radius_rows.child(kv(name, format!("{value} px")));
        }

        let focus = div().p(px(g.focus_offset + g.focus_width)).child(
            div()
                .p(px(g.focus_offset))
                .border(px(g.focus_width))
                .border_color(rgba(ring.to_rgba_u32()))
                .rounded(px(g.radius.default + g.focus_offset))
                .child(
                    div()
                        .w(px(72.0))
                        .h(px(f.controls.metrics().height))
                        .border(px(g.border_width))
                        .border_color(rgba(theme.semantic.border.default.to_rgba_u32()))
                        .rounded(px(g.radius.default)),
                ),
        );

        let mut heights = Stack::new().gap(Space::Md);
        for density in Density::ALL {
            let m = f.controls.for_density(density);
            heights = heights.child(
                Stack::new()
                    .gap(Space::Xs)
                    .child(Text::new(format!(
                        "{}: {} px high, {} px side padding, {} px icons, {} px gap{}",
                        density.name(),
                        m.height,
                        m.padding_x,
                        m.icon_size,
                        m.gap,
                        if density == f.controls.density {
                            " (the application's)"
                        } else {
                            ""
                        }
                    )))
                    .child(
                        row()
                            .items_center()
                            .child(
                                Button::new(("geometry-button", density as usize))
                                    .label("Save changes")
                                    .variant(ButtonVariant::Primary)
                                    .density(density),
                            )
                            .child(
                                Button::new(("geometry-outline", density as usize))
                                    .leading_icon(Icon::Check)
                                    .label("Apply")
                                    .variant(ButtonVariant::Outline)
                                    .density(density),
                            )
                            .child(
                                Select::new(("geometry-select", density as usize), "Region")
                                    .item("a", "Region A")
                                    .item("b", "Region B")
                                    .default_value("a")
                                    .density(density),
                            )
                            .child(
                                ToggleGroup::new(("geometry-segments", density as usize), "View")
                                    .item("list", "List")
                                    .item("grid", "Grid")
                                    .default_value(["list"])
                                    .density(density),
                            ),
                    ),
            );
        }

        page(
            Page::Geometry,
            Stack::new()
                .gap(Space::Lg)
                .child(section("Corner radius"))
                .child(Text::new(format!(
                    "Two settings, for the whole application, chosen in the toolbar: Square is {} px and Subtle {} px (ADR 0042 made it 2 px after a drawn 0.8 px corner could not be seen). Small marks such as a check box stay square. They are drawn here at their real size.",
                    CornerRadius::Square.px(),
                    CornerRadius::Subtle.px()
                )).muted())
                .child(radii)
                .child(radius_rows)
                .child(section("Borders and focus"))
                .child(
                    Stack::new()
                        .gap(Space::Xs)
                        .child(kv("border.width", format!("{} px", g.border_width)))
                        .child(kv("focus.width", format!("{} px", g.focus_width)))
                        .child(kv("focus.offset", format!("{} px of the surface between a control and its ring", g.focus_offset)))
                        .child(kv("thumb.gap", format!("{} px between a track and its thumb", g.thumb_gap))),
                )
                .child(Text::new("A token sample of the accent's focus ring around a control's edge. Tab to a real control on any page to see the real one.").muted())
                .child(Row::new().child(focus))
                .child(section("Control heights"))
                .child(Text::new(format!(
                    "Height at each density, at the user's text size: {}. Real controls, one row per density.",
                    crate::buttons::heights()
                )).muted())
                .child(group("Comparison"))
                .child(heights),
        )
    }
}
