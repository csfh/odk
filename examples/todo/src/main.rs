//! A to-do list: the second example application. See `lib.rs` for what it shows.

use odk::prelude::*;
use todo::{Store, Todo};

fn main() {
    // Read before the window opens, as the theme is: one query, and the list is in memory.
    let store = Store::open(todo::default_store().as_deref());
    odk::ui::run(|cx| {
        odk::ui::open_window(cx, WindowOptions::default(), |_, cx| {
            cx.new(|_| Todo::new(store))
        })
        .unwrap();
    });
}
