//! Checklist items 21, 22 and 39 on the Rune host: what disappears runs an exit and keeps what it
//! needs to draw it, a transition reversed partway does not jump, and one that is interrupted
//! ends on the state and leaves nothing running.
//!
//! Rune owns the presence lifecycle: a removed element stays drawn, flagged `exiting`, until its
//! exit has run; a returning one continues from where it is. These tests hold the overlays to
//! that: none of them may unmount with `Tx::instant` or `settle`, draw its exit from state it no
//! longer has, or ask for a frame once it has settled.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{
    Command, CommandItem, Dialog, DropdownMenu, Face, Hosted, Menu, MenuItem, Popover, Progress,
    Select, Sheet, Side, Switch, Text, Toast, Toaster, frame, headless,
};
use rune::{Cx, El, Runtime, col};

type Rt = Runtime<Hosted<App>>;

const SIZE: (f32, f32) = (480.0, 360.0);
const DT: f64 = 1.0 / 60.0;

fn face() -> Face {
    static FACE: OnceLock<Face> = OnceLock::new();
    *FACE.get_or_init(|| {
        let output = std::process::Command::new("fc-match")
            .args(["-f", "%{file}", "sans"])
            .output()
            .expect("fc-match");
        let path = String::from_utf8(output.stdout).expect("utf-8");
        Face::leak(std::fs::read(path.trim()).expect("font"), "Inter".into())
    })
}

/// Which overlays are open, and which are in the page at all (a component that goes).
struct App {
    dialog: bool,
    left: bool,
    right: bool,
    present: bool,
    menu: bool,
    palette: bool,
    popover: bool,
    chosen: Option<usize>,
    wifi: bool,
    value: f64,
    toaster: Toaster<App>,
}

impl Default for App {
    fn default() -> Self {
        App {
            dialog: false,
            left: false,
            right: false,
            present: true,
            menu: false,
            palette: false,
            popover: false,
            chosen: None,
            wifi: false,
            value: 10.0,
            toaster: Toaster::new(),
        }
    }
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let mut layers = Vec::new();
    if hosted.app.present {
        Dialog::<App>::new("dlg", "Title")
            .open(hosted.app.dialog)
            .on_close(|a: &mut App| a.dialog = false)
            .show(hosted, &mut layers);
        Sheet::<App>::new("left", "Left")
            .side(Side::Left)
            .open(hosted.app.left)
            .on_close(|a: &mut App| a.left = false)
            .show(hosted, &mut layers);
        Sheet::<App>::new("right", "Right")
            .side(Side::Right)
            .open(hosted.app.right)
            .on_close(|a: &mut App| a.right = false)
            .show(hosted, &mut layers);
    }
    let trigger = DropdownMenu::<App>::new("menu", "Actions")
        .menu(
            Menu::<App>::new()
                .item(MenuItem::new("New"))
                .item(MenuItem::new("Open")),
        )
        .open(hosted.app.menu)
        .on_open_change(|app: &mut App, open| app.menu = open)
        .show(hosted, &mut layers);
    Command::<App>::new("pal", "Command palette")
        .open(hosted.app.palette)
        .on_close(|a: &mut App| a.palette = false)
        .item(CommandItem::<App>::new("Save file"))
        .item(CommandItem::<App>::new("Open file"))
        .show(hosted, &mut layers);
    let mut body = col().key("page").child(trigger);
    if hosted.app.present {
        let popover = Popover::new("pop", "Filters")
            .title("Filters")
            .open(hosted.app.popover)
            .on_open_change(|a: &mut App, open| a.popover = open)
            .wrap(hosted, Text::new("Filters").show(hosted), &mut layers);
        let select = Select::new(
            "sl",
            "Fruit",
            vec!["Apple".into(), "Banana".into()],
            hosted.app.chosen,
            |a: &mut App, index| a.chosen = Some(index),
        )
        .show(hosted, &mut layers);
        body = body
            .child(popover)
            .child(select)
            .child(
                Switch::new("wifi", "Wi-Fi")
                    .checked(hosted.app.wifi)
                    .on_change(|a: &mut App, on| a.wifi = on)
                    .show(hosted),
            )
            .child(
                Progress::new("up", "Upload")
                    .value(hosted.app.value)
                    .show(hosted),
            );
        hosted.app.toaster.show(hosted, &mut layers);
    }
    frame(hosted, cx, body, layers)
}

fn open_page() -> Rt {
    let mut runtime = headless(
        Hosted::new(App::default(), ThemeSnapshot::emergency(), face()),
        page,
        SIZE,
    );
    for _ in 0..6 {
        runtime.frame(DT);
    }
    runtime
}

fn change(runtime: &mut Rt, change: impl FnOnce(&mut Hosted<App>)) {
    change(&mut runtime.state);
    runtime.invalidate();
}

fn run(runtime: &mut Rt, frames: usize) {
    for _ in 0..frames {
        runtime.frame(DT);
    }
}

/// Frames until the runtime asks for none, which a transition that is not running must reach.
fn settle(runtime: &mut Rt) {
    for _ in 0..240 {
        if !runtime.frame(DT) && runtime.wake_deadline().is_none() {
            return;
        }
    }
    panic!("still animating after four seconds");
}

fn open_everything(runtime: &mut Rt) {
    change(runtime, |h| {
        h.app.dialog = true;
        h.app.left = true;
        h.app.right = true;
    });
    settle(runtime);
}

fn close_everything(runtime: &mut Rt) {
    change(runtime, |h| {
        h.app.dialog = false;
        h.app.left = false;
        h.app.right = false;
    });
}

struct Seen {
    rect: [f32; 4],
    opacity: f32,
    exiting: bool,
}

fn seen(runtime: &Rt, key: &str) -> Option<Seen> {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .map(|e| Seen {
            rect: e.rect,
            opacity: e.opacity,
            exiting: e.exiting,
        })
}

fn seen_now(runtime: &Rt, key: &str) -> Seen {
    seen(runtime, key).unwrap_or_else(|| panic!("{key} is not drawn"))
}

/// Nothing exits, nothing runs and the runtime asks for no frame.
fn assert_nothing_running(runtime: &Rt, what: &str) {
    assert_eq!(
        runtime.wake_deadline(),
        None,
        "{what}: a frame is still wanted"
    );
    let leftover: Vec<_> = runtime
        .elements()
        .into_iter()
        .filter(|e| e.exiting)
        .map(|e| e.key)
        .collect();
    assert!(leftover.is_empty(), "{what}: still exiting: {leftover:?}");
}

/// A palette that is not the emergency one, to change the theme with.
fn another_theme() -> ThemeSnapshot {
    ThemeSnapshot::from_toml(
        r##"
mode = "dark"
accent = "#d0d0d0"
muted = "#8a8a8a"
background = "#202a30"
dark_background = "#141414"
lighter_background = "#2a2a2a"
foreground = "#d0d0d0"
red = "#d0d0d0"
yellow = "#d0d0d0"
green = "#d0d0d0"
blue = "#d0d0d0"
"##,
        None,
    )
    .expect("a palette")
}

const KEYS: [&str; 3] = ["dlg", "left", "right"];

#[test]
fn a_closing_overlay_runs_its_exit_and_keeps_what_it_draws_it_with() {
    let mut runtime = open_page();
    open_everything(&mut runtime);
    let open: Vec<_> = KEYS.iter().map(|k| seen_now(&runtime, k)).collect();
    assert!(
        open.iter().all(|s| !s.exiting && s.opacity > 0.99),
        "settled open"
    );

    close_everything(&mut runtime);
    run(&mut runtime, 3);
    for (key, before) in KEYS.iter().zip(&open) {
        let during = seen(&runtime, key).unwrap_or_else(|| panic!("{key} vanished with no exit"));
        assert!(during.exiting, "{key} runs an exit");
        assert_eq!(
            during.rect, before.rect,
            "{key} keeps the place and the side it was open at"
        );
        assert!(
            during.opacity < before.opacity,
            "{key} is fading ({} -> {})",
            before.opacity,
            during.opacity
        );
    }
    // A sheet leaves by the edge it came from: the left one stays at the left of the window and
    // the right one at the right, not both on the side the closing frame would give a default.
    assert_eq!(seen_now(&runtime, "left").rect[0], 0.0);
    let right = seen_now(&runtime, "right").rect;
    assert_eq!(right[0] + right[2], SIZE.0);

    settle(&mut runtime);
    for key in KEYS {
        assert!(
            seen(&runtime, key).is_none(),
            "{key} is gone after its exit"
        );
    }
    assert_nothing_running(&runtime, "after the exits");
}

#[test]
fn an_overlay_reopened_while_it_closes_does_not_jump() {
    let mut runtime = open_page();
    open_everything(&mut runtime);
    close_everything(&mut runtime);
    run(&mut runtime, 4);
    let closing: Vec<_> = KEYS.iter().map(|k| seen_now(&runtime, k)).collect();
    assert!(closing.iter().all(|s| s.exiting && s.opacity < 1.0));

    change(&mut runtime, |h| {
        h.app.dialog = true;
        h.app.left = true;
        h.app.right = true;
    });
    runtime.frame(DT);
    for (key, before) in KEYS.iter().zip(&closing) {
        let after = seen_now(&runtime, key);
        assert!(!after.exiting, "{key} is open again");
        assert_eq!(after.rect, before.rect, "{key} does not move");
        assert!(
            (after.opacity - before.opacity).abs() < 0.2,
            "{key} continues from {} and not from nothing: {}",
            before.opacity,
            after.opacity
        );
    }
    settle(&mut runtime);
    for key in KEYS {
        assert!(seen_now(&runtime, key).opacity > 0.99, "{key} ends open");
    }
    assert_nothing_running(&runtime, "after the reversal");
}

#[test]
fn an_overlay_closed_while_it_opens_does_not_jump() {
    let mut runtime = open_page();
    change(&mut runtime, |h| h.app.dialog = true);
    run(&mut runtime, 3);
    let opening = seen_now(&runtime, "dlg");
    assert!(
        opening.opacity > 0.0 && opening.opacity < 1.0,
        "mid-entrance"
    );

    change(&mut runtime, |h| h.app.dialog = false);
    runtime.frame(DT);
    let after = seen_now(&runtime, "dlg");
    assert!(after.exiting);
    assert!(
        (after.opacity - opening.opacity).abs() < 0.2,
        "the exit starts at {}, not at full: {}",
        opening.opacity,
        after.opacity
    );
    settle(&mut runtime);
    assert!(seen(&runtime, "dlg").is_none());
    assert_nothing_running(&runtime, "after the reversal");
}

#[test]
fn a_transition_interrupted_by_another_state_change_ends_on_the_state() {
    let mut runtime = open_page();
    open_everything(&mut runtime);
    // The dialog closes and, while it fades, the sheets move on; the last state is what shows.
    close_everything(&mut runtime);
    run(&mut runtime, 3);
    change(&mut runtime, |h| {
        h.app.left = true;
        h.app.right = false;
    });
    run(&mut runtime, 2);
    change(&mut runtime, |h| h.app.left = false);
    settle(&mut runtime);
    for key in KEYS {
        assert!(seen(&runtime, key).is_none(), "{key} ends closed");
    }
    assert_nothing_running(&runtime, "after the last change");

    change(&mut runtime, |h| h.app.dialog = true);
    run(&mut runtime, 2);
    change(&mut runtime, |h| h.app.dialog = false);
    run(&mut runtime, 2);
    change(&mut runtime, |h| h.app.dialog = true);
    settle(&mut runtime);
    assert!(!seen_now(&runtime, "dlg").exiting);
    assert!(seen_now(&runtime, "dlg").opacity > 0.99, "ends open");
    assert_nothing_running(&runtime, "after the dialog's flurry");
}

#[test]
fn a_theme_change_during_a_transition_leaves_nothing_running() {
    let mut runtime = open_page();
    open_everything(&mut runtime);
    close_everything(&mut runtime);
    run(&mut runtime, 3);

    change(&mut runtime, |h| h.set_theme(another_theme()));
    settle(&mut runtime);
    for key in KEYS {
        assert!(seen(&runtime, key).is_none(), "{key} is gone");
    }
    assert_nothing_running(&runtime, "after the theme change");

    // And one that lands as an overlay opens.
    change(&mut runtime, |h| h.app.dialog = true);
    run(&mut runtime, 3);
    change(&mut runtime, |h| h.set_theme(ThemeSnapshot::emergency()));
    settle(&mut runtime);
    assert!(seen_now(&runtime, "dlg").opacity > 0.99);
    assert_nothing_running(&runtime, "after the theme change mid-entrance");
}

#[test]
fn reduced_motion_turned_on_during_a_transition_ends_it_at_once() {
    let mut runtime = open_page();
    open_everything(&mut runtime);
    close_everything(&mut runtime);
    run(&mut runtime, 3);
    assert!(seen_now(&runtime, "dlg").exiting);

    let mut calm = ThemeSnapshot::emergency();
    calm.reduced_motion = true;
    change(&mut runtime, |h| h.set_theme(calm));
    run(&mut runtime, 2);
    // An exit that has already started keeps the fade it began with (`present` cannot reach an
    // element that is already removed): it runs out and nothing is left running.
    settle(&mut runtime);
    for key in KEYS {
        assert!(seen(&runtime, key).is_none(), "{key} is gone");
    }
    assert_nothing_running(&runtime, "after reduced motion");

    // With it on, an overlay shows whole at once and goes at once.
    change(&mut runtime, |h| h.app.dialog = true);
    run(&mut runtime, 2);
    assert!(
        seen_now(&runtime, "dlg").opacity > 0.99,
        "reduced motion shows the end state"
    );
    change(&mut runtime, |h| h.app.dialog = false);
    run(&mut runtime, 2);
    assert!(
        seen(&runtime, "dlg").is_none_or(|s| s.opacity < 0.01),
        "reduced motion hides at once"
    );
    settle(&mut runtime);
    assert_nothing_running(&runtime, "reduced motion, closed");
}

#[test]
fn a_component_removed_mid_transition_leaves_nothing_running() {
    let mut runtime = open_page();
    open_everything(&mut runtime);
    close_everything(&mut runtime);
    run(&mut runtime, 3);
    assert!(seen_now(&runtime, "left").exiting);

    // The page stops building the overlays at all while they fade.
    change(&mut runtime, |h| h.app.present = false);
    settle(&mut runtime);
    for key in KEYS {
        assert!(seen(&runtime, key).is_none(), "{key} is gone");
    }
    assert_nothing_running(&runtime, "after the component went");

    // And one that goes while it is open.
    change(&mut runtime, |h| h.app.present = true);
    open_everything(&mut runtime);
    change(&mut runtime, |h| h.app.present = false);
    settle(&mut runtime);
    for key in KEYS {
        assert!(seen(&runtime, key).is_none(), "{key} is gone");
    }
    assert_nothing_running(&runtime, "after an open component went");
}

const POPUPS: [&str; 2] = ["menu-menu", "pal"];

fn calm() -> ThemeSnapshot {
    let mut calm = ThemeSnapshot::emergency();
    calm.reduced_motion = true;
    calm
}

#[test]
fn a_menu_and_the_palette_exit_where_they_were_and_can_be_reopened() {
    let mut runtime = open_page();
    change(&mut runtime, |h| {
        h.app.menu = true;
        h.app.palette = true;
    });
    settle(&mut runtime);
    let open: Vec<_> = POPUPS.iter().map(|k| seen_now(&runtime, k)).collect();

    change(&mut runtime, |h| {
        h.app.menu = false;
        h.app.palette = false;
    });
    run(&mut runtime, 3);
    let closing: Vec<_> = POPUPS.iter().map(|k| seen_now(&runtime, k)).collect();
    for ((key, before), during) in POPUPS.iter().zip(&open).zip(&closing) {
        assert!(during.exiting, "{key} runs an exit");
        assert_eq!(during.rect, before.rect, "{key} keeps its place");
    }

    change(&mut runtime, |h| {
        h.app.menu = true;
        h.app.palette = true;
    });
    runtime.frame(DT);
    for (key, before) in POPUPS.iter().zip(&closing) {
        let after = seen_now(&runtime, key);
        assert!(!after.exiting, "{key} is open again");
        assert_eq!(after.rect, before.rect, "{key} does not move");
        assert!(
            (after.opacity - before.opacity).abs() < 0.2,
            "{key}: {} -> {}",
            before.opacity,
            after.opacity
        );
    }
    settle(&mut runtime);
    assert_nothing_running(&runtime, "after the popups settled open");

    change(&mut runtime, |h| {
        h.app.menu = false;
        h.app.palette = false;
        h.app.present = false;
    });
    settle(&mut runtime);
    assert_nothing_running(&runtime, "after the popups and their page went");
}

#[test]
fn reduced_motion_shows_and_hides_every_overlay_at_once() {
    let mut runtime = open_page();
    change(&mut runtime, |h| h.set_theme(calm()));
    run(&mut runtime, 2);
    let everything = |h: &mut Hosted<App>, on: bool| {
        h.app.dialog = on;
        h.app.left = on;
        h.app.right = on;
        h.app.menu = on;
        h.app.palette = on;
    };
    change(&mut runtime, |h| everything(h, true));
    run(&mut runtime, 2);
    for key in ["dlg", "left", "right", "menu-menu", "pal"] {
        assert!(
            seen_now(&runtime, key).opacity > 0.99,
            "{key} is whole at once"
        );
    }
    // Nor is anything inside them, or the scrims, still on its way in.
    let faint: Vec<_> = runtime
        .elements()
        .into_iter()
        .filter(|e| e.opacity < 0.99)
        .map(|e| (e.key, e.text))
        .collect();
    assert!(faint.is_empty(), "still fading in: {faint:?}");
    change(&mut runtime, |h| everything(h, false));
    run(&mut runtime, 2);
    for key in ["dlg", "left", "right", "menu-menu", "pal"] {
        assert!(seen(&runtime, key).is_none(), "{key} is gone at once");
    }
    assert_nothing_running(&runtime, "reduced motion, everything closed");
}

/// Opens something, closes it and opens it again while it goes, as the application would.
fn exits_and_returns(runtime: &mut Rt, key: &str, open: impl Fn(&mut Rt), close: impl Fn(&mut Rt)) {
    open(runtime);
    settle(runtime);
    let whole = seen_now(runtime, key);
    assert!(!whole.exiting && whole.opacity > 0.99, "{key} is open");

    close(runtime);
    run(runtime, 3);
    let closing = seen(runtime, key).unwrap_or_else(|| panic!("{key} vanished with no exit"));
    assert!(closing.exiting, "{key} runs an exit");
    assert_eq!(closing.rect, whole.rect, "{key} keeps its place");
    assert!(closing.opacity < whole.opacity, "{key} is fading");

    open(runtime);
    runtime.frame(DT);
    let back = seen_now(runtime, key);
    assert!(!back.exiting, "{key} is open again");
    assert_eq!(back.rect, closing.rect, "{key} does not move");
    assert!(
        (back.opacity - closing.opacity).abs() < 0.2,
        "{key} continues from {} and not from nothing: {}",
        closing.opacity,
        back.opacity
    );
    settle(runtime);
    assert!(seen_now(runtime, key).opacity > 0.99, "{key} ends open");
    assert_nothing_running(runtime, "after the reversal");

    close(runtime);
    settle(runtime);
    assert!(seen(runtime, key).is_none(), "{key} is gone after its exit");
    assert_nothing_running(runtime, "after the exit");
}

fn open_popover(runtime: &mut Rt) {
    change(runtime, |h| h.app.popover = true);
}

fn close_popover(runtime: &mut Rt) {
    change(runtime, |h| h.app.popover = false);
}

fn click(runtime: &mut Rt, key: &str) {
    let (x, y) = runtime
        .find(key)
        .unwrap_or_else(|| panic!("no element {key}"));
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    runtime.frame(DT);
}

/// The trigger opens the list: a press on it while it is closed, or on its way out.
fn open_select(runtime: &mut Rt) {
    click(runtime, "sl-trigger");
}

fn close_select(runtime: &mut Rt) {
    runtime.key(rune::KeyEvent::from(rune::Key::Escape));
    runtime.frame(DT);
}

#[test]
fn a_popover_exits_where_it_was_and_can_be_reopened() {
    let mut runtime = open_page();
    exits_and_returns(&mut runtime, "pop-panel", open_popover, close_popover);
}

#[test]
fn a_select_list_exits_where_it_was_and_can_be_reopened() {
    let mut runtime = open_page();
    exits_and_returns(&mut runtime, "sl-popup", open_select, close_select);
}

#[test]
fn reduced_motion_shows_and_hides_a_popover_and_a_select_at_once() {
    let mut runtime = open_page();
    change(&mut runtime, |h| h.set_theme(calm()));
    run(&mut runtime, 2);
    for (key, open, close) in [
        (
            "pop-panel",
            open_popover as fn(&mut Rt),
            close_popover as fn(&mut Rt),
        ),
        ("sl-popup", open_select, close_select),
    ] {
        open(&mut runtime);
        run(&mut runtime, 2);
        assert!(
            seen_now(&runtime, key).opacity > 0.99,
            "{key} is whole at once"
        );
        let faint: Vec<_> = runtime
            .elements()
            .into_iter()
            .filter(|e| e.opacity < 0.99)
            .map(|e| (e.key, e.text))
            .collect();
        assert!(faint.is_empty(), "{key}: still fading in: {faint:?}");
        close(&mut runtime);
        run(&mut runtime, 2);
        assert!(seen(&runtime, key).is_none(), "{key} is gone at once");
        assert_nothing_running(&runtime, key);
    }
}

fn thumb(runtime: &Rt) -> f32 {
    seen_now(runtime, "wifi-thumb").rect[0]
}

fn share(runtime: &Rt) -> f32 {
    seen_now(runtime, "up-bar").rect[2] / seen_now(runtime, "up-track").rect[2]
}

/// Where the thumb and the bar are when they are off and on, and at 10 and 90.
fn rests(runtime: &mut Rt) -> ((f32, f32), (f32, f32)) {
    let off = (thumb(runtime), share(runtime));
    change(runtime, |h| {
        h.app.wifi = true;
        h.app.value = 90.0;
    });
    settle(runtime);
    let on = (thumb(runtime), share(runtime));
    change(runtime, |h| {
        h.app.wifi = false;
        h.app.value = 10.0;
    });
    settle(runtime);
    assert_eq!((thumb(runtime), share(runtime)), off, "and back");
    assert!(on.0 > off.0 && on.1 > off.1);
    (off, on)
}

fn between(now: f32, off: f32, on: f32) -> bool {
    now > off + 0.5 && now < on - 0.5
}

#[test]
fn a_switch_and_a_bar_interrupted_land_on_the_final_state() {
    let mut runtime = open_page();
    let (off, on) = rests(&mut runtime);

    // Flipped, flipped back and flipped again before they arrive: they get to the last state.
    for (wifi, value) in [(true, 90.0), (false, 30.0), (true, 90.0)] {
        change(&mut runtime, |h| {
            h.app.wifi = wifi;
            h.app.value = value;
        });
        run(&mut runtime, 2);
    }
    assert!(between(thumb(&runtime), off.0, on.0), "still moving");
    settle(&mut runtime);
    assert_eq!((thumb(&runtime), share(&runtime)), on, "odd flips: on");
    assert_nothing_running(&runtime, "after the flips");

    // A theme change halfway keeps moving from where it was, and arrives.
    change(&mut runtime, |h| {
        h.app.wifi = false;
        h.app.value = 10.0;
    });
    run(&mut runtime, 3);
    let (before, bar) = (thumb(&runtime), share(&runtime));
    assert!(between(before, off.0, on.0) && bar > off.1 && bar < on.1);
    change(&mut runtime, |h| h.set_theme(another_theme()));
    runtime.frame(DT);
    assert!(
        (thumb(&runtime) - before).abs() < (on.0 - off.0) / 2.0,
        "no restart or jump: {before} then {}",
        thumb(&runtime)
    );
    settle(&mut runtime);
    assert_eq!((thumb(&runtime), share(&runtime)), off, "and it arrives");
    assert_nothing_running(&runtime, "after the theme change");

    // Reduced motion turned on halfway: they are where they are going, at once.
    change(&mut runtime, |h| {
        h.app.wifi = true;
        h.app.value = 90.0;
    });
    run(&mut runtime, 3);
    assert!(between(thumb(&runtime), off.0, on.0));
    change(&mut runtime, |h| h.set_theme(calm()));
    run(&mut runtime, 2);
    assert_eq!(thumb(&runtime), on.0, "the thumb is at its place at once");
    // A spring of the bar's width that is already running is not cut short by Rune: it runs out
    // (about half a second) and arrives at the value. Only changes after this are at once.
    settle(&mut runtime);
    assert_eq!(share(&runtime), on.1, "the bar arrives");
    assert_nothing_running(&runtime, "after reduced motion");
}

#[test]
fn a_switch_and_a_bar_removed_mid_transition_leave_nothing_running() {
    let mut runtime = open_page();
    let (off, on) = rests(&mut runtime);
    change(&mut runtime, |h| {
        h.app.wifi = true;
        h.app.value = 90.0;
    });
    run(&mut runtime, 3);
    assert!(between(thumb(&runtime), off.0, on.0));

    change(&mut runtime, |h| h.app.present = false);
    settle(&mut runtime);
    assert_nothing_running(&runtime, "after they went");

    // They come back where their state is, not partway from where they were.
    change(&mut runtime, |h| h.app.present = true);
    runtime.frame(DT);
    assert_eq!((thumb(&runtime), share(&runtime)), on, "settled at once");
    settle(&mut runtime);
    assert_nothing_running(&runtime, "after they came back");
}

fn toast_key(runtime: &Rt) -> Option<Seen> {
    seen(runtime, "toast-0")
}

#[test]
fn a_toast_that_is_dismissed_exits_where_it_was_and_appears_at_once_with_reduced_motion() {
    let mut runtime = open_page();
    let toast = Toast::<App>::new("Saved").description("Your changes are safe.");
    let id = toast.id();
    runtime.state.app.toaster.raise(toast);
    runtime.invalidate();
    settle_for(&mut runtime, 40);
    let whole = toast_key(&runtime).expect("the toast shows");
    assert!(!whole.exiting && whole.opacity > 0.99);

    runtime.state.app.toaster.dismiss(id);
    runtime.invalidate();
    run(&mut runtime, 3);
    let closing = toast_key(&runtime).expect("it stays while it exits");
    assert!(closing.exiting, "an exit runs");
    assert_eq!(closing.rect, whole.rect, "in the place it had");
    // The wake that was asked for the toast's own time is not taken back (Rune has no way to
    // cancel one from outside a handler), so it comes once, finds nothing and ends: let the
    // toast's time pass.
    for _ in 0..40 {
        runtime.frame(0.5);
    }
    settle(&mut runtime);
    assert!(toast_key(&runtime).is_none());
    assert_nothing_running(&runtime, "after the toast went");

    // With motion reduced it is there at once, with everything in it.
    change(&mut runtime, |h| h.set_theme(calm()));
    run(&mut runtime, 2);
    let again = Toast::<App>::new("Again").description("Still safe.");
    let id = again.id();
    runtime.state.app.toaster.raise(again);
    runtime.invalidate();
    run(&mut runtime, 2);
    let faint: Vec<_> = runtime
        .elements()
        .into_iter()
        .filter(|e| e.opacity < 0.99)
        .map(|e| (e.key, e.text))
        .collect();
    assert!(toast_key(&runtime).is_some(), "it shows");
    assert!(faint.is_empty(), "still fading in: {faint:?}");
    runtime.state.app.toaster.dismiss(id);
    runtime.invalidate();
    run(&mut runtime, 2);
    assert!(toast_key(&runtime).is_none(), "and is gone at once");
}

/// A toast's clock runs from the time it shows, so frames are given a long step to run it.
fn settle_for(runtime: &mut Rt, frames: usize) {
    run(runtime, frames);
}
