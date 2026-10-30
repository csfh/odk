//! One representative instance of every component, for the benchmarks and for the checklist's
//! performance tests (`docs/spec/component-checklist.md`, items 35 and 36). A sample is built from
//! the public API only, as an application would, with the props that make a component do the most
//! work (a select with its items, a field with its text). Each is a function of its place among
//! the instances in a window, so that every instance has ids of its own.
//!
//! Kept beside the components so that adding a component without a sample fails a test: the
//! checklist compares [`SAMPLES`] with its own list of components.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use crate::prelude::*;
use crate::{CommandItem, MenuItem, Toast, toast};

/// A component as the benchmarks and the performance tests draw it.
#[derive(Clone, Copy)]
pub struct Sample {
    /// The component, as the checklist names it.
    pub name: &'static str,
    /// How many instances make a busy window of it.
    pub instances: usize,
    /// The `n`th instance, with ids of its own.
    pub build: fn(usize) -> AnyElement,
    /// What the sample asks of its window once opened, for a component that is not an element:
    /// a toast is raised, not placed.
    pub setup: Option<fn(&mut Window, &mut App)>,
    /// Whether it keeps asking for redraws while it is on screen, as a spinner does.
    pub animated: bool,
}

impl std::fmt::Debug for Sample {
    /// By its name, which is what a benchmark labels it with.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)
    }
}

const fn sample(name: &'static str, instances: usize, build: fn(usize) -> AnyElement) -> Sample {
    Sample {
        name,
        instances,
        build,
        setup: None,
        animated: false,
    }
}

/// A valid PNG of one pixel, for the image samples and tests.
pub const PIXEL: [u8; 70] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xa8, 0x5a, 0xf4, 0xfd,
    0x3f, 0x00, 0x06, 0xc0, 0x03, 0x13, 0xcf, 0xe3, 0x70, 0x2a, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// How large an image sample is. ODK's own.
const IMAGE_SIZE: f32 = 64.0;

/// How tall a chart sample is and how many points its line has: a sparkline. ODK's own.
const CHART_HEIGHT: f32 = 48.0;
const CHART_POINTS: usize = 60;

/// How many rows each table sample has, and how tall it is: a table draws only the rows in
/// view, so a thousand cost what twenty do (ADR 0070). ODK's own.
const TABLE_ROWS: u32 = 1000;
const TABLE_HEIGHT: f32 = 240.0;

/// How tall a resizable sample is: a group fills its parent, which gives it a height of its
/// own here, as a page's region would. ODK's own.
const RESIZABLE_HEIGHT: f32 = 160.0;

/// Every component, in the checklist's order.
pub static SAMPLES: &[Sample] = &[
    sample("Alert", 20, |n| {
        Alert::new(("alert", n), "Backup scheduled")
            .description("Runs tonight at 02:00, when the machine is idle.")
            .action(Button::new(("alert-action", n)).label("Change"))
            .on_dismiss(|_, _, _| {})
            .into_any_element()
    }),
    sample("Badge", 60, |n| {
        Badge::new(format!("Badge {n}"))
            .tone(Tone::Success)
            .icon("✓")
            .into_any_element()
    }),
    sample("Breadcrumb", 20, |n| {
        Breadcrumb::new(("breadcrumb", n))
            .item("Home", |_, _| {})
            .item("Projects", |_, _| {})
            .item("Omarchy", |_, _| {})
            .current("Settings")
            .into_any_element()
    }),
    sample("Button", 60, |n| {
        Button::new(("button", n))
            .label("Save changes")
            .variant(ButtonVariant::Primary)
            .leading_icon("✓")
            .into_any_element()
    }),
    sample("Chart", 20, |n| {
        div()
            .h(px(CHART_HEIGHT))
            .w_full()
            .child(
                Chart::new(("chart", n), "CPU over the last minute")
                    .description("Between 0 and 100%")
                    .paint(|scene, _| {
                        let (w, h) = (scene.width(), scene.height());
                        let mut path = ChartPath::new();
                        for i in 0..CHART_POINTS {
                            let x = w * i as f32 / (CHART_POINTS - 1) as f32;
                            let y = h * (0.5 + 0.4 * (i as f32 * 0.3).sin());
                            path = if i == 0 {
                                path.move_to(x, y)
                            } else {
                                path.line_to(x, y)
                            };
                        }
                        scene.stroke(&path, Ink::Series(0), StrokeWidth::Regular);
                        scene.text(
                            0.0,
                            0.0,
                            "100%",
                            Ink::TextMuted,
                            TextAnchor::Start,
                            TextBaseline::Top,
                            true,
                        );
                    }),
            )
            .into_any_element()
    }),
    sample("Checkbox", 40, |n| {
        Checkbox::new(("checkbox", n), "Wrap long lines")
            .default_checked(n % 2 == 0)
            .into_any_element()
    }),
    Sample {
        instances: 1,
        ..sample("Command", 1, |n| {
            let mut command = Command::new(("command", n), "Commands")
                .open(true)
                .placeholder("Type a command…");
            for label in ["Open", "Save", "Rename", "Close", "Quit"] {
                command = command.item(CommandItem::new(label).hint("Ctrl+O"));
            }
            command.into_any_element()
        })
    },
    sample("ContextMenu", 20, |n| {
        ContextMenu::new(
            ("context-menu", n),
            "Row actions",
            div().p_4().child(Text::new("Right-click me")),
        )
        .menu(|m| {
            m.item(MenuItem::new("Copy").hint("Ctrl+C"))
                .item(MenuItem::new("Paste").hint("Ctrl+V"))
                .separator()
                .item(MenuItem::new("Remove").danger())
        })
        .into_any_element()
    }),
    sample("DataTable", 4, |n| {
        let rows: Vec<(u32, String, f64)> = (0..TABLE_ROWS)
            .map(|i| (i, format!("process-{i}"), (i * 37 % 1000) as f64 / 10.0))
            .collect();
        div()
            .h(px(TABLE_HEIGHT))
            .w_full()
            .child(
                DataTable::new(("table", n), "Processes", rows)
                    .row_key(|r: &(u32, String, f64)| r.0.to_string().into())
                    .column(
                        Column::new("name", "Name", |r: &(u32, String, f64)| r.1.clone().into())
                            .grow(2.0)
                            .sort_by(|r: &(u32, String, f64)| r.1.clone()),
                    )
                    .column(
                        Column::new("pid", "PID", |r: &(u32, String, f64)| {
                            r.0.to_string().into()
                        })
                        .chars(7)
                        .align_end()
                        .sort_by(|r: &(u32, String, f64)| r.0),
                    )
                    .column(
                        Column::new("cpu", "CPU", |r: &(u32, String, f64)| {
                            format!("{:.1}%", r.2).into()
                        })
                        .chars(7)
                        .align_end()
                        .sort_by(|r: &(u32, String, f64)| r.2),
                    )
                    .default_sort("cpu", SortDirection::Descending)
                    .row_menu(|_, menu| menu.item(MenuItem::new("End")))
                    .on_select(|_, _, _| {}),
            )
            .into_any_element()
    }),
    Sample {
        instances: 1,
        ..sample("Dialog", 1, |n| {
            Dialog::new(("dialog", n), "Rename file")
                .open(true)
                .description("The new name is used everywhere the file is open.")
                .content(|_, _| Input::new("dialog-name", "Name").default_value("notes.md"))
                .action(|_, _| Button::new("dialog-cancel").label("Cancel"))
                .action(|_, _| {
                    Button::new("dialog-rename")
                        .label("Rename")
                        .variant(ButtonVariant::Primary)
                })
                .into_any_element()
        })
    },
    sample("DropdownMenu", 20, |n| {
        DropdownMenu::new(("dropdown", n), "File")
            .menu(|m| {
                m.item(MenuItem::new("New").hint("Ctrl+N"))
                    .item(MenuItem::new("Open…").hint("Ctrl+O"))
                    .separator()
                    .item(MenuItem::new("Quit"))
            })
            .into_any_element()
    }),
    sample("Empty", 20, |n| {
        Empty::new(format!("No matching processes {n}"))
            .icon(Icon::Search)
            .description("Nothing matches that search.")
            .action(Button::new(("empty-clear", n)).label("Clear search"))
            .into_any_element()
    }),
    sample("FormField", 20, |n| {
        FormField::new(("form-field", n), "Email")
            .help("We never share it")
            .child(Input::new(("form-field-input", n), "Email").placeholder("you@example.com"))
            .into_any_element()
    }),
    sample("Heading", 40, |n| {
        Heading::new(format!("Heading {n}"))
            .level(2)
            .into_any_element()
    }),
    sample("HoverCard", 20, |n| {
        HoverCard::new(
            ("hover-card", n),
            "About the account",
            Text::new("@aleksandra"),
        )
        .content(|_, _| Text::new("Joined in 2024"))
        .into_any_element()
    }),
    sample("Image", 20, |n| {
        div()
            .size(px(IMAGE_SIZE))
            .child(Image::new(("image", n), "A pixel").source(PIXEL.to_vec()))
            .into_any_element()
    }),
    sample("Input", 40, |n| {
        Input::new(("input", n), "Name")
            .default_value("Aleksandra Kowalska")
            .clearable(true)
            .into_any_element()
    }),
    sample("InputGroup", 20, |n| {
        InputGroup::new(("input-group", n), "Website")
            .prefix("https://")
            .suffix(".com")
            .default_value("example")
            .into_any_element()
    }),
    sample("Item", 20, |n| {
        Item::new(format!("Natural scroll {n}"))
            .description("Content moves with your fingers, the way a phone does.")
            .child(Text::new("~/.config/hypr/input.lua").mono().muted())
            .action(Switch::new(("item-switch", n), "Natural scroll"))
            .into_any_element()
    }),
    sample("ItemGroup", 10, |n| {
        ItemGroup::new(format!("Touchpad {n}"))
            .description("Feel for the trackpad.")
            .child(
                Item::new("Natural scroll")
                    .description("Content moves with your fingers.")
                    .action(Switch::new(("item-group-natural", n), "Natural scroll")),
            )
            .child(
                Item::new("Tap to click")
                    .description("A light tap is a click.")
                    .action(Switch::new(("item-group-tap", n), "Tap to click")),
            )
            .into_any_element()
    }),
    sample("Kbd", 60, |_| Kbd::new("Ctrl+Shift+K").into_any_element()),
    sample("NavList", 10, |n| {
        NavList::new(("nav-list", n), "Settings")
            .group("Account")
            .item("profile", "Profile")
            .icon("profile", "◉")
            .item("security", "Security")
            .item("billing", "Billing")
            .counter("billing", "2")
            .group("Application")
            .item("appearance", "Appearance")
            .item("keys", "Shortcuts")
            .default_value("profile")
            .into_any_element()
    }),
    sample("NumberInput", 20, |n| {
        NumberInput::new(("number", n), "Quantity")
            .min(0.0)
            .max(99.0)
            .default_value(3.0)
            .into_any_element()
    }),
    sample("PasswordInput", 20, |n| {
        PasswordInput::new(("password", n), "Password")
            .default_value("hunter2")
            .into_any_element()
    }),
    sample("Popover", 20, |n| {
        Popover::new(
            ("popover", n),
            "Filter",
            Button::new(("popover-trigger", n)).label("Filter"),
        )
        .title("Filter results")
        .content(|_, _| Text::new("Show only unread messages."))
        .into_any_element()
    }),
    sample("Progress", 40, |n| {
        Progress::new(("progress", n), "Uploading")
            .value(35.0 + (n % 5) as f64 * 10.0)
            .show_value(true)
            .into_any_element()
    }),
    sample("Resizable", 10, |n| {
        div()
            .h(px(RESIZABLE_HEIGHT))
            .w_full()
            .child(
                Resizable::horizontal(("resizable", n), "List and detail")
                    .panel(
                        ResizablePanel::new()
                            .default_size(0.35)
                            .min_size(0.2)
                            .child(Text::new("Processes")),
                    )
                    .panel(
                        ResizablePanel::new()
                            .min_size(0.3)
                            .child(Text::new("Detail")),
                    )
                    .panel(
                        ResizablePanel::new()
                            .default_size(0.2)
                            .child(Text::new("Log")),
                    )
                    .on_layout(|_, _, _| {}),
            )
            .into_any_element()
    }),
    sample("RadioGroup", 20, |n| {
        RadioGroup::new(("radio", n), "Theme")
            .item("system", "System")
            .item("light", "Light")
            .item("dark", "Dark")
            .default_value("system")
            .into_any_element()
    }),
    sample("SearchInput", 20, |n| {
        SearchInput::new(("search", n), "Search settings")
            .placeholder("Search settings…")
            .shortcut("Ctrl+K")
            .into_any_element()
    }),
    sample("Select", 20, |n| {
        (0..12)
            .fold(Select::new(("select", n), "Country"), |select, i| {
                select.item(format!("c{i}"), format!("Country {i}"))
            })
            .default_value("c3")
            .into_any_element()
    }),
    sample("Separator", 60, |_| {
        Separator::horizontal().into_any_element()
    }),
    Sample {
        instances: 1,
        ..sample("Sheet", 1, |n| {
            Sheet::new(("sheet", n), "Filters")
                .open(true)
                .content(|_, _| {
                    Stack::new()
                        .gap(Space::Md)
                        .child(Text::new("Narrow the list."))
                        .child(Switch::new("sheet-switch", "Only unread"))
                })
                .into_any_element()
        })
    },
    sample("Skeleton", 20, |_| Skeleton::text(3).into_any_element()),
    Sample {
        animated: true,
        ..sample("Spinner", 20, |n| {
            Spinner::new(("spinner", n)).into_any_element()
        })
    },
    sample("StatusIndicator", 40, |_| {
        StatusIndicator::new(Status::Active, "Online").into_any_element()
    }),
    sample("Switch", 40, |n| {
        Switch::new(("switch", n), "Wi-Fi")
            .default_checked(n % 2 == 0)
            .into_any_element()
    }),
    sample("Tabs", 10, |n| {
        Tabs::new(("tabs", n), "Settings sections")
            .tab("general", "General", |_, _| {
                Text::new("The general section.")
            })
            .tab("network", "Network", |_, _| {
                Text::new("The network section.")
            })
            .tab("sound", "Sound", |_, _| Text::new("The sound section."))
            .default_value("general")
            .into_any_element()
    }),
    sample("Text", 80, |n| {
        Text::new(format!(
            "Paragraph {n}: the quick brown fox jumps over the lazy dog, twice, to wrap."
        ))
        .into_any_element()
    }),
    sample("Textarea", 10, |n| {
        Textarea::new(("textarea", n), "Notes")
            .rows(3)
            .default_value("A line.\nAnother line.\nA third, a little longer than the others.")
            .into_any_element()
    }),
    Sample {
        instances: 0,
        setup: Some(|window, cx| {
            toast(
                window,
                cx,
                Toast::new("Saved")
                    .variant(Tone::Success)
                    .description("notes.md was saved."),
            )
        }),
        ..sample("Toast", 0, |_| div().into_any_element())
    },
    sample("ToggleGroup", 20, |n| {
        ToggleGroup::new(("toggle-group", n), "Alignment")
            .item("left", "Left")
            .item("center", "Center")
            .item("right", "Right")
            .default_value(["left"])
            .into_any_element()
    }),
    sample("Tooltip", 40, |n| {
        Tooltip::new(
            ("tooltip", n),
            "Save the file",
            Button::new(("tooltip-button", n)).label("Save"),
        )
        .into_any_element()
    }),
];
