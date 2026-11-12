//! Checklist item 33 on the Rune host: an overlay opened from inside a modal one (a `Dialog` or a
//! `Sheet`) is drawn above it and works there: it takes clicks and keys, is not cut off by the
//! modal's panel or its scrolling body, and Escape closes it before it closes the modal. Nothing
//! here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{
    Button, Dialog, DropdownMenu, Face, Hosted, Menu, MenuItem, Popover, Select, Sheet, Side, Text,
    frame, headless,
};
use rune::{Cx, El, Key, KeyEvent, Runtime, col};

type Rt = Runtime<Hosted<App>>;

const WINDOW: (f32, f32) = (480.0, 360.0);

fn face() -> Face {
    static FACE: OnceLock<Face> = OnceLock::new();
    *FACE.get_or_init(Face::bundled)
}

/// Which modal holds the overlays, and what the application heard.
#[derive(Default)]
struct App {
    sheet: bool,
    modal: bool,
    chosen: Option<usize>,
    picked: Vec<String>,
    popover: bool,
    menu: bool,
    closes: u32,
    /// How many lines of filler push the overlays' triggers down, so the body scrolls.
    filler: usize,
}

/// The popups of the modal's content go after the modal in `layers`, since a later layer draws
/// above an earlier one: `inner` is what the content built, `show` is where the modal goes.
fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let app = &hosted.app;
    let mut layers = Vec::new();
    let mut inner = Vec::new();

    let select = Select::new(
        "sel",
        "Fruit",
        vec!["Apple".into(), "Banana".into(), "Cherry".into()],
        app.chosen,
        |app: &mut App, index| app.chosen = Some(index),
    )
    .show(hosted, &mut inner);
    let menu = DropdownMenu::<App>::new("menu", "Actions")
        .menu(
            Menu::<App>::new()
                .item(
                    MenuItem::new("Rename")
                        .on_select(|app: &mut App| app.picked.push("Rename".into())),
                )
                .item(
                    MenuItem::new("Delete")
                        .on_select(|app: &mut App| app.picked.push("Delete".into())),
                ),
        )
        .open(app.menu)
        .on_open_change(|app: &mut App, open| app.menu = open)
        .show(hosted, &mut inner);
    let popover = Popover::new("pop", "Filters")
        .title("Filters")
        .open(app.popover)
        .on_open_change(|app: &mut App, open| app.popover = open)
        .content(|hosted: &Hosted<App>| {
            Button::new("apply", "Apply", |_: &mut App| {}).show(hosted)
        })
        .wrap(hosted, Text::new("Filters").show(hosted), &mut inner);

    let mut content = col().gap(8.0);
    for n in 0..app.filler {
        content = content.child(Text::new(format!("Filler line {n}")).show(hosted));
    }
    // The triggers are last, at the bottom of a body that may scroll.
    let content = content.child(select).child(menu).child(popover);

    if app.sheet {
        Sheet::<App>::new("modal", "Settings")
            .side(Side::Right)
            .open(app.modal)
            .on_close(|app: &mut App| {
                app.closes += 1;
                app.modal = false;
            })
            .content(content)
            .show(hosted, &mut layers);
    } else {
        Dialog::<App>::new("modal", "Settings")
            .open(app.modal)
            .on_close(|app: &mut App| {
                app.closes += 1;
                app.modal = false;
            })
            .content(content)
            .show(hosted, &mut layers);
    }
    layers.extend(inner);
    frame(hosted, cx, col().key("page"), layers)
}

fn open(app: App) -> Rt {
    let mut runtime = headless(
        Hosted::new(
            App { modal: true, ..app },
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
    for _ in 0..24 {
        runtime.frame(1.0 / 60.0);
    }
}

fn tap(runtime: &mut Rt, key: &str) {
    let (x, y) = runtime
        .find(key)
        .unwrap_or_else(|| panic!("no element {key}"));
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(runtime);
}

fn press(runtime: &mut Rt, key: Key) {
    runtime.key(KeyEvent::from(key));
    settle(runtime);
}

fn has(runtime: &Rt, key: &str) -> bool {
    runtime
        .elements()
        .iter()
        .any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn rect(runtime: &Rt, key: &str) -> [f32; 4] {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no element {key}"))
        .rect
}

fn ancestors(runtime: &Rt, key: &str) -> Vec<String> {
    let elements = runtime.elements();
    let mut at = elements
        .iter()
        .position(|e| e.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no element {key}"));
    let mut keys = Vec::new();
    while let Some(parent) = elements[at].parent {
        at = parent;
        if let Some(key) = elements[at].key.clone() {
            keys.push(key);
        }
    }
    keys
}

fn focused(runtime: &Rt) -> Option<String> {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.focused)
        .and_then(|e| e.key)
}

/// For both modals, since they are the two that GPUI could not nest an overlay in.
fn both(check: impl Fn(&str, bool)) {
    check("a dialog", false);
    check("a sheet", true);
}

#[test]
fn a_select_in_a_modal_opens_above_it_and_takes_a_click() {
    both(|name, sheet| {
        let mut runtime = open(App {
            sheet,
            ..App::default()
        });
        tap(&mut runtime, "sel-trigger");
        assert!(has(&runtime, "sel-list"), "{name}: the list opens");
        let above = ancestors(&runtime, "sel-list");
        assert!(
            above.iter().any(|k| k == "odk-root") && !above.iter().any(|k| k == "modal"),
            "{name}: a layer of its own, not inside the modal's panel: {above:?}"
        );
        // The list is above the modal's panel: it is the element at its own place.
        let (x, y) = runtime.find("sel-1").expect("the second option");
        tap(&mut runtime, "sel-1");
        let _ = (x, y);
        assert_eq!(
            runtime.state.app.chosen,
            Some(1),
            "{name}: it took the click"
        );
        assert!(!has(&runtime, "sel-list"), "{name}: and closed");
        assert!(has(&runtime, "modal"), "{name}: the modal stays open");
        assert_eq!(runtime.state.app.closes, 0, "{name}");
    });
}

#[test]
fn escape_closes_the_overlay_before_the_modal() {
    both(|name, sheet| {
        let mut runtime = open(App {
            sheet,
            ..App::default()
        });
        for (trigger, popup) in [
            ("sel-trigger", "sel-list"),
            ("menu", "menu-menu"),
            ("pop", "pop-panel"),
        ] {
            tap(&mut runtime, trigger);
            assert!(has(&runtime, popup), "{name}: {popup} opens");
            press(&mut runtime, Key::Escape);
            assert!(!has(&runtime, popup), "{name}: Escape closes {popup}");
            assert!(has(&runtime, "modal"), "{name}: and not the modal");
            assert_eq!(runtime.state.app.closes, 0, "{name}");
        }
        press(&mut runtime, Key::Escape);
        assert_eq!(
            runtime.state.app.closes, 1,
            "{name}: the next Escape closes the modal"
        );
    });
}

#[test]
fn a_menu_in_a_modal_takes_keys_and_a_click() {
    both(|name, sheet| {
        let mut runtime = open(App {
            sheet,
            ..App::default()
        });
        tap(&mut runtime, "menu");
        assert!(has(&runtime, "menu-menu"), "{name}");
        assert!(
            focused(&runtime).is_some_and(|k| k.starts_with("menu-")),
            "{name}: focus is in the menu: {:?}",
            focused(&runtime)
        );
        press(&mut runtime, Key::Down);
        press(&mut runtime, Key::Enter);
        assert_eq!(
            runtime.state.app.picked.len(),
            1,
            "{name}: a key chose an item"
        );
        tap(&mut runtime, "menu");
        tap(&mut runtime, "menu-item-1");
        assert_eq!(
            runtime.state.app.picked.last().map(String::as_str),
            Some("Delete"),
            "{name}: a click chose one"
        );
        assert!(has(&runtime, "modal"), "{name}");
    });
}

#[test]
fn a_popover_in_a_modal_takes_a_click() {
    both(|name, sheet| {
        let mut runtime = open(App {
            sheet,
            ..App::default()
        });
        tap(&mut runtime, "pop");
        assert!(has(&runtime, "pop-panel"), "{name}");
        assert!(has(&runtime, "apply"), "{name}: its content is there");
        tap(&mut runtime, "apply");
        assert!(
            has(&runtime, "modal"),
            "{name}: a press in it is not the modal's"
        );
        assert_eq!(runtime.state.app.closes, 0, "{name}");
    });
}

#[test]
fn an_overlay_is_not_cut_off_by_the_modals_panel_or_its_scrolling_body() {
    both(|name, sheet| {
        // A body that is much taller than the window scrolls, with the triggers at its end.
        let mut runtime = open(App {
            sheet,
            filler: 40,
            ..App::default()
        });
        let panel = rect(&runtime, "modal");
        assert!(
            panel[1] + panel[3] <= WINDOW.1 + 0.5,
            "{name}: the panel fits"
        );
        // Bring the triggers into view with the keyboard, as a person would.
        for _ in 0..8 {
            if focused(&runtime).as_deref() == Some("sel-trigger") {
                break;
            }
            press(&mut runtime, Key::Tab);
        }
        assert_eq!(focused(&runtime).as_deref(), Some("sel-trigger"), "{name}");
        press(&mut runtime, Key::Enter);
        assert!(has(&runtime, "sel-list"), "{name}: the list opens");
        let list = rect(&runtime, "sel-list");
        let (top, bottom) = (list[1], list[1] + list[3]);
        assert!(list[3] > 20.0, "{name}: the list has a height: {list:?}");
        assert!(
            top >= -0.5 && bottom <= WINDOW.1 + 0.5,
            "{name}: it keeps inside the window: {list:?}"
        );
        // Whatever the panel's edges were, it is not drawn inside them: its parents do not
        // include the panel or the scrolling body.
        let above = ancestors(&runtime, "sel-list");
        assert!(!above.iter().any(|k| k == "modal"), "{name}: {above:?}");
    });
}
