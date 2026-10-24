//! Best effort: Hyprland tiles a new window to fill the screen unless a rule says otherwise,
//! and GPUI 0.2.2 cannot ask a Wayland compositor for a floating dialog. When running under
//! Hyprland the dialog asks it, through `hyprctl`, to float, size and centre the window.
//! Anything that fails is ignored: the dialog still works, tiled.
//!
//! Omarchy's own scripts do the same: try the Lua dispatcher of recent Hyprland, then the
//! older one.

use std::process::{Command, Stdio};
use std::time::Duration;

const CLASS: &str = "oma-consent";

fn dispatch(lua: &str, legacy: &[&str]) {
    let ok = Command::new("hyprctl")
        .args(["dispatch", lua])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        let _ = Command::new("hyprctl")
            .arg("dispatch")
            .args(legacy)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

/// The address and floating state of the dialog's window, once the compositor knows it.
fn find_window() -> Option<(String, bool)> {
    let output = Command::new("hyprctl")
        .args(["clients", "-j"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let clients: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    for client in clients.as_array()? {
        if client.get("class").and_then(|c| c.as_str()) == Some(CLASS) {
            let address = client.get("address")?.as_str()?.to_owned();
            let floating = client.get("floating")?.as_bool()?;
            return Some((address, floating));
        }
    }
    None
}

/// Makes the window float at `width` by `height`, centred. Runs on its own thread.
pub fn float_dialog(width: u32, height: u32) {
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        return;
    }
    std::thread::spawn(move || {
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(60));
            let Some((address, floating)) = find_window() else {
                continue;
            };
            let window = format!("address:{address}");
            if !floating {
                dispatch(
                    &format!(
                        "hl.dsp.window.float({{ window = \"{window}\", action = \"toggle\" }})"
                    ),
                    &["togglefloating", &window],
                );
            }
            dispatch(
                &format!(
                    "hl.dsp.window.resize({{ window = \"{window}\", x = {width}, y = {height} }})"
                ),
                &[
                    "resizewindowpixel",
                    &format!("exact {width} {height},{window}"),
                ],
            );
            dispatch(
                &format!("hl.dsp.window.center({{ window = \"{window}\" }})"),
                &["centerwindow"],
            );
            return;
        }
    });
}
