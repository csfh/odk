//! `oma-consent`: shows what an application asks for and prints the user's answer.
//!
//! It decides nothing and keeps nothing (ADR 0025). The request is TOML on standard input and
//! the answer is one word on standard output, as in `docs/spec/consent-dialog.md`. It links Rune
//! directly (ADR 0089) and depends on neither `oma` nor ODK (ADR 0016).

mod dialog;
mod hyprland;
mod request;
mod theme;

use std::io::Read;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use rune::{App, AppSpec, WindowOptions};

use dialog::{Answer, Dialog, WIDTH};
use request::Request;
use theme::Theme;

fn spec_init() -> Dialog {
    unreachable!("the dialog is passed through App::init")
}

fn main() -> ExitCode {
    let mut text = String::new();
    if std::io::stdin().read_to_string(&mut text).is_err() {
        eprintln!("oma-consent: cannot read the request");
        return ExitCode::from(2);
    }
    let request = match Request::parse(&text) {
        Ok(request) => request,
        Err(e) => {
            eprintln!("oma-consent: {e}");
            return ExitCode::from(2);
        }
    };

    let height = request.height();
    // The title names the application, and the process lives for one dialog.
    let title: &'static str = Box::leak(format!("Allow {}?", request.name).into_boxed_str());
    let answer: Answer = Arc::new(Mutex::new(None));
    let dialog = Dialog::new(request, Theme::load(), answer.clone());

    rune::set_app_id("oma-consent");
    let spec = AppSpec {
        on_key: Some(Dialog::on_key),
        ..AppSpec::new(title, (WIDTH, height), spec_init, Dialog::view)
    };
    // The window is small and fixed in size.
    let options = WindowOptions {
        resizable: false,
        min_size: Some((WIDTH, height)),
        max_size: Some((WIDTH, height)),
        ..WindowOptions::default()
    };
    hyprland::float_dialog(WIDTH as u32, height as u32);
    let result = App::new(spec)
        .args(None::<String>)
        .init(move || dialog)
        .window_options(options)
        .tab_visits_controls(true)
        .try_run();
    if let Err(e) = result {
        eprintln!("oma-consent: cannot open the window: {e}");
        return ExitCode::from(2);
    }

    // Closing the window without choosing is Cancel.
    let given = answer
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_else(|| "cancel".to_owned());
    println!("{given}");
    ExitCode::SUCCESS
}
