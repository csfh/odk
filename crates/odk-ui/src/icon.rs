//! ODK's icons (ADR 0080): a subset of Remix Icon 4.8.0, under the Apache License 2.0 in
//! `icons/remixicon/LICENSE`, embedded in the binary and drawn by GPUI as SVG in a component's
//! colour. A component that shows an icon takes a [`Glyph`]: one of these, or a character.

use std::borrow::Cow;

use gpui::{
    AnyElement, AssetSource, Div, ElementId, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, div, px, svg,
};
use odk_theme::Color;
use odk_theme::component::Interactive;
use odk_theme::semantic::{Role, Treatment};

use crate::theme::{colour, mono};

/// One of the icons ODK ships. Each is a Remix Icon of the same look (`-line`, or `-fill` where
/// the name says so); a new one is added by its file in `icons/remixicon/` and a variant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    /// A select's indicator: `arrow-down-s-line`.
    ChevronDown,
    /// An action that goes on: `arrow-right-line`.
    ArrowRight,
    /// A checked box or the chosen item of a list: `check-line`.
    Check,
    /// A step down, or a box that is partly checked: `subtract-line`.
    Minus,
    /// A step up: `add-line`.
    Plus,
    /// Clear or dismiss: `close-line`.
    Close,
    /// A search field: `search-line`.
    Search,
    /// Show a password: `eye-line`.
    Eye,
    /// Hide a password: `eye-off-line`.
    EyeOff,
    /// Not a favourite: `star-line`.
    Star,
    /// A favourite: `star-fill`.
    StarFilled,
    /// Information: `information-line`.
    Info,
    /// Done or fine: `checkbox-circle-line`.
    Success,
    /// Needs attention: `error-warning-line`.
    Warning,
    /// Failed: `close-circle-line`.
    Danger,
    /// An open section, or the step up beside a chevron down: `arrow-up-s-line`.
    ChevronUp,
    /// A breadcrumb's separator, or what opens beside: `arrow-right-s-line`.
    ChevronRight,
    /// A column sorted ascending: `arrow-up-line`.
    ArrowUp,
    /// A column sorted descending: `arrow-down-line`.
    ArrowDown,
    /// More of what is folded away, such as a breadcrumb's middle: `more-line`.
    More,
    /// Settings: `settings-3-line`.
    Settings,
    /// The chosen item of a set, such as a menu's radio item, drawn small: `circle-fill`.
    Dot,
    /// An image that cannot be shown: `image-line`.
    Image,
    /// A new file: `file-add-line`.
    FileAdd,
    /// Open a file: `folder-open-line`.
    FolderOpen,
    /// An inbox: `inbox-line`.
    Inbox,
    /// Send, or what was sent: `send-plane-line`.
    Send,
    /// A draft: `draft-line`.
    Draft,
    /// Spam: `spam-2-line`.
    Spam,
    /// A list view: `list-unordered`.
    List,
    /// A grid view: `layout-grid-line`.
    Grid,
    /// A view in columns: `layout-column-line`.
    Columns,
    /// Bold text: `bold`.
    Bold,
    /// Italic text: `italic`.
    Italic,
    /// Underlined text: `underline`.
    Underline,
    /// A person, such as a profile: `user-line`.
    User,
    /// People, such as members: `team-line`.
    Team,
    /// Payment, such as billing: `bank-card-line`.
    BankCard,
    /// Notifications: `notification-3-line`.
    Notification,
    /// A handle or an address: `at-line`.
    At,
    /// A percentage: `percent-line`.
    Percent,
}

impl Icon {
    /// The icon of a status, so it reads without its colour (ADR 0083): `Info`, `Success`,
    /// `Warning` and `Danger` for those roles. Accent and neutral have none. Alerts, toasts,
    /// marked badges and form messages draw it.
    pub const fn status(role: Role) -> Option<Self> {
        match role {
            Role::Info => Some(Self::Info),
            Role::Success => Some(Self::Success),
            Role::Warning => Some(Self::Warning),
            Role::Danger => Some(Self::Danger),
            Role::Accent | Role::Neutral => None,
        }
    }

    pub const ALL: [Self; 41] = [
        Self::ChevronDown,
        Self::ArrowRight,
        Self::Check,
        Self::Minus,
        Self::Plus,
        Self::Close,
        Self::Search,
        Self::Eye,
        Self::EyeOff,
        Self::Star,
        Self::StarFilled,
        Self::Info,
        Self::Success,
        Self::Warning,
        Self::Danger,
        Self::ChevronUp,
        Self::ChevronRight,
        Self::ArrowUp,
        Self::ArrowDown,
        Self::More,
        Self::Settings,
        Self::Dot,
        Self::Image,
        Self::FileAdd,
        Self::FolderOpen,
        Self::Inbox,
        Self::Send,
        Self::Draft,
        Self::Spam,
        Self::List,
        Self::Grid,
        Self::Columns,
        Self::Bold,
        Self::Italic,
        Self::Underline,
        Self::User,
        Self::Team,
        Self::BankCard,
        Self::Notification,
        Self::At,
        Self::Percent,
    ];

    /// The path GPUI asks [`Assets`] for.
    const fn path(self) -> &'static str {
        match self {
            Self::ChevronDown => "odk/icons/arrow-down-s-line.svg",
            Self::ArrowRight => "odk/icons/arrow-right-line.svg",
            Self::Check => "odk/icons/check-line.svg",
            Self::Minus => "odk/icons/subtract-line.svg",
            Self::Plus => "odk/icons/add-line.svg",
            Self::Close => "odk/icons/close-line.svg",
            Self::Search => "odk/icons/search-line.svg",
            Self::Eye => "odk/icons/eye-line.svg",
            Self::EyeOff => "odk/icons/eye-off-line.svg",
            Self::Star => "odk/icons/star-line.svg",
            Self::StarFilled => "odk/icons/star-fill.svg",
            Self::Info => "odk/icons/information-line.svg",
            Self::Success => "odk/icons/checkbox-circle-line.svg",
            Self::Warning => "odk/icons/error-warning-line.svg",
            Self::Danger => "odk/icons/close-circle-line.svg",
            Self::ChevronUp => "odk/icons/arrow-up-s-line.svg",
            Self::ChevronRight => "odk/icons/arrow-right-s-line.svg",
            Self::ArrowUp => "odk/icons/arrow-up-line.svg",
            Self::ArrowDown => "odk/icons/arrow-down-line.svg",
            Self::More => "odk/icons/more-line.svg",
            Self::Settings => "odk/icons/settings-3-line.svg",
            Self::Dot => "odk/icons/circle-fill.svg",
            Self::Image => "odk/icons/image-line.svg",
            Self::FileAdd => "odk/icons/file-add-line.svg",
            Self::FolderOpen => "odk/icons/folder-open-line.svg",
            Self::Inbox => "odk/icons/inbox-line.svg",
            Self::Send => "odk/icons/send-plane-line.svg",
            Self::Draft => "odk/icons/draft-line.svg",
            Self::Spam => "odk/icons/spam-2-line.svg",
            Self::List => "odk/icons/list-unordered.svg",
            Self::Grid => "odk/icons/layout-grid-line.svg",
            Self::Columns => "odk/icons/layout-column-line.svg",
            Self::Bold => "odk/icons/bold.svg",
            Self::Italic => "odk/icons/italic.svg",
            Self::Underline => "odk/icons/underline.svg",
            Self::User => "odk/icons/user-line.svg",
            Self::Team => "odk/icons/team-line.svg",
            Self::BankCard => "odk/icons/bank-card-line.svg",
            Self::Notification => "odk/icons/notification-3-line.svg",
            Self::At => "odk/icons/at-line.svg",
            Self::Percent => "odk/icons/percent-line.svg",
        }
    }

    /// The SVG, as it is in `icons/remixicon/`.
    const fn svg(self) -> &'static [u8] {
        match self {
            Self::ChevronDown => include_bytes!("../icons/remixicon/arrow-down-s-line.svg"),
            Self::ArrowRight => include_bytes!("../icons/remixicon/arrow-right-line.svg"),
            Self::Check => include_bytes!("../icons/remixicon/check-line.svg"),
            Self::Minus => include_bytes!("../icons/remixicon/subtract-line.svg"),
            Self::Plus => include_bytes!("../icons/remixicon/add-line.svg"),
            Self::Close => include_bytes!("../icons/remixicon/close-line.svg"),
            Self::Search => include_bytes!("../icons/remixicon/search-line.svg"),
            Self::Eye => include_bytes!("../icons/remixicon/eye-line.svg"),
            Self::EyeOff => include_bytes!("../icons/remixicon/eye-off-line.svg"),
            Self::Star => include_bytes!("../icons/remixicon/star-line.svg"),
            Self::StarFilled => include_bytes!("../icons/remixicon/star-fill.svg"),
            Self::Info => include_bytes!("../icons/remixicon/information-line.svg"),
            Self::Success => include_bytes!("../icons/remixicon/checkbox-circle-line.svg"),
            Self::Warning => include_bytes!("../icons/remixicon/error-warning-line.svg"),
            Self::Danger => include_bytes!("../icons/remixicon/close-circle-line.svg"),
            Self::ChevronUp => include_bytes!("../icons/remixicon/arrow-up-s-line.svg"),
            Self::ChevronRight => include_bytes!("../icons/remixicon/arrow-right-s-line.svg"),
            Self::ArrowUp => include_bytes!("../icons/remixicon/arrow-up-line.svg"),
            Self::ArrowDown => include_bytes!("../icons/remixicon/arrow-down-line.svg"),
            Self::More => include_bytes!("../icons/remixicon/more-line.svg"),
            Self::Settings => include_bytes!("../icons/remixicon/settings-3-line.svg"),
            Self::Dot => include_bytes!("../icons/remixicon/circle-fill.svg"),
            Self::Image => include_bytes!("../icons/remixicon/image-line.svg"),
            Self::FileAdd => include_bytes!("../icons/remixicon/file-add-line.svg"),
            Self::FolderOpen => include_bytes!("../icons/remixicon/folder-open-line.svg"),
            Self::Inbox => include_bytes!("../icons/remixicon/inbox-line.svg"),
            Self::Send => include_bytes!("../icons/remixicon/send-plane-line.svg"),
            Self::Draft => include_bytes!("../icons/remixicon/draft-line.svg"),
            Self::Spam => include_bytes!("../icons/remixicon/spam-2-line.svg"),
            Self::List => include_bytes!("../icons/remixicon/list-unordered.svg"),
            Self::Grid => include_bytes!("../icons/remixicon/layout-grid-line.svg"),
            Self::Columns => include_bytes!("../icons/remixicon/layout-column-line.svg"),
            Self::Bold => include_bytes!("../icons/remixicon/bold.svg"),
            Self::Italic => include_bytes!("../icons/remixicon/italic.svg"),
            Self::Underline => include_bytes!("../icons/remixicon/underline.svg"),
            Self::User => include_bytes!("../icons/remixicon/user-line.svg"),
            Self::Team => include_bytes!("../icons/remixicon/team-line.svg"),
            Self::BankCard => include_bytes!("../icons/remixicon/bank-card-line.svg"),
            Self::Notification => include_bytes!("../icons/remixicon/notification-3-line.svg"),
            Self::At => include_bytes!("../icons/remixicon/at-line.svg"),
            Self::Percent => include_bytes!("../icons/remixicon/percent-line.svg"),
        }
    }
}

/// What a component shows where it takes an icon: one of ODK's [`Icon`]s, or a character, as
/// every icon was before ADR 0080 and as an application may still give one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Glyph {
    Icon(Icon),
    Text(SharedString),
}

impl From<Icon> for Glyph {
    fn from(icon: Icon) -> Self {
        Self::Icon(icon)
    }
}

impl From<&'static str> for Glyph {
    fn from(text: &'static str) -> Self {
        Self::Text(text.into())
    }
}

impl From<String> for Glyph {
    fn from(text: String) -> Self {
        Self::Text(text.into())
    }
}

impl From<SharedString> for Glyph {
    fn from(text: SharedString) -> Self {
        Self::Text(text)
    }
}

/// The icons as GPUI's asset source, which `run` gives the application. Nothing else is an
/// asset: an `Image` draws bytes the application holds (ADR 0072).
pub(crate) struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(Icon::ALL
            .into_iter()
            .find(|icon| icon.path() == path)
            .map(|icon| Cow::Borrowed(icon.svg())))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(Icon::ALL
            .into_iter()
            .filter(|icon| icon.path().starts_with(path))
            .map(|icon| icon.path().into())
            .collect())
    }
}

/// The colours of an icon: at rest, under the pointer and pressed, the text colours a character
/// in its place would take.
pub(crate) type Inks = [Color; 3];

/// The text colours of a control in each pointer state, which its icon takes.
pub(crate) fn inks_of(colours: &Interactive<Treatment>) -> Inks {
    [colours.default, colours.hover, colours.pressed].map(|t| t.foreground)
}

/// `glyph` in a square of `size`: a character in the monospace face and the text colour around
/// it, which follows its control under the pointer; or one of ODK's icons in `inks`, following
/// `pointer`'s group as [`drawn`] does.
pub(crate) fn slot(
    glyph: Glyph,
    size: f32,
    inks: Inks,
    pointer: Option<(SharedString, ElementId)>,
) -> Div {
    let el = div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(size));
    match glyph {
        Glyph::Text(text) => el
            .font_family(mono())
            .text_size(px(size))
            .line_height(px(size))
            .child(text),
        Glyph::Icon(icon) => el.child(drawn(icon, size, inks, pointer)),
    }
}

/// An icon in one colour, for a mark that does not follow the pointer: a select's indicator, a
/// menu's check, a field's icon.
pub(crate) fn still(icon: Icon, size: f32, ink: Color) -> AnyElement {
    drawn(icon, size, [ink; 3], None)
}

/// `icon` in a square of `size`, rounded to whole pixels, in `inks`. With `pointer`, the GPUI
/// group whose hover and press it follows and an id of its own, it takes the hover and pressed
/// inks there; without, or when they equal the resting one, it listens to nothing.
pub(crate) fn drawn(
    icon: Icon,
    size: f32,
    inks: Inks,
    pointer: Option<(SharedString, ElementId)>,
) -> AnyElement {
    #[cfg(test)]
    crate::drawn::record_icon(icon, inks);
    let [rest, hover, pressed] = inks;
    let el = svg()
        .path(icon.path())
        .size(px(size.round()))
        .flex_none()
        .text_color(colour(rest));
    match pointer {
        Some((group, id)) if hover != rest || pressed != rest => {
            let (hover, pressed) = (colour(hover), colour(pressed));
            el.id(id)
                .group_hover(group.clone(), move |s| s.text_color(hover))
                .group_active(group, move |s| s.text_color(pressed))
                .into_any_element()
        }
        _ => el.into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every icon is served at its path, as the SVG in `icons/remixicon/`, on Remix Icon's
    /// 24-unit grid in the colour it is drawn in.
    #[test]
    fn every_icon_loads_through_the_asset_source() {
        for icon in Icon::ALL {
            let bytes = Assets.load(icon.path()).unwrap().expect("served");
            let text = std::str::from_utf8(&bytes).unwrap();
            assert!(text.starts_with("<svg"), "{icon:?}");
            assert!(text.contains("viewBox=\"0 0 24 24\""), "{icon:?}");
            assert!(text.contains("fill=\"currentColor\""), "{icon:?}");
        }
        assert!(Assets.load("odk/icons/missing.svg").unwrap().is_none());
        assert_eq!(Assets.list("odk/icons/").unwrap().len(), Icon::ALL.len());
    }

    #[test]
    fn each_icon_has_a_path_of_its_own() {
        let mut paths: Vec<_> = Icon::ALL.iter().map(|i| i.path()).collect();
        paths.sort_unstable();
        paths.dedup();
        assert_eq!(paths.len(), Icon::ALL.len());
    }
}
