//! The window ODK draws once GPUI has left (ADR 0089).
//!
//! This crate is the port's host. `odk` does not depend on it, and `odk-ui` still draws with
//! GPUI, until the port's last step. Nothing here links GPUI. A component's own state lives in
//! [`Store`]: an open list, a caret and a held key.

mod alert;
mod badge;
mod breadcrumb;
mod button;
mod chart;
mod checkbox;
mod command;
mod data_table;
mod describe;
mod dialog;
mod empty;
mod face;
mod files;
mod follow;
mod font;
mod form_field;
mod host;
mod hover;
mod hover_card;
mod icon;
mod image;
mod images;
mod ink;
mod input;
mod input_group;
mod item;
mod item_group;
mod kbd;
mod layout;
mod menu;
mod nav_list;
mod number;
mod number_input;
mod password_input;
mod placement;
mod popover;
mod post;
mod progress;
mod radio;
mod resizable;
mod search_input;
mod select;
mod separator;
mod sheet;
mod skeleton;
mod spinner;
mod status;
mod store;
mod style;
mod switch;
mod tabs;
mod text;
mod textarea;
mod toast;
mod toggle;
mod toggle_group;
mod tone;
mod tooltip;

pub use alert::Alert;
pub use badge::Badge;
pub use breadcrumb::Breadcrumb;
pub use button::Button;
pub use chart::{Chart, ChartHeight, ChartPath, Ink, Scene, StrokeWidth, TextAnchor, TextBaseline};
pub use checkbox::Checkbox;
pub use command::{Command, CommandItem};
pub use data_table::{Column, DataTable, SortDirection, SortKey};
pub use dialog::Dialog;
pub use empty::Empty;
pub use face::Face;
pub use files::{FileChooser, FileChooserError, StartFolder};
pub use follow::{ColourOptionsError, Follower, Update, follow, reload_theme, set_colour_options};
pub use form_field::{FieldControl, Fielded, FormField, form};
pub use host::{
    Hosted, WindowConfig, configure, end_theme_preview, focused_component, frame, headless,
    preview_theme, run, set_reduced_motion, start,
};
pub use hover_card::HoverCard;
pub use icon::{Glyph, Icon};
pub use image::{Image, ImageFit};
pub use ink::ink;
pub use input::Input;
pub use input_group::InputGroup;
pub use item::Item;
pub use item_group::ItemGroup;
pub use kbd::Kbd;
pub use layout::{Cell, ContentColumn, Layout, Space};
pub use menu::{ContextMenu, DropdownMenu, Menu, MenuItem};
pub use nav_list::NavList;
pub use number_input::NumberInput;
pub use odk_a11y::{Accessible, Describe, Role};
pub use password_input::PasswordInput;
pub use placement::{Align, Placement};
pub use popover::Popover;
pub use post::{NoWindow, on_host};
pub use progress::Progress;
pub use radio::RadioGroup;
pub use resizable::{Resizable, ResizablePanel};
pub use search_input::SearchInput;
pub use select::Select;
pub use separator::Separator;
pub use sheet::{Sheet, Side};
pub use skeleton::Skeleton;
pub use spinner::Spinner;
pub use status::{Status, StatusIndicator};
pub use store::Store;
pub use switch::Switch;
pub use tabs::Tabs;
pub use text::{Flow, Heading, Text};
pub use textarea::Textarea;
pub use toast::{MAX_VISIBLE, Toast, ToastId, Toaster};
pub use toggle_group::ToggleGroup;
pub use tone::Tone;
pub use tooltip::Tooltip;

/// What an application imports to write a view: the host, the layout builders and the components'
/// builders it draws with most.
pub mod prelude {
    pub use crate::{Button, Heading, Hosted, Layout, Space, Text, WindowConfig, frame, run};
    pub use rune::{Cx, El};
}
