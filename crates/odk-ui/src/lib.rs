//! ODK's UI components (ADR 0027), built on the pinned GPUI and coloured only by the tokens
//! of `odk-theme` (ADR 0030). The components are listed in `docs/spec/odk-ui-components.md`.

mod alert;
mod app;
mod badge;
mod breadcrumb;
mod button;
mod chart;
mod checkbox;
#[cfg(test)]
mod checklist;
mod command;
mod data_table;
mod decoration;
mod dialog;
#[cfg(test)]
mod drawn;
mod editor;
mod files;
mod focus;
mod form_field;
mod hover_card;
mod icon;
mod image;
mod input;
mod input_group;
mod interaction;
mod kbd;
mod layout;
mod link;
mod menu;
mod menu_list;
mod nav_list;
mod number;
mod number_input;
mod overlay;
mod password_input;
mod placement;
mod popover;
mod progress;
mod radio;
mod resizable;
mod reveal;
#[cfg(any(test, feature = "test-support"))]
pub mod samples;
mod search_input;
mod select;
mod separator;
mod sheet;
mod skeleton;
mod spinner;
mod status;
mod switch;
mod tabs;
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub mod test_support;
mod text;
mod text_layout;
mod textarea;
mod theme;
mod toast;
mod toggle;
mod toggle_group;
mod tone;
mod tooltip;
mod window;

/// The pinned GPUI itself, for code the components do not cover (ADR 0004).
pub use gpui;

pub use alert::Alert;
pub use app::{Shell, open_window, run};
pub use badge::Badge;
pub use breadcrumb::Breadcrumb;
pub use button::{Button, Chord, Size};
pub use chart::{Chart, ChartHeight, ChartPath, Ink, Scene, StrokeWidth, TextAnchor, TextBaseline};
pub use checkbox::Checkbox;
pub use command::{Command, CommandItem};
pub use data_table::{Column, DataTable, SortDirection, SortKey};
pub use decoration::{
    Applied, Autosize, Clamp, EndEllipsis, FixedWidth, FlowKind, MiddleEllipsis, SizeMode, Wrap,
};
pub use dialog::Dialog;
pub use files::{FileChooser, FileChooserError, StartFolder};
pub use focus::{FocusNext, FocusPrevious, focused_component};
pub use form_field::{FieldControl, Fielded, FormField, form};
pub use hover_card::HoverCard;
pub use icon::{Glyph, Icon};
pub use image::{Image, ImageFit};
pub use input::Input;
pub use input_group::InputGroup;
pub use kbd::Kbd;
pub use layout::{
    Space, StyledExt, content_column, h_flex, middle_ellipsis, root, scroll_y, sidebar_frame,
    truncate_end, truncate_lines, v_flex,
};
pub use menu::{ContextMenu, DropdownMenu, Menu, MenuItem};
pub use nav_list::NavList;
pub use number_input::NumberInput;
pub use odk_a11y::{Accessible, Describe, Role};
/// What a `Button` takes from the design language of ADR 0040: its variant, the emphasis that
/// overrides the variant's, and a density of its own; and what a `Badge` takes, any Role at any
/// Emphasis (the Role is `odk::theme::semantic::Role`, since `Role` here is accessibility's).
pub use odk_theme::component::{BadgeAppearance, ButtonVariant};
pub use odk_theme::foundation::Density;
pub use odk_theme::semantic::Emphasis;
pub use password_input::PasswordInput;
pub use placement::{Align, Placement};
pub use popover::Popover;
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
pub use switch::Switch;
pub use tabs::Tabs;
pub use text::{Heading, Text};
pub use textarea::Textarea;
pub use theme::{
    colour_options, components, configure, end_theme_preview, followed_theme, foundation,
    foundation_config, preview_theme, previewed_theme, reload_theme, set_colour_options,
    set_reduced_motion, theme_diagnostics, theme_snapshot,
};
pub use toast::{
    MAX_VISIBLE as MAX_VISIBLE_TOASTS, Toast, ToastId, dismiss_toast, set_toast_limit, toast,
};
pub use toggle_group::ToggleGroup;
pub use tone::Tone;
pub use tooltip::Tooltip;
pub use window::{IntoWindowOptions, WindowConfig};

/// Everything a view usually needs: GPUI's own prelude, a few core GPUI types, and ODK's
/// components.
pub mod prelude {
    pub use gpui::prelude::*;
    pub use gpui::{App, Context, Entity, SharedString, Window, WindowOptions, div};

    pub use crate::{
        Alert, Autosize, Badge, BadgeAppearance, Breadcrumb, Button, ButtonVariant, Chart,
        ChartHeight, ChartPath, Checkbox, Chord, Clamp, Column, Command, CommandItem, ContextMenu,
        DataTable, Density, Dialog, DropdownMenu, Emphasis, EndEllipsis, FileChooser, FixedWidth,
        FormField, Glyph, Heading, HoverCard, Icon, Image, ImageFit, Ink, Input, InputGroup, Kbd,
        Menu, MenuItem, MiddleEllipsis, NavList, NumberInput, PasswordInput, Popover, Progress,
        RadioGroup, Resizable, ResizablePanel, Scene, SearchInput, Select, Separator, Sheet, Side,
        Size, Skeleton, SortDirection, SortKey, Space, Spinner, StartFolder, Status,
        StatusIndicator, StrokeWidth, StyledExt, Switch, Tabs, Text, TextAnchor, TextBaseline,
        Textarea, Toast, ToggleGroup, Tone, Tooltip, WindowConfig, Wrap, content_column, form,
        h_flex, middle_ellipsis, root, scroll_y, sidebar_frame, toast, truncate_end,
        truncate_lines, v_flex,
    };
}
