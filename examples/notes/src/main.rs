//! Runs the notes application: `odk oma dev examples/notes`, or by hand under a session.

use notes::{Notes, data_dir, view};
use odk::ui::{WindowConfig, run};

fn main() {
    let window = WindowConfig::new((560.0, 360.0)).title("Notes");
    if let Err(error) = run(Notes::new(data_dir()), window, view) {
        eprintln!("notes: {error}");
        std::process::exit(1);
    }
}
