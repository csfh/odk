//! What a checkbox and a switch share: the row that puts the control beside its label, so a
//! press on the label toggles the control, and Space toggles it from the keyboard.
//!
//! The row is the control's one tab stop and its accessible element; the box or track inside it
//! is only drawn, and carries the focus ring while the row has the keyboard's focus. State is
//! the application's: a toggle shows what it is given and says what it would become.

use std::rc::Rc;

use crate::button::activator;
use crate::describe::apply;
use crate::host::Hosted;
use crate::icon::Icon;
use crate::ink::ink;
use crate::layout::inline;
use crate::store::Activator;
use crate::style::Paint;
use crate::text::{Flow, words};
use odk_a11y::Accessible;
use odk_theme::Color;
use odk_theme::foundation::TypeRole;
use rune::{El, KeyEvent, Tx, row};

/// What a press does: it works out the state the toggle would make, keeps it when the toggle
/// holds its own, and tells the application.
pub(crate) type Toggle<T> = Rc<dyn Fn(&mut Hosted<T>)>;

/// What a checkbox or a switch tells the application: the state a press would make.
pub(crate) type Change<T> = Rc<dyn Fn(&mut T, bool)>;

/// How a control's label is drawn, from the control's component style, and which keys toggle it.
pub(crate) struct Look {
    pub colour: Color,
    pub role: TypeRole,
    /// Between the control and its label.
    pub gap: f32,
    /// The colour of a wrong choice's mark after the label, so it never shows by colour alone.
    pub mark: Option<Color>,
    /// Whether Enter toggles it as well as Space: a switch does, a checkbox does not.
    pub enter: bool,
}

/// `control` beside `label`: a row as wide as its content and no wider than its column, so a
/// long label wraps with the control centred on its first line. A press on the row, or Space
/// (and Enter, with `enter`) released on it, runs `toggle`.
pub(crate) fn labelled<T: 'static>(
    paint: &Paint,
    accessible: &Accessible,
    id: &str,
    label: String,
    control: El,
    look: Look,
    toggle: Toggle<T>,
) -> El {
    let usable = !accessible.disabled;
    let enter = look.enter;
    let line = paint.role(look.role).line_height;
    // The row is the pointer target. Pad it when the text line is shorter than the minimum
    // target, so the control stays centred on the first line and the target still meets it.
    let target = paint.tokens.foundation().geometry.target_min;
    let pad = ((target - line) / 2.0).max(0.0);
    let mut labelled = apply(row().key(id.to_string()), accessible)
        .items_start()
        // A floor on the height only: a width would replace the row's content width.
        .min_h(target)
        .pt(pad)
        .pb(pad)
        .gap(look.gap)
        .focusable(usable)
        // The control draws the ring, as the row is only where the focus sits.
        .no_focus_ring()
        .shrink()
        .child(row().h(line).center_y().child(control))
        .child(words(paint, label, look.role, look.colour, Flow::Wrap));
    if let Some(colour) = look.mark {
        labelled = labelled.child(
            row()
                .h(line)
                .center_y()
                .child(Icon::Warning.mark(paint.role(look.role).size, ink(colour))),
        );
    }
    // As wide as its content and no wider than its column, so a long label wraps beside the box.
    if !usable {
        return inline(labelled);
    }
    let press = toggle.clone();
    let release = toggle;
    let key_id = id.to_string();
    let up_id = id.to_string();
    inline(
        labelled
            .on_press(move |hosted: &mut Hosted<T>| press(hosted))
            .on_key(move |hosted: &mut Hosted<T>, ev: &KeyEvent, tx: &mut Tx| {
                // Space toggles a checkbox and a switch; Enter toggles a switch only (WAI-ARIA).
                // Rune presses a focused control on Space's press, so take the key here and act
                // on its release.
                let Some(key) = toggling(ev, enter) else {
                    return false;
                };
                if !tx.key_repeat() {
                    let slot = hosted.ui.button_mut(&key_id);
                    if slot.armed.is_none() {
                        slot.armed = Some(key);
                    }
                }
                true
            })
            .on_key_up(move |hosted: &mut Hosted<T>, ev: &KeyEvent, _: &mut Tx| {
                let Some(key) = toggling(ev, enter) else {
                    return false;
                };
                let slot = hosted.ui.button_mut(&up_id);
                if slot.armed != Some(key) {
                    return false;
                }
                slot.armed = None;
                release(hosted);
                true
            }),
    )
}

/// The key that toggles: Space, and Enter where `enter` says so.
fn toggling(ev: &KeyEvent, enter: bool) -> Option<Activator> {
    activator(ev).filter(|key| enter || *key == Activator::Space)
}
