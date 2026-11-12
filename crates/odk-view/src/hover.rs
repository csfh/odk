//! What a tooltip and a hover card share: a panel beside its trigger that opens after the pointer
//! rests on the trigger, stays while it is on the panel, and closes after it leaves both.

use crate::host::Hosted;
use crate::placement::{Align, Placement};
use crate::store::Part;
use crate::style::Paint;
use rune::{El, Key, KeyEvent, Tx, row};
use std::time::Duration;

/// What a hover panel is: its id, which side and edge of the trigger it prefers, and the panel.
pub(crate) struct Beside<'a> {
    pub id: &'a str,
    pub placement: Placement,
    pub align: Align,
    pub panel: El,
}

/// The panel beside `trigger`, which `layers` takes while it is open. `trigger` sits in a wrapper
/// the size of its content, which is what the pointer rests on and the panel is placed beside.
/// Escape closes it. The delays are the foundation's `hover_open` and `hover_close`.
pub(crate) fn hover<T: 'static>(
    hosted: &Hosted<T>,
    paint: &Paint,
    beside: Beside<'_>,
    trigger: El,
    layers: &mut Vec<El>,
) -> El {
    let Beside {
        id,
        placement,
        align,
        panel,
    } = beside;
    let motion = &paint.tokens.foundation().motion;
    let (opens, closes) = (motion.hover_open, motion.hover_close);
    let gap = paint.tokens.foundation().spacing.s1;
    let open = hosted.ui.hover_open(id);
    let (enter, leave, escape) = (id.to_string(), id.to_string(), id.to_string());
    let on_hover = move |hosted: &mut Hosted<T>, inside: bool, _: &mut Tx| {
        hosted.ui.hover_set(&enter, Part::Trigger, inside);
    };
    let wrapper = row()
        .key(id.to_string())
        .self_start()
        .hover_delays(opens, closes)
        .on_hover_change(on_hover)
        .on_key(move |hosted: &mut Hosted<T>, ev: &KeyEvent, _: &mut Tx| {
            if ev.key != Key::Escape || !hosted.ui.hover_open(&escape) {
                return false;
            }
            hosted.ui.hover_dismiss(&escape);
            true
        })
        .child(trigger);
    if open {
        // It stays while the pointer is on it, so a pointer that moves from the trigger onto the
        // panel does not close it (WCAG 1.4.13).
        layers.push(
            paint
                .present(panel)
                // The pointer that has crossed from the trigger to the panel keeps it open at
                // once; leaving it closes after the same wait.
                .hover_delays(Duration::ZERO, closes)
                .on_hover_change(move |hosted: &mut Hosted<T>, inside: bool, _: &mut Tx| {
                    hosted.ui.hover_set(&leave, Part::Panel, inside);
                })
                .anchored(id.to_string(), placement.side(), align.rune(), gap),
        );
    }
    wrapper
}
