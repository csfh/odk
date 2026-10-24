//! Menus and overlays: the menu surface that `DropdownMenu`, `ContextMenu` and a `Select`'s list
//! share, its items, popovers (`HoverCard`), tooltips, dialogs, sheets, toasts and the command
//! palette (`docs/spec/odk-component-tokens.md`). Every overlay is the same surface and edge,
//! set apart from what is below by its border rather than a shadow.

use super::catalogue::{Entry, Geometry, Measure, Summary, Template};
use super::geometry::{self, ControlGeometry, FocusRing, SurfaceGeometry};
use super::state::States;
use super::{ComponentTokens, Interactive, TreatmentRef};
use crate::Color;
use crate::foundation::{Density, FoundationTokens, TypeRole};
use crate::semantic::{
    BorderToken as B, ColourRef as C, Emphasis, Role, State, SurfaceToken as S, TextToken as T,
    Treatment,
};

/// The surface every overlay is drawn on.
const SURFACE: TreatmentRef = TreatmentRef::new(
    C::Surface(S::Overlay),
    C::Text(T::Primary),
    C::Border(B::Default),
);

/// An overlay: its surface, the text in it and, for a modal one, the scrim behind it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayStyle {
    /// Background the surface, foreground its title and body, border its edge.
    pub surface: Treatment,
    /// A description under the title.
    pub secondary: Color,
    /// `Some` for a dialog or a sheet, which dim the window behind them.
    pub backdrop: Option<Color>,
    pub geometry: SurfaceGeometry,
}

/// The surface of a menu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MenuStyle {
    pub surface: Treatment,
    pub separator: Color,
    /// A heading over a group of items: quiet text, not an item.
    pub heading: Color,
    /// The space above and below a separator.
    pub separator_margin: f32,
    pub geometry: SurfaceGeometry,
}

/// One menu item: of a `DropdownMenu`, a `ContextMenu`, a `Select`'s list or a `Command`'s
/// list. Hovered, highlighted (where the keyboard is) and selected are three things: a hover is
/// a light tint, the highlight a stronger one, and the selected item has a check mark.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MenuItemStyle {
    /// Background its fill, foreground its label; no border.
    pub colours: Interactive<Treatment>,
    pub current: Treatment,
    /// A hint at its end, such as a shortcut.
    pub hint: Interactive<Color>,
    /// The check mark of the selected item, `Some` when `states.selected`.
    pub indicator: Option<Color>,
    pub geometry: ControlGeometry,
    pub responds: bool,
}

/// A toast: an overlay with a status mark.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToastStyle {
    pub overlay: OverlayStyle,
    /// The mark in its role's colour, beside the title. It is a character as well as a colour
    /// ([`super::status_mark`]).
    pub mark: Color,
}

/// A command palette: a dialog's surface, holding a field ([`ComponentTokens::field`]) over a
/// list of menu items ([`ComponentTokens::menu_item`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommandStyle {
    pub overlay: OverlayStyle,
    /// The line shown when nothing matches.
    pub empty: Color,
}

impl ComponentTokens {
    pub fn menu(&self) -> MenuStyle {
        let f = &self.foundation;
        MenuStyle {
            surface: self.paint(SURFACE),
            separator: self.colour(C::Border(B::Subtle)),
            heading: self.colour(C::Text(T::Tertiary)),
            separator_margin: f.spacing.s1,
            geometry: menu_geometry(f),
        }
    }

    /// A menu item. `danger` is an item that destroys something, drawn in the danger role.
    pub fn menu_item(&self, density: Density, states: States, danger: bool) -> MenuItemStyle {
        let highlighted = states.highlighted;
        let (colours, current) = self.interactive(states, |s| item(danger, highlighted, s));
        let hint = Interactive::by_state(states, |s| self.colour(hint(danger, highlighted, s)));
        MenuItemStyle {
            colours,
            current,
            hint,
            indicator: states.selected.then_some(current.foreground),
            geometry: item_geometry(&self.foundation, density),
            responds: states.responds(),
        }
    }

    /// A `HoverCard`, or any popover.
    pub fn popover(&self) -> OverlayStyle {
        let f = &self.foundation;
        self.overlay(
            false,
            geometry::surface(
                f,
                f.layout.panel_padding,
                f.layout.panel_padding,
                f.spacing.s2,
                TypeRole::BodyStrong,
                TypeRole::Body,
            ),
        )
    }

    pub fn tooltip(&self) -> OverlayStyle {
        self.overlay(false, tooltip_geometry(&self.foundation))
    }

    pub fn dialog(&self) -> OverlayStyle {
        self.overlay(true, modal_geometry(&self.foundation))
    }

    /// A sheet: a dialog's surface, against one edge of the window. Its border is on the side
    /// that faces the window.
    pub fn sheet(&self) -> OverlayStyle {
        self.overlay(true, sheet_geometry(&self.foundation))
    }

    /// A toast in `role`'s colour.
    pub fn toast(&self, role: Role) -> ToastStyle {
        ToastStyle {
            overlay: self.overlay(false, toast_geometry(&self.foundation)),
            mark: self.colour(super::feedback::status_ink(role)),
        }
    }

    /// The focus ring of what opens an overlay when it is not a control with a ring of its own: a
    /// `HoverCard`'s or a `ContextMenu`'s wrapper around content, a popover's trigger.
    pub fn trigger_focus(&self) -> FocusRing {
        self.ring(C::Border(B::Focus))
    }

    pub fn command(&self) -> CommandStyle {
        CommandStyle {
            overlay: self.dialog(),
            empty: self.colour(C::Text(T::Tertiary)),
        }
    }

    fn overlay(&self, modal: bool, geometry: SurfaceGeometry) -> OverlayStyle {
        OverlayStyle {
            surface: self.paint(SURFACE),
            secondary: self.colour(C::Text(T::Secondary)),
            backdrop: modal.then(|| self.colour(C::Surface(S::Scrim))),
            geometry,
        }
    }
}

/// An item's fill and label, by precedence: disabled, pressed, highlighted, hovered. A hover is
/// the foreground's lightest tint and the highlight, where the keyboard is, the neutral muted
/// fill under hover, a step stronger; pressing a highlighted item steps once more. The theme's
/// own `selection` is not used for the highlight: in many themes it is within a JND of the
/// overlay (in Vantablack it is the overlay). A danger item uses the danger role's own subtle and
/// muted treatments, which its text was chosen for.
pub(crate) fn item(danger: bool, highlighted: bool, state: State) -> TreatmentRef {
    let role = if danger { Role::Danger } else { Role::Neutral };
    let fill = |emphasis, state| TreatmentRef::role(role, emphasis, state).with_border(C::None);
    match (state, danger, highlighted) {
        (State::Disabled, ..) => TreatmentRef::new(C::None, C::Text(T::Disabled), C::None),
        (State::Pressed, _, true) => fill(Emphasis::Muted, State::Pressed),
        (_, _, true) => fill(Emphasis::Muted, State::Hover),
        (state, true, false) => fill(Emphasis::Subtle, state),
        (State::Default, false, false) => TreatmentRef::new(C::None, C::Text(T::Primary), C::None),
        (State::Hover, false, false) => {
            TreatmentRef::new(C::Surface(S::Hover), C::Text(T::Primary), C::None)
        }
        (State::Pressed, false, false) => {
            TreatmentRef::new(C::Surface(S::Pressed), C::Text(T::Primary), C::None)
        }
    }
}

/// A hint is tertiary text, and the item's own text colour on a highlight or in a danger item.
fn hint(danger: bool, highlighted: bool, state: State) -> C {
    match (state, danger, highlighted) {
        (State::Disabled, ..) => C::Text(T::Disabled),
        (_, false, false) => C::Text(T::Tertiary),
        _ => item(danger, highlighted, state).foreground,
    }
}

fn menu_geometry(f: &FoundationTokens) -> SurfaceGeometry {
    geometry::surface(
        f,
        f.spacing.s1,
        f.spacing.s1,
        f.spacing.s0,
        TypeRole::Body,
        TypeRole::Body,
    )
}

/// A menu item is as high as a control at its density, with the menu's own padding and the gap
/// of layout spacing between its parts.
fn item_geometry(f: &FoundationTokens, density: Density) -> ControlGeometry {
    ControlGeometry {
        padding_x: f.spacing.s2,
        gap: f.layout.menu_item_gap,
        ..geometry::item(f, density, f.controls.for_density(density).height)
    }
}

fn tooltip_geometry(f: &FoundationTokens) -> SurfaceGeometry {
    geometry::surface(
        f,
        f.spacing.s2,
        f.spacing.s1,
        f.spacing.s0,
        TypeRole::Small,
        TypeRole::Small,
    )
}

fn modal_geometry(f: &FoundationTokens) -> SurfaceGeometry {
    geometry::surface(
        f,
        f.layout.dialog_padding,
        f.layout.dialog_padding,
        f.layout.form_gap,
        TypeRole::Heading,
        TypeRole::Body,
    )
}

/// A sheet is attached to a window edge, so it has no rounded corner of its own.
fn sheet_geometry(f: &FoundationTokens) -> SurfaceGeometry {
    SurfaceGeometry {
        radius: f.geometry.radius.none,
        ..modal_geometry(f)
    }
}

fn toast_geometry(f: &FoundationTokens) -> SurfaceGeometry {
    geometry::surface(
        f,
        f.layout.panel_padding,
        f.layout.panel_padding,
        f.spacing.s2,
        TypeRole::BodyStrong,
        TypeRole::Body,
    )
}

fn surface_summary(g: SurfaceGeometry) -> Summary {
    Summary {
        height: None,
        shape: super::Shape::Rect { radius: g.radius },
        type_role: Some(g.body),
    }
}

fn item_summary(f: &FoundationTokens, d: Density) -> Summary {
    let g = item_geometry(f, d);
    Summary {
        height: Some(g.height),
        shape: g.shape(),
        type_role: Some(g.type_role),
    }
}

pub(crate) fn catalogue(out: &mut Vec<Entry>) {
    use Measure::Text;
    for (name, danger, highlighted) in [
        ("normal", false, false),
        ("highlighted", false, true),
        ("danger", true, false),
        ("danger highlighted", true, true),
    ] {
        let t = Template::new("menu item", name, item_summary).in_overlay();
        let t = if danger {
            t.role(Role::Danger, None)
        } else {
            t
        };
        t.states(out, &[Text], |s| item(danger, highlighted, s));
        for state in [State::Default, State::Hover, State::Pressed] {
            let colours = item(danger, highlighted, state);
            let hint = TreatmentRef::new(
                colours.background,
                hint(danger, highlighted, state),
                C::None,
            );
            let label = match state {
                State::Default => "hint",
                State::Hover => "hint hover",
                _ => "hint pressed",
            };
            t.entry(out, label, &[Text], hint);
        }
    }

    let surfaces: [(&str, Geometry); 6] = [
        ("menu", |f, _| surface_summary(menu_geometry(f))),
        ("popover", |f, _| {
            surface_summary(geometry::surface(
                f,
                f.layout.panel_padding,
                f.layout.panel_padding,
                f.spacing.s2,
                TypeRole::BodyStrong,
                TypeRole::Body,
            ))
        }),
        ("tooltip", |f, _| surface_summary(tooltip_geometry(f))),
        ("dialog", |f, _| surface_summary(modal_geometry(f))),
        ("sheet", |f, _| surface_summary(sheet_geometry(f))),
        ("toast", |f, _| surface_summary(toast_geometry(f))),
    ];
    for (name, summary) in surfaces {
        let t = Template::new(name, "surface", summary)
            .in_overlay()
            .fixed_size();
        t.entry(out, "default", &[Text], SURFACE);
        t.entry(
            out,
            "secondary",
            &[Text],
            SURFACE
                .with_border(C::None)
                .with_foreground(C::Text(T::Secondary)),
        );
        if matches!(name, "dialog" | "sheet") {
            t.entry(
                out,
                "backdrop",
                &[],
                TreatmentRef::new(C::Surface(S::Scrim), C::None, C::None),
            );
        }
        if name == "menu" {
            t.entry(
                out,
                "heading",
                &[Text],
                TreatmentRef::new(C::Surface(S::Overlay), C::Text(T::Tertiary), C::None),
            );
            t.entry(
                out,
                "separator",
                &[],
                TreatmentRef::new(C::None, C::None, C::Border(B::Subtle)),
            );
        }
    }
    for role in Role::ALL {
        let t = Template::new("toast", "mark", |f, _| surface_summary(toast_geometry(f)))
            .role(role, None)
            .in_overlay()
            .fixed_size();
        t.entry(
            out,
            "default",
            &[Text],
            TreatmentRef::new(
                C::Surface(S::Overlay),
                super::feedback::status_ink(role),
                C::None,
            ),
        );
    }
    Template::new("command", "empty", |f, _| {
        surface_summary(modal_geometry(f))
    })
    .in_overlay()
    .fixed_size()
    .entry(
        out,
        "default",
        &[Text],
        TreatmentRef::new(C::Surface(S::Overlay), C::Text(T::Tertiary), C::None),
    );
}
