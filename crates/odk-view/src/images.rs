//! The decoded images a window holds, for `Image` (ADR 0072).
//!
//! Rune decodes on its own threads and keeps a decoded image only while a handle to it lives,
//! so each `Image` keeps its handle here, with the bytes it was made from. The same bytes again
//! (the same `Arc`) are neither hashed nor decoded again.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use rune::{ImageCache, Waker};

#[derive(Default)]
pub(crate) struct Images {
    inner: RefCell<Inner>,
}

#[derive(Default)]
struct Inner {
    cache: Option<ImageCache>,
    slots: HashMap<String, Slot>,
}

struct Slot {
    bytes: Arc<[u8]>,
    image: rune::Image,
    /// Whether the last build drew this image still pending, so the poll has one to watch.
    waiting: bool,
}

/// Where an image stands.
pub(crate) enum Load {
    /// Rune has not finished decoding it.
    Pending,
    Ready(rune::Image),
    /// The bytes are not an image Rune can decode.
    Failed,
}

impl Images {
    /// Makes the cache with the waker Rune supplies in `AppSpec::setup`, so a finished decode
    /// wakes the window.
    pub(crate) fn connect(&self, waker: Waker) {
        let mut inner = self.inner.borrow_mut();
        if inner.cache.is_none() {
            inner.cache = Some(ImageCache::new(waker));
        }
    }

    /// The image `bytes` make for the component `id`.
    pub(crate) fn load(&self, id: &str, bytes: &Arc<[u8]>) -> Load {
        let mut inner = self.inner.borrow_mut();
        let Inner { cache, slots } = &mut *inner;
        // A window that was never started, such as a headless one, has no waker to wake.
        let cache = cache.get_or_insert_with(|| ImageCache::new(Arc::new(|| {})));
        let stale = slots
            .get(id)
            .is_none_or(|slot| !Arc::ptr_eq(&slot.bytes, bytes));
        if stale {
            slots.insert(
                id.to_string(),
                Slot {
                    bytes: bytes.clone(),
                    image: cache.load(bytes.clone()),
                    waiting: false,
                },
            );
        }
        let slot = slots.get_mut(id).expect("the slot was just made");
        let load = if slot.image.error().is_some() {
            Load::Failed
        } else if slot.image.is_ready() {
            Load::Ready(slot.image.clone())
        } else {
            Load::Pending
        };
        slot.waiting = matches!(load, Load::Pending);
        load
    }

    /// Whether an image the last build waited for has settled, ready or failed, so the view
    /// runs again. Asks for nothing while none is waiting (checklist item 35).
    pub(crate) fn settled(&self) -> bool {
        let mut inner = self.inner.borrow_mut();
        let mut any = false;
        for slot in inner.slots.values_mut() {
            if slot.waiting && slot.image.is_ready() {
                slot.waiting = false;
                any = true;
            }
        }
        any
    }
}
