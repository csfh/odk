//! Overlays with more rows than the window has room for (`docs/roadmap.md`, Track A, "the focus
//! reveal for overlays and overlay sizing"): the list is no taller than the window, stays inside
//! it wherever its trigger is, scrolls, and keeps the row the keyboard is on in view. Nothing
//! here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{
    Command, CommandItem, ContextMenu, DropdownMenu, Face, Hosted, Menu, MenuItem, Select, Text,
    frame, headless,
};
use rune::{Cx, El, Key, KeyEvent, Runtime, col};

type Rt = Runtime<Hosted<App>>;

const WINDOW: (f32, f32) = (480.0, 240.0);
const ROWS: usize = 60;

fn face() -> Face {
    static FACE: OnceLock<Face> = OnceLock::new();
    *FACE.get_or_init(Face::bundled)
}

#[derive(Default)]
struct App {
    /// How far down the window the triggers sit.
    top: f32,
    chosen: Option<usize>,
    menu: bool,
    palette: bool,
}

fn labels() -> Vec<String> {
    (0..ROWS).map(|n| format!("Option {n:02}")).collect()
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let app = &hosted.app;
    let mut layers = Vec::new();
    let select = Select::new("sel", "Fruit", labels(), app.chosen, |a: &mut App, i| {
        a.chosen = Some(i)
    })
    .show(hosted, &mut layers);
    let mut menu = Menu::<App>::new();
    for n in 0..ROWS {
        menu = menu.item(MenuItem::new(format!("Item {n:02}")));
    }
    let dropdown = DropdownMenu::<App>::new("menu", "Actions")
        .menu(menu)
        .open(app.menu)
        .on_open_change(|a: &mut App, open| a.menu = open)
        .show(hosted, &mut layers);
    let mut palette = Command::<App>::new("pal", "Palette")
        .open(app.palette)
        .on_close(|a: &mut App| a.palette = false);
    for n in 0..ROWS {
        palette = palette.item(CommandItem::<App>::new(format!("Command {n:02}")));
    }
    palette.show(hosted, &mut layers);
    let mut context_menu = Menu::<App>::new();
    for n in 0..ROWS {
        context_menu = context_menu.item(MenuItem::new(format!("Entry {n:02}")));
    }
    let context = ContextMenu::<App>::new(
        "ctx",
        "Row",
        col()
            .w(120.0)
            .h(40.0)
            .child(Text::new("Right-click").show(hosted)),
    )
    .menu(context_menu)
    .show(hosted, &mut layers);
    let body = col()
        .key("page")
        .pt(app.top)
        .gap(8.0)
        .child(select)
        .child(dropdown)
        .child(context);
    frame(hosted, cx, body, layers)
}

fn open(top: f32) -> Rt {
    let mut runtime = headless(
        Hosted::new(
            App {
                top,
                ..App::default()
            },
            ThemeSnapshot::emergency(),
            face(),
        ),
        page,
        WINDOW,
    );
    settle(&mut runtime);
    runtime
}

fn settle(runtime: &mut Rt) {
    for _ in 0..30 {
        runtime.frame(1.0 / 60.0);
    }
}

fn rect(runtime: &Rt, key: &str) -> Option<[f32; 4]> {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key) && !e.exiting)
        .map(|e| e.rect)
}

fn tap(runtime: &mut Rt, key: &str) {
    let (x, y) = runtime.find(key).unwrap_or_else(|| panic!("no {key}"));
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(runtime);
}

fn press(runtime: &mut Rt, key: Key) {
    runtime.key(KeyEvent::from(key));
    settle(runtime);
}

fn in_window(rect: [f32; 4]) -> bool {
    rect[0] >= -0.5
        && rect[1] >= -0.5
        && rect[0] + rect[2] <= WINDOW.0 + 0.5
        && rect[1] + rect[3] <= WINDOW.1 + 0.5
}

/// A row is in view when what its ancestors' clips leave of it is all of it, to a hair.
fn row_in_view(runtime: &Rt, key: &str) -> bool {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key) && !e.exiting)
        .is_some_and(|e| e.rect[3] > 0.0 && e.visible[3] >= e.rect[3] - 0.5)
}

fn focused(runtime: &Rt) -> Option<String> {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.focused)
        .and_then(|e| e.key)
}

const TOPS: [f32; 3] = [0.0, 100.0, 190.0];

#[test]
fn a_select_with_many_options_fits_the_window_and_keeps_its_highlight_in_view() {
    for top in TOPS {
        let mut rt = open(top);
        tap(&mut rt, "sel-trigger");
        let list = rect(&rt, "sel-popup").expect("the popup");
        assert!(
            in_window(list),
            "top {top}: popup {list:?} is inside the window"
        );
        assert!(
            list[3] >= 60.0,
            "top {top}: the popup has room to use: {list:?}"
        );
        assert!(
            row_in_view(&rt, "sel-0"),
            "top {top}: it opens on the first option"
        );
        for step in 1..=30 {
            press(&mut rt, Key::Down);
            assert!(
                row_in_view(&rt, &format!("sel-{step}")),
                "top {top}: option {step} is the highlight and is in view"
            );
        }
        // End goes to the last option, and it is in view too.
        press(&mut rt, Key::End);
        assert!(
            row_in_view(&rt, &format!("sel-{}", ROWS - 1)),
            "top {top}: the last option"
        );
    }
}

#[test]
fn a_select_opens_on_its_chosen_option_in_view() {
    let mut rt = open(0.0);
    rt.state.app.chosen = Some(45);
    rt.invalidate();
    settle(&mut rt);
    tap(&mut rt, "sel-trigger");
    assert!(
        row_in_view(&rt, "sel-45"),
        "the chosen option is in view as the list opens"
    );
}

#[test]
fn a_dropdown_menu_with_many_items_fits_the_window_and_keeps_the_focused_item_in_view() {
    for top in TOPS {
        let mut rt = open(top);
        tap(&mut rt, "menu");
        let list = rect(&rt, "menu-menu").expect("the menu");
        assert!(
            in_window(list),
            "top {top}: menu {list:?} is inside the window"
        );
        assert!(list[3] >= 60.0, "top {top}: room to use: {list:?}");
        for _ in 0..30 {
            press(&mut rt, Key::Down);
        }
        let at = focused(&rt).expect("an item has focus");
        assert!(at.starts_with("menu-item-"), "{at}");
        assert!(
            row_in_view(&rt, &at),
            "top {top}: the focused item {at} is in view"
        );
    }
}

#[test]
fn a_menu_at_the_pointer_fits_the_window_wherever_it_opens() {
    for top in [0.0, 60.0, 100.0, 120.0] {
        let mut rt = open(top);
        let (x, y) = rt.find("ctx").expect("the area");
        rt.secondary_down(x, y);
        rt.secondary_up(x, y);
        settle(&mut rt);
        let list = rect(&rt, "ctx-menu").unwrap_or_else(|| panic!("no menu, top {top}"));
        assert!(
            in_window(list),
            "top {top}: menu {list:?} is inside the window"
        );
        assert!(list[3] >= 60.0, "top {top}: room to use: {list:?}");
        assert!(
            (list[0] - x).abs() < 2.0,
            "top {top}: the menu opens at the pointer, x {x}: {list:?}"
        );
    }
}

#[test]
fn a_palette_with_many_commands_fits_the_window_and_keeps_its_highlight_in_view() {
    let mut rt = open(0.0);
    rt.state.app.palette = true;
    rt.invalidate();
    settle(&mut rt);
    let panel = rect(&rt, "pal").expect("the palette");
    assert!(in_window(panel), "palette {panel:?} is inside the window");
    assert!(
        row_in_view(&rt, "pal-item-0"),
        "it opens on the first command"
    );
    for step in 1..=30 {
        press(&mut rt, Key::Down);
        assert!(
            row_in_view(&rt, &format!("pal-item-{step}")),
            "command {step} is the highlight and is in view"
        );
    }
    // End moves the field's caret, not the list: walk down to the last command.
    for _ in 31..ROWS {
        press(&mut rt, Key::Down);
    }
    assert!(
        row_in_view(&rt, &format!("pal-item-{}", ROWS - 1)),
        "the last command is in view"
    );
}
