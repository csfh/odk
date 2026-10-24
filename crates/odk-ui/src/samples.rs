//! One representative instance of every component, for the benchmarks and for the checklist's
//! performance tests (`docs/spec/component-checklist.md`, items 35 and 36). A sample is built from
//! the public API only, as an application would, with the props that make a component do the most
//! work (a select with its items, a field with its text). Each is a function of its place among
//! the instances in a window, so that every instance has ids of its own.
//!
//! Kept beside the components so that adding a component without a sample fails a test: the
//! checklist compares [`SAMPLES`] with its own list of components.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div};

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
                    v_flex()
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
