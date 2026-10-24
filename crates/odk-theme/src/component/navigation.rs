//! Navigation: tabs, segmented controls (`ToggleGroup`), links in a path (`Breadcrumb`, the link
//! a `DropdownMenu` can open from) and the items of a sidebar (`NavList`)
//! (`docs/spec/odk-component-tokens.md`).

use super::catalogue::{Entry, Measure, Summary, Template};
use super::geometry::{self, ControlGeometry, FocusRing};
use super::state::States;
use super::{ComponentTokens, Interactive, TreatmentRef};
use crate::Color;
use crate::foundation::{Density, FoundationTokens, TypeRole};
use crate::semantic::{
    BorderToken as B, ColourRef as C, Emphasis, Role, State, SurfaceToken as S, TextToken as T,
    Treatment,
};

/// How a navigation item shows it is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum NavLook {
    /// By its text and an indicator only: no fill.
    #[default]
    Text,
    /// With a fill as well: the neutral muted treatment.
    Surface,
}

/// One tab. A selected tab has an indicator, the accent's focus colour, along the list's
/// baseline, in either look.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TabStyle {
    /// Background its fill, foreground its label; no border.
    pub colours: Interactive<Treatment>,
    pub current: Treatment,
    /// `Some` for the selected tab.
    pub indicator: Option<Color>,
    /// The indicator's thickness: the focus indicator's width.
    pub indicator_width: f32,
    /// The line under the whole list.
    pub baseline: Color,
    pub focus: FocusRing,
    pub focus_visible: bool,
    pub geometry: ControlGeometry,
    pub responds: bool,
}

/// The frame of a segmented control, around its segments.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmentedStyle {
    /// Its edge; no fill.
    pub border: Color,
    pub geometry: ControlGeometry,
}

/// One segment of a segmented control. `states.selected` (or `states.checked`) is on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmentStyle {
    pub colours: Interactive<Treatment>,
    pub current: Treatment,
    pub focus: FocusRing,
    pub focus_visible: bool,
    /// As high as the frame inside its border.
    pub geometry: ControlGeometry,
    pub responds: bool,
}

/// One item of a sidebar: the tab's mapping in its surface look, standing on the sidebar's own
/// surface, so it takes the application's corner radius. `states.selected` is the current page,
/// which has the neutral muted fill and an indicator along its leading edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavItemStyle {
    /// Background its fill, foreground its label, icon and count; no border.
    pub colours: Interactive<Treatment>,
    pub current: Treatment,
    /// `Some` for the current page: the accent's focus colour, or disabled text.
    pub indicator: Option<Color>,
    /// The indicator's thickness: the focus indicator's width.
    pub indicator_width: f32,
    pub focus: FocusRing,
    pub focus_visible: bool,
    /// A control's height, padding, icon size and gap at the density, with no border.
    pub geometry: ControlGeometry,
    pub responds: bool,
}

/// What a sidebar has around its items: the headers of its groups and the space between them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavListStyle {
    pub header: Color,
    pub header_role: TypeRole,
    /// A header's inset, the items' padding, so its text lines up with theirs.
    pub header_padding_x: f32,
    /// Between a header and its first item.
    pub header_gap: f32,
    /// Between the last item of a group and the next header.
    pub group_gap: f32,
}

/// A link in a path. `states.selected` is the current page, which is not a link.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinkStyle {
    pub colours: Interactive<Treatment>,
    pub current: Treatment,
    /// Whether the label is underlined in each pointer state: a link is underlined under the
    /// pointer, where its place in the path no longer sets it apart.
    pub underline: Interactive<bool>,
    /// The mark between two links.
    pub separator: Color,
    pub gap: f32,
    pub type_role: TypeRole,
    pub focus: FocusRing,
    pub focus_visible: bool,
    pub responds: bool,
}

impl ComponentTokens {
    pub fn tab(&self, look: NavLook, density: Density, states: States) -> TabStyle {
        let (colours, current) = self.interactive(states, |s| tab(look, states.selected, s));
        let f = &self.foundation;
        TabStyle {
            colours,
            current,
            indicator: states
                .selected
                .then(|| self.colour(tab_indicator(states.disabled))),
            indicator_width: f.geometry.focus_width,
            baseline: self.colour(C::Border(B::Subtle)),
            focus: self.ring(C::Border(B::Focus)),
            focus_visible: states.shows_focus(),
            geometry: geometry::item(f, density, f.controls.for_density(density).height),
            responds: states.responds(),
        }
    }

    pub fn segmented(&self, density: Density) -> SegmentedStyle {
        SegmentedStyle {
            border: self.colour(C::Border(B::Default)),
            geometry: geometry::control(&self.foundation, density, TypeRole::Body),
        }
    }

    pub fn segment(&self, density: Density, states: States) -> SegmentStyle {
        let on = states.selected || states.checked;
        let (colours, current) = self.interactive(states, |s| segment(on, s));
        SegmentStyle {
            colours,
            current,
            focus: self.ring(C::Border(B::Focus)),
            focus_visible: states.shows_focus(),
            geometry: segment_geometry(&self.foundation, density),
            responds: states.responds(),
        }
    }

    /// An item of a sidebar (`NavList`).
    pub fn nav_item(&self, density: Density, states: States) -> NavItemStyle {
        let (colours, current) =
            self.interactive(states, |s| tab(NavLook::Surface, states.selected, s));
        let f = &self.foundation;
        NavItemStyle {
            colours,
            current,
            indicator: states
                .selected
                .then(|| self.colour(tab_indicator(states.disabled))),
            indicator_width: f.geometry.focus_width,
            focus: self.ring(C::Border(B::Focus)),
            focus_visible: states.shows_focus(),
            geometry: nav_item_geometry(f, density),
            responds: states.responds(),
        }
    }

    /// The headers and spacing of a sidebar at `density`.
    pub fn nav_list(&self, density: Density) -> NavListStyle {
        let f = &self.foundation;
        NavListStyle {
            header: self.colour(NAV_HEADER),
            header_role: TypeRole::Small,
            header_padding_x: nav_item_geometry(f, density).padding_x,
            header_gap: f.spacing.s1,
            group_gap: f.spacing.s4,
        }
    }

    /// A breadcrumb's item, or a menu that opens from a link.
    pub fn nav_link(&self, states: States) -> LinkStyle {
        let current_page = states.selected;
        let (colours, current) = self.interactive(states, |s| link(current_page, s));
        let underline = Interactive::by_state(states, |s| {
            !current_page && matches!(s, State::Hover | State::Pressed)
        });
        LinkStyle {
            colours,
            current,
            underline,
            separator: self.colour(C::Text(T::Tertiary)),
            gap: self.foundation.spacing.s2,
            type_role: TypeRole::Body,
            focus: self.ring(C::Border(B::Focus)),
            focus_visible: states.shows_focus(),
            responds: states.responds() && !current_page,
        }
    }
}

/// The fill of something selected: the neutral muted treatment as it is under the pointer, 5 JND
/// from the canvas, as a menu's highlight is, so an unselected item under the pointer
/// (`surface.hover`, 2 JND) never passes for it. Under the pointer it stays; pressed it is the
/// muted pressed fill. No edge.
fn neutral_fill(state: State) -> TreatmentRef {
    let shown = match state {
        State::Default | State::Hover => State::Hover,
        state => state,
    };
    TreatmentRef::role(Role::Neutral, Emphasis::Muted, shown).with_border(C::None)
}

/// A tab's fill and label: no fill and secondary text until hovered, primary text when
/// selected, and the selected fill as well in the surface look (a sidebar's row).
pub(crate) fn tab(look: NavLook, selected: bool, state: State) -> TreatmentRef {
    match (state, selected, look) {
        (State::Disabled, true, NavLook::Surface) => neutral_fill(State::Disabled),
        (State::Disabled, _, _) => TreatmentRef::new(C::None, C::Text(T::Disabled), C::None),
        (state, true, NavLook::Surface) => neutral_fill(state),
        (State::Default, selected, _) => TreatmentRef::new(
            C::None,
            if selected {
                C::Text(T::Primary)
            } else {
                C::Text(T::Secondary)
            },
            C::None,
        ),
        (State::Hover, ..) => TreatmentRef::new(C::Surface(S::Hover), C::Text(T::Primary), C::None),
        (State::Pressed, ..) => {
            TreatmentRef::new(C::Surface(S::Pressed), C::Text(T::Primary), C::None)
        }
    }
}

const fn tab_indicator(disabled: bool) -> C {
    if disabled {
        C::Text(T::Disabled)
    } else {
        C::Focus(Role::Accent)
    }
}

/// A segment: no fill and primary text off, the selected fill on.
pub(crate) fn segment(on: bool, state: State) -> TreatmentRef {
    match (on, state) {
        (true, State::Disabled) => TreatmentRef::new(
            neutral_fill(State::Disabled).background,
            C::Text(T::Disabled),
            C::None,
        ),
        (false, State::Disabled) => TreatmentRef::new(C::None, C::Text(T::Disabled), C::None),
        (true, state) => neutral_fill(state),
        (false, State::Default) => TreatmentRef::new(C::None, C::Text(T::Primary), C::None),
        (false, State::Hover) => {
            TreatmentRef::new(C::Surface(S::Hover), C::Text(T::Primary), C::None)
        }
        (false, State::Pressed) => {
            TreatmentRef::new(C::Surface(S::Pressed), C::Text(T::Primary), C::None)
        }
    }
}

/// A link: secondary text, primary under the pointer; the current page is primary text.
pub(crate) fn link(current_page: bool, state: State) -> TreatmentRef {
    let text = match state {
        State::Disabled => C::Text(T::Disabled),
        _ if current_page => C::Text(T::Primary),
        State::Default => C::Text(T::Secondary),
        State::Hover | State::Pressed => C::Text(T::Primary),
    };
    TreatmentRef::new(C::None, text, C::None)
}

/// A sidebar's group header: the quietest readable text.
const NAV_HEADER: C = C::Text(T::Tertiary);

fn segment_geometry(f: &FoundationTokens, density: Density) -> ControlGeometry {
    let frame = geometry::control(f, density, TypeRole::Body);
    geometry::item(f, density, frame.height - 2.0 * frame.border_width)
}

/// A control's box with no border, its text centred, and the application's corners: a sidebar's
/// item stands on its own rather than inside another's frame.
fn nav_item_geometry(f: &FoundationTokens, density: Density) -> ControlGeometry {
    let height = f.controls.for_density(density).height;
    ControlGeometry {
        radius: f.geometry.radius.default,
        ..geometry::item(f, density, height)
    }
}

fn nav_item_summary(f: &FoundationTokens, d: Density) -> Summary {
    let g = nav_item_geometry(f, d);
    Summary {
        height: Some(g.height),
        shape: g.shape(),
        type_role: Some(g.type_role),
    }
}

fn tab_summary(f: &FoundationTokens, d: Density) -> Summary {
    let g = geometry::item(f, d, f.controls.for_density(d).height);
    Summary {
        height: Some(g.height),
        shape: g.shape(),
        type_role: Some(g.type_role),
    }
}

fn segment_summary(f: &FoundationTokens, d: Density) -> Summary {
    let g = segment_geometry(f, d);
    Summary {
        height: Some(g.height),
        shape: g.shape(),
        type_role: Some(g.type_role),
    }
}

fn link_summary(f: &FoundationTokens, _: Density) -> Summary {
    Summary {
        height: None,
        shape: super::Shape::Rect {
            radius: f.geometry.radius.none,
        },
        type_role: Some(TypeRole::Body),
    }
}

pub(crate) fn catalogue(out: &mut Vec<Entry>) {
    use Measure::{Mark, Text};
    for (look, look_name) in [(NavLook::Text, "text"), (NavLook::Surface, "surface")] {
        for (selected, name) in [(false, "unselected"), (true, "selected")] {
            let t = Template::new("tab", format!("{look_name} {name}"), tab_summary);
            t.states(out, &[Text], |s| tab(look, selected, s));
            if selected {
                t.entry(
                    out,
                    "indicator",
                    &[Mark],
                    TreatmentRef::new(C::None, tab_indicator(false), C::None),
                );
            }
            t.focus(out, C::Border(B::Focus));
        }
    }
    for (selected, name) in [(false, "unselected"), (true, "current page")] {
        let t = Template::new("nav item", name, nav_item_summary);
        t.states(out, &[Text], |s| tab(NavLook::Surface, selected, s));
        if selected {
            t.entry(
                out,
                "indicator",
                &[Mark],
                TreatmentRef::new(C::None, tab_indicator(false), C::None),
            );
        }
        t.focus(out, C::Border(B::Focus));
    }
    Template::new("nav item", "header", |f, _| Summary {
        height: None,
        shape: super::Shape::Rect {
            radius: f.geometry.radius.none,
        },
        type_role: Some(TypeRole::Small),
    })
    .fixed_size()
    .entry(
        out,
        "default",
        &[Text],
        TreatmentRef::new(C::None, NAV_HEADER, C::None),
    );
    for (on, name) in [(false, "off"), (true, "on")] {
        let t = Template::new("segment", name, segment_summary);
        t.states(out, &[Text], |s| segment(on, s));
        t.focus(out, C::Border(B::Focus));
    }
    for (current_page, name) in [(false, "link"), (true, "current page")] {
        let t = Template::new("nav link", name, link_summary).fixed_size();
        t.states(out, &[Text], |s| link(current_page, s));
        if !current_page {
            t.focus(out, C::Border(B::Focus));
        }
    }
    Template::new("nav link", "separator", link_summary)
        .fixed_size()
        .entry(
            out,
            "default",
            &[Text],
            TreatmentRef::new(C::None, C::Text(T::Tertiary), C::None),
        );
}
