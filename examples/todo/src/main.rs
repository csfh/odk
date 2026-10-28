//! A to-do list: the second example application. See `lib.rs` for what it shows.

use odk::prelude::*;
use todo::{Store, Todo};

fn main() {
    // Read before the window opens, as the theme is: one query, and the list is in memory.
    let store = Store::open(todo::default_store().as_deref());
    odk::ui::run(|cx| {
        let window = WindowConfig::new("com.example.todo")
            .title("Todo")
            .size(480.0, 600.0)
            .min_size(320.0, 320.0);
        odk::ui::open_window(cx, window, |_, cx| cx.new(|_| Todo::new(store))).unwrap();
    });
}
