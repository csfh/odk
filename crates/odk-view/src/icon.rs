//! ODK's icons (ADR 0080, ADR 0083): a subset of Remix Icon 4.8.0, in `icons/remixicon/` and
//! compiled in.
//!
//! Rune draws an SVG icon of paths itself (`rune::icon`, a `text::Icon`), tinted with a colour,
//! and every Remix icon here is one filled path. That needs no `svg` feature, so no `resvg`.

use odk_theme::semantic::Role;
use rune::{El, Icon as Drawing};

/// An icon's name and file in `icons/remixicon/`, written once for the enum, the list and
/// the drawings.
macro_rules! icons {
    ($($(#[$doc:meta])* $name:ident => $file:literal,)*) => {
        /// One of the icons ODK ships. Each is a Remix Icon of the same look (`-line`, or `-fill`
        /// where the name says so).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Icon {
            $($(#[$doc])* $name,)*
        }

        impl Icon {
            /// Every icon, in the order they are declared.
            pub const ALL: [Self; [$(Self::$name),*].len()] = [$(Self::$name),*];

            /// The Remix Icon this is, by its file name.
            pub const fn file(self) -> &'static str {
                match self {
                    $(Self::$name => concat!($file, ".svg"),)*
                }
            }

            /// Rune's drawing of this icon: the file's own SVG, whose root Rune's parser skips.
            fn drawing(self) -> &'static Drawing {
                static DRAWINGS: [Drawing; [$(Icon::$name),*].len()] = [$(
                    Drawing {
                        name: $file,
                        view: VIEW,
                        body: include_str!(concat!(
                            "../icons/remixicon/", $file, ".svg"
                        )),
                    },
                )*];
                &DRAWINGS[self as usize]
            }
        }
    };
}

/// Remix Icon's grid: every file is `viewBox="0 0 24 24"` (a test reads each file for it).
const VIEW: f32 = 24.0;

icons! {
    /// A select's indicator: `arrow-down-s-line`.
    ChevronDown => "arrow-down-s-line",
    /// An action that goes on: `arrow-right-line`.
    ArrowRight => "arrow-right-line",
    /// A checked box or the chosen item of a list: `check-line`.
    Check => "check-line",
    /// A step down, or a box that is partly checked: `subtract-line`.
    Minus => "subtract-line",
    /// A step up: `add-line`.
    Plus => "add-line",
    /// Clear or dismiss: `close-line`.
    Close => "close-line",
    /// A search field: `search-line`.
    Search => "search-line",
    /// Show a password: `eye-line`.
    Eye => "eye-line",
    /// Hide a password: `eye-off-line`.
    EyeOff => "eye-off-line",
    /// Not a favourite: `star-line`.
    Star => "star-line",
    /// A favourite: `star-fill`.
    StarFilled => "star-fill",
    /// Information: `information-line`.
    Info => "information-line",
    /// Done or fine: `checkbox-circle-line`.
    Success => "checkbox-circle-line",
    /// Needs attention: `error-warning-line`.
    Warning => "error-warning-line",
    /// Failed: `close-circle-line`.
    Danger => "close-circle-line",
    /// An open section, or the step up beside a chevron down: `arrow-up-s-line`.
    ChevronUp => "arrow-up-s-line",
    /// A breadcrumb's separator, or what opens beside: `arrow-right-s-line`.
    ChevronRight => "arrow-right-s-line",
    /// A column sorted ascending: `arrow-up-line`.
    ArrowUp => "arrow-up-line",
    /// A column sorted descending: `arrow-down-line`.
    ArrowDown => "arrow-down-line",
    /// More of what is folded away, such as a breadcrumb's middle: `more-line`.
    More => "more-line",
    /// Settings: `settings-3-line`.
    Settings => "settings-3-line",
    /// The chosen item of a set, such as a menu's radio item, drawn small: `circle-fill`.
    Dot => "circle-fill",
    /// An image that cannot be shown: `image-line`.
    Image => "image-line",
    /// A new file: `file-add-line`.
    FileAdd => "file-add-line",
    /// Open a file: `folder-open-line`.
    FolderOpen => "folder-open-line",
    /// An inbox: `inbox-line`.
    Inbox => "inbox-line",
    /// Send, or what was sent: `send-plane-line`.
    Send => "send-plane-line",
    /// A draft: `draft-line`.
    Draft => "draft-line",
    /// Spam: `spam-2-line`.
    Spam => "spam-2-line",
    /// A list view: `list-unordered`.
    List => "list-unordered",
    /// A grid view: `layout-grid-line`.
    Grid => "layout-grid-line",
    /// A view in columns: `layout-column-line`.
    Columns => "layout-column-line",
    /// Bold text: `bold`.
    Bold => "bold",
    /// Italic text: `italic`.
    Italic => "italic",
    /// Underlined text: `underline`.
    Underline => "underline",
    /// A person, such as a profile: `user-line`.
    User => "user-line",
    /// People, such as members: `team-line`.
    Team => "team-line",
    /// Payment, such as billing: `bank-card-line`.
    BankCard => "bank-card-line",
    /// Notifications: `notification-3-line`.
    Notification => "notification-3-line",
    /// A handle or an address: `at-line`.
    At => "at-line",
    /// A percentage: `percent-line`.
    Percent => "percent-line",
}

impl Icon {
    /// The icon of a status, so it reads without its colour (ADR 0083): `Info`, `Success`,
    /// `Warning` and `Danger` for those roles. Accent and neutral have none.
    pub const fn status(role: Role) -> Option<Self> {
        match role {
            Role::Info => Some(Self::Info),
            Role::Success => Some(Self::Success),
            Role::Warning => Some(Self::Warning),
            Role::Danger => Some(Self::Danger),
            Role::Accent | Role::Neutral => None,
        }
    }

    /// What Rune draws of this icon at `px` pixels square: its coverage, row by row. A test reads
    /// it to see that Rune's parser took the path, since a path it cannot read draws nothing.
    pub fn coverage(self, px: u32) -> Vec<u8> {
        rune::text::icon_coverage(self.drawing(), px)
    }

    /// The icon in a square of `size` points, in `colour`. A mark is decoration: the control
    /// that shows it names itself, so a screen reader does not meet it (ADR 0083).
    pub fn mark(self, size: f32, colour: rune::Color) -> El {
        rune::icon(self.drawing(), size).color(colour).a11y_hidden()
    }
}

/// What a component shows where it takes an icon: one of ODK's icons, or a character, as an
/// application may still give one (ADR 0083).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Glyph {
    Icon(Icon),
    Text(String),
}

impl From<Icon> for Glyph {
    fn from(icon: Icon) -> Self {
        Self::Icon(icon)
    }
}

impl From<&str> for Glyph {
    fn from(text: &str) -> Self {
        Self::Text(text.to_string())
    }
}

impl From<String> for Glyph {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

/// The weight a character glyph is set in.
const GLYPH_WEIGHT: f32 = 400.0;

impl Glyph {
    /// The glyph in a square of `size` points, in `colour`: an icon, or a character in the
    /// monospace face, centred. It is decoration, so a screen reader does not meet it.
    pub(crate) fn slot(self, size: f32, colour: rune::Color) -> El {
        match self {
            Self::Icon(icon) => icon.mark(size, colour),
            Self::Text(text) => rune::el()
                .size(size, size)
                .center()
                .child(
                    rune::text(text)
                        .font(rune::TextStyle {
                            family: rune::Family::Mono,
                            line_height: size,
                            ..rune::TextStyle::new(size, GLYPH_WEIGHT)
                        })
                        .color(colour)
                        .a11y_hidden(),
                )
                .a11y_hidden(),
        }
    }
}
