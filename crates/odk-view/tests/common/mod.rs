//! One sample of every public component, for checklist tests that run over all of them.
//!
//! A test file includes this with `mod common;`. A [`Sample`] builds the whole page for its
//! component: the component in its interesting state (open, when it has an open state), with
//! `text` as its label, title or placeholder. [`open`] runs that page on Rune's headless runtime
//! and reveals the components that open on a press or a rest of the pointer.

#![allow(
    dead_code,
    reason = "each test file includes this module and uses some of it"
)]

use std::cell::RefCell;
use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_theme::foundation::FoundationConfig;
use odk_view::{
    Alert, Align, Badge, Breadcrumb, Button, Chart, ChartHeight, Checkbox, Column, Command,
    CommandItem, ContextMenu, DataTable, Dialog, DropdownMenu, Empty, Face, FormField, Heading,
    Hosted, HoverCard, Icon, Image, Input, InputGroup, Item, ItemGroup, Kbd, Menu, MenuItem,
    NavList, NumberInput, PasswordInput, Placement, Popover, Progress, RadioGroup, Resizable,
    ResizablePanel, SearchInput, Select, Separator, Sheet, Side, Skeleton, Spinner, Status,
    StatusIndicator, Switch, Tabs, Text, Toast, ToastId, Toaster, ToggleGroup, Tone, Tooltip,
    frame, headless, ink,
};
use rune::{Cx, El, Runtime, col, row};

/// The page a sample builds, with the text it puts in its label.
pub type Build = fn(&Hosted<()>, &mut Cx<Hosted<()>>, &str) -> El;

/// A component's sample: its name as `lib.rs` exports it, whether a user can press, focus or type
/// into it, and the page that shows it.
pub struct Sample {
    pub name: &'static str,
    pub interactive: bool,
    pub build: Build,
}

/// A worst-case string for a label: long, unbroken, non-Latin or emoji.
pub struct Content {
    pub name: &'static str,
    pub text: &'static str,
}

pub const CONTENT: &[Content] = &[
    Content {
        name: "plain",
        text: "Save",
    },
    Content {
        name: "long",
        text: "A sentence of about ninety characters, written to see how a label wraps when it runs long.",
    },
    Content {
        name: "unbroken",
        text: "Unbreakable_word_with_underscores_and_no_spaces_at_all_here_ok",
    },
    Content {
        name: "japanese",
        text: "日本語の文字を含む、とても長いラベルで、枠に収まらないテキストです。",
    },
    Content {
        name: "emoji",
        text: "Family 👨‍👩‍👧‍👦 and flags 🇩🇰 🇯🇵 with text 🎉 end",
    },
];

pub fn face() -> Face {
    static FACE: OnceLock<Face> = OnceLock::new();
    *FACE.get_or_init(|| {
        let output = std::process::Command::new("fc-match")
            .args(["-f", "%{file}", "sans"])
            .output()
            .expect("fc-match");
        assert!(output.status.success(), "fc-match sans");
        let path = String::from_utf8(output.stdout).expect("fc-match speaks utf-8");
        let bytes = std::fs::read(path.trim()).expect("read the sans font");
        Face::leak(bytes, "Inter".into())
    })
}

thread_local! {
    /// The sample [`page`] builds next, set by [`open`] before the runtime starts.
    static CURRENT: RefCell<Option<(Build, String)>> = const { RefCell::new(None) };
}

/// The view every sample runs under: the current sample's page.
fn page(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let (build, text) = CURRENT.with(|current| {
        current
            .borrow()
            .clone()
            .expect("open() sets the sample first")
    });
    build(hosted, cx, &text)
}

/// Runs `runtime` for six frames, which is enough for a first draw to settle.
pub fn settle(runtime: &mut Runtime<Hosted<()>>) {
    for _ in 0..6 {
        runtime.frame(1.0 / 60.0);
    }
}

/// Opens `sample` with `text` at `size` on the emergency theme, with `config` when given, and
/// reveals the components that open on a press or a rest of the pointer.
pub fn open(
    sample: &Sample,
    text: &str,
    size: (f32, f32),
    config: Option<FoundationConfig>,
) -> Runtime<Hosted<()>> {
    open_with(sample, text, size, ThemeSnapshot::emergency(), config)
}

/// [`open`] on `theme`, a palette and a text size of the caller's choosing.
pub fn open_with(
    sample: &Sample,
    text: &str,
    size: (f32, f32),
    theme: ThemeSnapshot,
    config: Option<FoundationConfig>,
) -> Runtime<Hosted<()>> {
    CURRENT.with(|current| *current.borrow_mut() = Some((sample.build, text.to_string())));
    let mut hosted = Hosted::new((), theme, face());
    if let Some(config) = config {
        hosted = hosted.with_config(config);
    }
    let mut runtime = headless(hosted, page, size);
    settle(&mut runtime);
    reveal(&mut runtime, sample.name);
    runtime
}

/// How long the first frame of `sample` takes to build and lay out, the least of `tries` (a
/// busy machine only ever makes one slower).
pub fn first_frame(
    sample: &Sample,
    text: &str,
    size: (f32, f32),
    tries: usize,
) -> std::time::Duration {
    CURRENT.with(|current| *current.borrow_mut() = Some((sample.build, text.to_string())));
    (0..tries)
        .map(|_| {
            let hosted = Hosted::new((), ThemeSnapshot::emergency(), face());
            let started = std::time::Instant::now();
            let runtime = headless(hosted, page, size);
            let took = started.elapsed();
            drop(runtime);
            took
        })
        .min()
        .expect("at least one try")
}

/// The components whose open state a user reaches by a press or a rest of the pointer.
fn reveal(runtime: &mut Runtime<Hosted<()>>, name: &str) {
    let (key, rests) = match name {
        "Select" => ("sample-select-trigger", false),
        "Tooltip" => ("sample-tooltip", true),
        "HoverCard" => ("sample-hover-card", true),
        _ => return,
    };
    let (x, y) = runtime
        .find(key)
        .unwrap_or_else(|| panic!("the {name} sample has no {key}"));
    if rests {
        runtime.pointer_move(x, y);
        // Longer than the foundation's hover delay.
        for _ in 0..10 {
            runtime.frame(0.1);
        }
    } else {
        runtime.pointer_down(x, y);
        runtime.pointer_up(x, y);
    }
    settle(runtime);
}

// Shared handlers. A sample's state is `()`, so every handler does nothing.

fn nothing(_: &mut ()) {}

fn typed(_: &mut (), _: String) {}

fn chosen(_: &mut (), _: usize) {}

/// The page of a sample: `body` in a padded column, with `layers` drawn over it.
fn scene(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>, body: El, layers: Vec<El>) -> El {
    frame(
        hosted,
        cx,
        col()
            .key("sample")
            .w_full()
            .padding(16.0)
            .child(col().key("sample-body").w_full().child(body)),
        layers,
    )
}

fn alert(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = Alert::new("sample-alert", t)
        .variant(Tone::Warning)
        .description("Something to know, and what to do about it.")
        .action(Button::new("sample-alert-act", "Retry", nothing).show(h))
        .on_dismiss(nothing)
        .show(h);
    scene(h, cx, body, vec![])
}

fn badge(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = Badge::new(t).tone(Tone::Success).show(h);
    scene(h, cx, body, vec![])
}

fn breadcrumb(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let mut layers = Vec::new();
    let body = Breadcrumb::<()>::new("sample-breadcrumb")
        .item("Home", nothing)
        .current(t)
        .show(h, &mut layers);
    scene(h, cx, body, layers)
}

fn button(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = Button::new("sample-button", t, nothing).show(h);
    scene(h, cx, body, vec![])
}

fn chart(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = Chart::<()>::new("sample-chart", t)
        .description(t)
        .height(ChartHeight::Regular)
        .paint(|scene, _| {
            scene.stroke_rect(
                0.0,
                0.0,
                scene.width(),
                scene.height(),
                odk_view::Ink::Axis,
                odk_view::StrokeWidth::Thin,
            );
        })
        .show(h);
    scene(h, cx, body, vec![])
}

fn checkbox(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = Checkbox::new("sample-checkbox", t).show(h);
    scene(h, cx, body, vec![])
}

fn command(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let mut layers = Vec::new();
    Command::<()>::new("sample-command", t)
        .open(true)
        .on_close(nothing)
        .returns_focus_to("sample-command-open")
        .item(CommandItem::new(t).hint("Ctrl+N").on_select(nothing))
        .item(CommandItem::new("Open file").hint("Ctrl+O"))
        .show(h, &mut layers);
    let body = Text::new("The palette is open.").show(h);
    scene(h, cx, body, layers)
}

fn context_menu(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let mut layers = Vec::new();
    let menu = Menu::<()>::new()
        .item(MenuItem::new(t).icon(Icon::Draft).on_select(nothing))
        .separator()
        .item(MenuItem::new("Delete").danger().on_select(nothing));
    let body = ContextMenu::<()>::new(
        "sample-context",
        t,
        col()
            .w(320.0)
            .h(120.0)
            .center()
            .child(Text::new("Right-click here").show(h)),
    )
    .menu(menu)
    .show(h, &mut layers);
    scene(h, cx, body, layers)
}

const ROWS: &[&str] = &["Alpha", "Beta", "Gamma"];

fn rows(_: &()) -> &[&'static str] {
    ROWS
}

fn data_table(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let mut layers = Vec::new();
    let body =
        DataTable::<(), &'static str>::new("sample-table", t, rows, |row: &&str| row.to_string())
            .column(Column::new("name", "Name", |row: &&str| row.to_string()).grow(2.0))
            .height(200.0)
            .show(h, &mut layers);
    scene(h, cx, body, layers)
}

fn dialog(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let mut layers = Vec::new();
    Dialog::<()>::new("sample-dialog", t)
        .open(true)
        .description("You have changes that are not saved yet.")
        .on_close(nothing)
        .returns_focus_to("sample-dialog-trigger")
        .action(Button::new("sample-dialog-no", "Discard", nothing).show(h))
        .action(Button::new("sample-dialog-yes", "Save", nothing).show(h))
        .show(h, &mut layers);
    let body = Text::new("The page behind the dialog.").show(h);
    scene(h, cx, body, layers)
}

fn empty(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = Empty::new(t)
        .description("Nothing matches that search.")
        .icon(Icon::Search)
        .action(Button::new("sample-empty-act", "Reload", nothing).show(h))
        .show(h);
    scene(h, cx, body, vec![])
}

fn form_field(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let mut layers = Vec::new();
    let body = FormField::new("sample-field", t)
        .description("Press the label to focus the field.")
        .required(true)
        .child(Input::new("sample-field-input", "Name", typed, nothing))
        .show(h, &mut layers);
    scene(h, cx, body, layers)
}

fn heading(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = Heading::new(t).level(2).show(h);
    scene(h, cx, body, vec![])
}

fn hover_card(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let mut layers = Vec::new();
    let body = HoverCard::new("sample-hover-card", t)
        .content(Text::new(t).show(h))
        .placement(Placement::Right)
        .align(Align::Start)
        .wrap(h, Text::new("@sample").show(h), &mut layers);
    scene(h, cx, body, layers)
}

fn icon(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = row()
        .gap(8.0)
        .child(Icon::Check.mark(20.0, ink(h.theme().semantic.text.primary)))
        .child(Text::new(t).show(h));
    scene(h, cx, body, vec![])
}

const PNG: &[u8] = include_bytes!("../fixtures/two.png");

fn image(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = col()
        .w(120.0)
        .h(80.0)
        .child(Image::new("sample-image", t).source(PNG).show(h));
    scene(h, cx, body, vec![])
}

fn input(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = Input::new("sample-input", t, typed, nothing).show(h);
    scene(h, cx, body, vec![])
}

fn input_group(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = InputGroup::<(), _, _>::new("sample-group", t, typed, nothing)
        .prefix("https://")
        .suffix(".example")
        .show(h);
    scene(h, cx, body, vec![])
}

fn item(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = Item::new(t)
        .description("Choose when you are told about activity.")
        .action(Badge::new("On").tone(Tone::Success).show(h))
        .action(Button::new("sample-item-act", "Edit", nothing).show(h))
        .show(h);
    scene(h, cx, body, vec![])
}

fn item_group(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = ItemGroup::new(t)
        .description("Where you are signed in.")
        .child(Item::new("Laptop").description("This device").show(h))
        .show(h);
    scene(h, cx, body, vec![])
}

fn kbd(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = Kbd::new(t).show(h);
    scene(h, cx, body, vec![])
}

fn menu(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let mut layers = Vec::new();
    let items = Menu::<()>::new()
        .heading("File")
        .item(
            MenuItem::new(t)
                .icon(Icon::Plus)
                .hint("Ctrl+N")
                .on_select(nothing),
        )
        .separator()
        .item(MenuItem::new("Autosave").checked(true).on_select(nothing))
        .item(MenuItem::new("Delete").danger().on_select(nothing));
    let body = DropdownMenu::<()>::new("sample-menu", t)
        .menu(items)
        .open(true)
        .on_open_change(|_: &mut (), _| {})
        .show(h, &mut layers);
    scene(h, cx, body, layers)
}

fn nav_list(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = NavList::new("sample-nav", t)
        .group("Workspace")
        .item("general", "General")
        .item("members", "Members")
        .group("Account")
        .item("billing", "Billing")
        .show(h);
    scene(h, cx, body, vec![])
}

fn number_input(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = NumberInput::<()>::new("sample-number", t)
        .min(0.0)
        .max(10.0)
        .default_value(3.0)
        .show(h);
    scene(h, cx, body, vec![])
}

fn password_input(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = PasswordInput::new("sample-password", t, typed, nothing)
        .placeholder(t)
        .show(h);
    scene(h, cx, body, vec![])
}

fn popover(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let mut layers = Vec::new();
    let body = Popover::<()>::new("sample-popover", t)
        .title(t)
        .content(|h: &Hosted<()>| Text::new("Filter the list.").show(h))
        .open(true)
        .on_open_change(|_: &mut (), _| {})
        .wrap(
            h,
            Button::new("sample-popover-trigger", "Filters", nothing).show(h),
            &mut layers,
        );
    scene(h, cx, body, layers)
}

fn progress(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = Progress::new("sample-progress", t)
        .value(40.0)
        .show_label(true)
        .show_value(true)
        .show(h);
    scene(h, cx, body, vec![])
}

fn radio_group(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = RadioGroup::<()>::new("sample-radio", t)
        .item("light", "Light")
        .item("dark", "Dark")
        .on_change(|_: &mut (), _: &str| {})
        .show(h);
    scene(h, cx, body, vec![])
}

fn resizable(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = Resizable::horizontal("sample-resizable", t)
        .panel(
            ResizablePanel::new()
                .default_size(0.4)
                .min_size(0.2)
                .child(Text::new(t).show(h)),
        )
        .panel(ResizablePanel::new().child(Text::new("Detail").show(h)))
        .show(h);
    scene(h, cx, body, vec![])
}

fn search_input(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = SearchInput::new("sample-search", t, typed, nothing)
        .shortcut("Ctrl+K")
        .show(h);
    scene(h, cx, body, vec![])
}

fn select(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let mut layers = Vec::new();
    let options = vec![t.to_string(), "Apple".to_string(), "Banana".to_string()];
    let body = Select::new("sample-select", t, options, None, chosen).show(h, &mut layers);
    scene(h, cx, body, layers)
}

fn separator(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = col()
        .gap(8.0)
        .child(Text::new(t).show(h))
        .child(Separator::horizontal().show(h))
        .child(Text::new("Below").show(h));
    scene(h, cx, body, vec![])
}

fn sheet(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let mut layers = Vec::new();
    Sheet::<()>::new("sample-sheet", t)
        .open(true)
        .side(Side::Right)
        .on_close(nothing)
        .returns_focus_to("sample-sheet-trigger")
        .content(Text::new("A modal sheet dims the page.").show(h))
        .show(h, &mut layers);
    let body = Text::new("The page behind the sheet.").show(h);
    scene(h, cx, body, layers)
}

fn skeleton(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = col()
        .w(320.0)
        .gap(12.0)
        .child(Skeleton::text(3).show(h))
        .child(Skeleton::block(48.0, 48.0).label(t).show(h));
    scene(h, cx, body, vec![])
}

fn spinner(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = Spinner::new().label(t).show(h);
    scene(h, cx, body, vec![])
}

fn status_indicator(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = StatusIndicator::new(Status::Error, t).show(h);
    scene(h, cx, body, vec![])
}

fn switch(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = Switch::new("sample-switch", t).show(h);
    scene(h, cx, body, vec![])
}

fn tabs(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = Tabs::<()>::new("sample-tabs", t)
        .tab("one", "One", |h: &Hosted<()>| {
            Text::new("The first panel.").show(h)
        })
        .tab("two", t, |h: &Hosted<()>| {
            Text::new("The second panel.").show(h)
        })
        .show(h);
    scene(h, cx, body, vec![])
}

fn text(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = Text::new(t).show(h);
    scene(h, cx, body, vec![])
}

fn textarea(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = odk_view::Textarea::new("sample-textarea", t, typed)
        .rows(2)
        .max_rows(5)
        .placeholder(t)
        .width(360.0)
        .show(h);
    scene(h, cx, body, vec![])
}

fn toast(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    // A toaster per build: the toast's id is named, so it is the same toast each frame.
    let toaster = Toaster::<()>::new();
    toaster.raise(
        Toast::new(t)
            .with_id(ToastId::named("sample-toast"))
            .description("Your changes are safe.")
            .persistent(),
    );
    let mut layers = Vec::new();
    toaster.show(h, &mut layers);
    let body = Text::new("The page behind the toast.").show(h);
    scene(h, cx, body, layers)
}

fn toggle_group(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let body = ToggleGroup::<()>::new("sample-toggle", t)
        .item("list", "List")
        .item("grid", "Grid")
        .default_value(["list"])
        .show(h);
    scene(h, cx, body, vec![])
}

fn tooltip(h: &Hosted<()>, cx: &mut Cx<Hosted<()>>, t: &str) -> El {
    let mut layers = Vec::new();
    let body = Tooltip::new("sample-tooltip", t).wrap(
        h,
        Button::new("sample-tooltip-trigger", "Hover me", nothing).show(h),
        &mut layers,
    );
    scene(h, cx, body, layers)
}

/// One sample for each public component of `odk-view`, in the order `lib.rs` exports them.
pub const SAMPLES: &[Sample] = &[
    Sample {
        name: "Alert",
        interactive: true,
        build: alert,
    },
    Sample {
        name: "Badge",
        interactive: false,
        build: badge,
    },
    Sample {
        name: "Breadcrumb",
        interactive: true,
        build: breadcrumb,
    },
    Sample {
        name: "Button",
        interactive: true,
        build: button,
    },
    Sample {
        name: "Chart",
        interactive: true,
        build: chart,
    },
    Sample {
        name: "Checkbox",
        interactive: true,
        build: checkbox,
    },
    Sample {
        name: "Command",
        interactive: true,
        build: command,
    },
    Sample {
        name: "ContextMenu",
        interactive: true,
        build: context_menu,
    },
    Sample {
        name: "DataTable",
        interactive: true,
        build: data_table,
    },
    Sample {
        name: "Dialog",
        interactive: true,
        build: dialog,
    },
    Sample {
        name: "Empty",
        interactive: false,
        build: empty,
    },
    Sample {
        name: "FormField",
        interactive: true,
        build: form_field,
    },
    Sample {
        name: "Heading",
        interactive: false,
        build: heading,
    },
    Sample {
        name: "HoverCard",
        interactive: true,
        build: hover_card,
    },
    Sample {
        name: "Icon",
        interactive: false,
        build: icon,
    },
    Sample {
        name: "Image",
        interactive: false,
        build: image,
    },
    Sample {
        name: "Input",
        interactive: true,
        build: input,
    },
    Sample {
        name: "InputGroup",
        interactive: true,
        build: input_group,
    },
    Sample {
        name: "Item",
        interactive: true,
        build: item,
    },
    Sample {
        name: "ItemGroup",
        interactive: false,
        build: item_group,
    },
    Sample {
        name: "Kbd",
        interactive: false,
        build: kbd,
    },
    Sample {
        name: "Menu",
        interactive: true,
        build: menu,
    },
    Sample {
        name: "NavList",
        interactive: true,
        build: nav_list,
    },
    Sample {
        name: "NumberInput",
        interactive: true,
        build: number_input,
    },
    Sample {
        name: "PasswordInput",
        interactive: true,
        build: password_input,
    },
    Sample {
        name: "Popover",
        interactive: true,
        build: popover,
    },
    Sample {
        name: "Progress",
        interactive: false,
        build: progress,
    },
    Sample {
        name: "RadioGroup",
        interactive: true,
        build: radio_group,
    },
    Sample {
        name: "Resizable",
        interactive: true,
        build: resizable,
    },
    Sample {
        name: "SearchInput",
        interactive: true,
        build: search_input,
    },
    Sample {
        name: "Select",
        interactive: true,
        build: select,
    },
    Sample {
        name: "Separator",
        interactive: false,
        build: separator,
    },
    Sample {
        name: "Sheet",
        interactive: true,
        build: sheet,
    },
    Sample {
        name: "Skeleton",
        interactive: false,
        build: skeleton,
    },
    Sample {
        name: "Spinner",
        interactive: false,
        build: spinner,
    },
    Sample {
        name: "StatusIndicator",
        interactive: false,
        build: status_indicator,
    },
    Sample {
        name: "Switch",
        interactive: true,
        build: switch,
    },
    Sample {
        name: "Tabs",
        interactive: true,
        build: tabs,
    },
    Sample {
        name: "Text",
        interactive: false,
        build: text,
    },
    Sample {
        name: "Textarea",
        interactive: true,
        build: textarea,
    },
    Sample {
        name: "Toast",
        interactive: true,
        build: toast,
    },
    Sample {
        name: "ToggleGroup",
        interactive: true,
        build: toggle_group,
    },
    Sample {
        name: "Tooltip",
        interactive: true,
        build: tooltip,
    },
];
