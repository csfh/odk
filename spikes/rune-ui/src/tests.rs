//! The spike's questions about components, answered on Rune's headless runtime.

use super::*;
use rune::{Env, Runtime};

fn runtime(state: State) -> Runtime<State> {
    let env = Env {
        system_dark: true,
        scale: 1.0,
    };
    let mut rt = Runtime::new(state, view, (480.0, 360.0), env);
    rt.frame(0.0);
    rt
}

fn focused(rt: &Runtime<State>) -> Option<String> {
    rt.elements()
        .into_iter()
        .find(|e| e.focused)
        .and_then(|e| e.key)
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
}

fn tap(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn press(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent::from(key));
    settle(rt);
}

#[test]
fn a_click_presses_the_button() {
    let mut rt = runtime(State::default());
    tap(&mut rt, "save");
    assert_eq!(rt.state.saves, 1);
}

#[test]
fn tab_then_enter_and_space_press_the_button() {
    let mut rt = runtime(State::default());
    press(&mut rt, Key::Tab);
    assert_eq!(
        focused(&rt),
        Some("save".into()),
        "Tab reaches the button first"
    );
    for key in [Key::Enter, Key::Text(" ".into())] {
        let before = rt.state.saves;
        press(&mut rt, key.clone());
        press(&mut rt, key.clone());
        assert_eq!(
            rt.state.saves, before,
            "{key:?} held down has not pressed yet"
        );
        rt.key_up(KeyEvent::from(key.clone()));
        settle(&mut rt);
        assert_eq!(rt.state.saves, before + 1, "{key:?}'s release presses once");
    }
}

#[test]
fn the_select_opens_and_a_click_chooses() {
    let mut rt = runtime(State::default());
    tap(&mut rt, "fruit-trigger");
    assert!(rt.state.open);
    assert!(rt.find("fruit-2").is_some(), "the listbox is drawn");
    let (_, ty) = rt.find("fruit-trigger").unwrap();
    let (_, oy) = rt.find("fruit-0").unwrap();
    assert!(oy > ty, "it opens under its trigger");
    tap(&mut rt, "fruit-2");
    assert_eq!(rt.state.fruit, Some(2));
    assert!(!rt.state.open);
}

#[test]
fn escape_closes_the_select_and_focus_goes_back_to_its_trigger() {
    let mut rt = runtime(State::default());
    press(&mut rt, Key::Tab);
    press(&mut rt, Key::Tab);
    assert_eq!(focused(&rt), Some("fruit-trigger".into()));
    press(&mut rt, Key::Down);
    assert!(rt.state.open, "Down opens it");
    press(&mut rt, Key::Escape);
    assert!(!rt.state.open, "Escape closes it");
    assert_eq!(focused(&rt), Some("fruit-trigger".into()));
}

#[test]
fn arrows_and_enter_choose_in_the_listbox() {
    let mut rt = runtime(State::default());
    tap(&mut rt, "fruit-trigger");
    press(&mut rt, Key::Down);
    press(&mut rt, Key::Down);
    press(&mut rt, Key::Up);
    press(&mut rt, Key::Enter);
    assert_eq!(rt.state.fruit, Some(1));
}

#[test]
fn a_click_outside_closes_the_select() {
    let mut rt = runtime(State::default());
    tap(&mut rt, "fruit-trigger");
    rt.pointer_down(460.0, 340.0);
    rt.pointer_up(460.0, 340.0);
    settle(&mut rt);
    assert!(!rt.state.open);
    assert_eq!(rt.state.fruit, None);
}

#[test]
fn typing_edits_the_input_and_undo_takes_it_back() {
    let mut rt = runtime(State::default());
    tap(&mut rt, "name");
    for c in "Ada".chars() {
        press(&mut rt, Key::Text(c.to_string()));
    }
    assert_eq!(rt.state.name.text, "Ada");
    press(&mut rt, Key::Backspace);
    assert_eq!(rt.state.name.text, "Ad");
}

#[test]
fn a_screen_reader_sees_roles_names_and_values() {
    let mut rt = runtime(State {
        fruit: Some(0),
        ..State::default()
    });
    settle(&mut rt);
    let tree = rt.accessibility_json().to_string();
    for needle in ["\"button\"", "Save", "combo-box", "Fruit", "Apple", "Name"] {
        assert!(tree.contains(needle), "{needle} missing from {tree}");
    }
}

#[test]
fn the_text_size_scales_controls() {
    let small = runtime(State::default());
    let large = runtime(State {
        host: Host {
            base_size: 18.0,
            ..Host::default_host()
        },
        ..State::default()
    });
    let height = |rt: &Runtime<State>| {
        let ([_, _, _, height], _) = rt.presented("save").unwrap();
        height
    };
    assert!(
        (height(&large) / height(&small) - 1.5).abs() < 0.01,
        "18 px is half as tall again as 12 px"
    );
}

#[test]
fn reduced_motion_from_odkd_wins_over_the_host() {
    let mut rt = runtime(State {
        host: Host {
            reduced_motion: true,
            ..Host::default_host()
        },
        ..State::default()
    });
    rt.set_system_reduced_motion(false);
    settle(&mut rt);
    assert!(
        rt.reduced_motion,
        "odkd says reduce: the host's no does not undo it"
    );

    let mut rt = runtime(State::default());
    rt.set_system_reduced_motion(true);
    settle(&mut rt);
    assert!(
        !rt.reduced_motion,
        "odkd says no: GNOME's or KDE's yes is ignored"
    );
}

#[test]
fn an_idle_window_asks_for_no_frames() {
    let mut rt = runtime(State::default());
    settle(&mut rt);
    let wants = rt.frame(1.0 / 60.0);
    assert!(!wants, "nothing animates, so nothing is redrawn");
}

/// Sans text, for measuring which family the sans slot holds.
fn sans(_: &State, _: &mut Cx<State>) -> El {
    row().child(
        text("Saved 0 times, the quick brown fox")
            .key("probe")
            .font(TextStyle::new(14.0, 400.0)),
    )
}

fn probe_width(families: Option<&str>) -> f32 {
    let env = Env {
        system_dark: true,
        scale: 1.0,
    };
    let mut rt = Runtime::new(State::default(), sans, (480.0, 360.0), env);
    if let Some(f) = families {
        rt.set_font_families(f, "monospace");
    }
    rt.frame(0.0);
    rt.presented("probe").unwrap().0[2]
}

#[test]
fn an_installed_family_rune_was_not_given_is_not_found() {
    // C059 is a serif with no metric match to Inter, so a hit would change the width.
    let bundled = probe_width(None);
    let installed = probe_width(Some("C059"));
    let unknown = probe_width(Some("No Such Family"));
    let named_bundled = probe_width(Some("Inter"));
    let mono = probe_width(Some("JetBrainsMono Nerd Font"));
    println!(
        "default {bundled}, Inter {named_bundled}, C059 {installed}, JetBrainsMono {mono}, unknown {unknown}"
    );
    assert_eq!(
        named_bundled, bundled,
        "the control: naming the bundled family keeps it"
    );
    assert_eq!(
        installed, unknown,
        "C059 falls back as an unknown family does"
    );
}
