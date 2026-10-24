//! What each component drew on its last frame, for the checklist's tests: the style it looked up
//! from the component tokens, by the part name tests find it with. Kept per thread, like the test
//! theme. Components with a probe of their own (`button::probe`, `input::drawn`, ...) predate
//! this one.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static DRAWN: RefCell<HashMap<String, Box<dyn Any>>> = RefCell::new(HashMap::new());
}

/// Records what the part named `part` drew.
pub(crate) fn record<T: Any>(part: &str, drawn: T) {
    DRAWN.with(|d| d.borrow_mut().insert(part.to_owned(), Box::new(drawn)));
}

/// What the part named `part` drew last. Panics if it drew nothing, or something else.
pub(crate) fn last<T: Any + Clone>(part: &str) -> T {
    DRAWN.with(|d| {
        d.borrow()
            .get(part)
            .unwrap_or_else(|| panic!("no part {part:?} has been drawn"))
            .downcast_ref::<T>()
            .unwrap_or_else(|| panic!("part {part:?} drew something else"))
            .clone()
    })
}
