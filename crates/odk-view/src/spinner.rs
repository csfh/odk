//! An indeterminate activity indicator on the Rune host (`docs/spec/odk-component-tokens.md`):
//! one implementation, standing alone or inside a `Button` or a field.

use std::time::Duration;

use crate::describe::apply;
use crate::host::Hosted;
use crate::ink::ink;
use crate::style;
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::foundation::Density;
use rune::{El, Family, TextStyle, Tx, el, text};

/// The frames of one turn: a braille dot circling, which suits the monospace look.
const FRAMES: [&str; 8] = ["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];
/// The least a spinner waits for its next glyph. ODK's own, not a design token: a wake that
/// comes before the glyph has changed redraws for nothing, and a clock that lands exactly on a
/// glyph's edge must not ask for a wait of zero.
const LEAST_WAIT: Duration = Duration::from_millis(1);
/// Shown instead of the animation when the user asked for reduced motion.
const STILL: &str = "⣿";
/// A glyph's weight.
const WEIGHT: f32 = 400.0;

/// An indeterminate activity indicator: work is going on, with no promise of how much is left. A
/// square of the density's icon size, in secondary text. It turns once per the foundation's
/// `cycle`, and stands still, still visible, when motion is reduced. Its accessible name says
/// what is loading ("Loading" by default), so it never relies on its movement.
///
/// The turning is the host's: while a spinner is built, its poll wakes at each new glyph and no
/// oftener, and while none is, it asks for nothing.
pub struct Spinner {
    density: Option<Density>,
    label: String,
}

impl Spinner {
    pub fn new() -> Self {
        Self {
            density: None,
            label: "Loading".into(),
        }
    }

    /// Its own density, whatever the application's.
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }

    /// The accessible name; "Loading" by default.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn show<T>(self, hosted: &Hosted<T>) -> El {
        let accessible = self.accessible();
        let paint = style::paint(hosted);
        let s = paint.tokens.spinner(paint.tokens.density(self.density));
        let size = s.size;
        let shown = if animates(s.still, s.turn) {
            // A spinner in the tree is what the host's poll turns.
            hosted.ui.note_spinner();
            FRAMES[hosted.ui.spinner_frame().min(FRAMES.len() - 1)]
        } else {
            STILL
        };
        apply(el(), &accessible).size(size, size).center().child(
            text(shown)
                .font(TextStyle {
                    family: Family::Mono,
                    line_height: size,
                    ..TextStyle::new(size, WEIGHT)
                })
                .color(ink(s.colour))
                .a11y_hidden(),
        )
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl Describe for Spinner {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Progress, self.label.clone()).busy(true)
    }
}

/// Whether the spinner turns: not when motion is reduced (checklist item 20).
fn animates(still: bool, turn: Duration) -> bool {
    !still && !turn.is_zero()
}

/// The glyph `into` seconds after the turns began, and how long until the one after it.
pub(crate) fn position(into: f64, turn: Duration) -> (usize, Duration) {
    let turn = turn.as_secs_f64();
    let step = turn / FRAMES.len() as f64;
    let into = into.max(0.0) % turn;
    let index = ((into / turn * FRAMES.len() as f64) as usize).min(FRAMES.len() - 1);
    let next = Duration::from_secs_f64((step - into % step).max(0.0)).max(LEAST_WAIT);
    (index, next)
}

/// The host's turn of every spinner at `now`: a wake for the next glyph, and whether the glyph
/// is a new one, so the view runs again only then.
pub(crate) fn turn<T>(hosted: &mut Hosted<T>, now: f64, tx: &mut Tx) -> bool {
    if !hosted.ui.spinning() {
        return false;
    }
    let paint = style::paint(hosted);
    let s = paint.tokens.spinner(paint.tokens.density(None));
    if !animates(s.still, s.turn) {
        return false;
    }
    let since = hosted.ui.spinner_epoch(now);
    let (index, next) = position(now - since, s.turn);
    tx.wake_after(next);
    hosted.ui.set_spinner_frame(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_the_frames_once_per_turn_and_waits_for_the_next() {
        let turn = Duration::from_millis(800);
        assert_eq!(position(0.0, turn).0, 0);
        assert_eq!(position(0.4, turn).0, 4);
        assert_eq!(position(0.79, turn).0, 7);
        assert_eq!(position(0.8, turn).0, 0, "and round again");
        // A frame is 100 ms; 30 ms into one leaves 70 ms.
        let (_, next) = position(0.03, turn);
        assert!((next.as_secs_f64() - 0.07).abs() < 1e-6, "{next:?}");
        // On a glyph's edge the wait is a whole step, never zero.
        assert!(position(0.1, turn).1 >= Duration::from_millis(99));
    }

    #[test]
    fn a_turn_of_no_time_or_a_still_spinner_does_not_turn() {
        assert!(!animates(true, Duration::from_millis(800)));
        assert!(!animates(false, Duration::ZERO));
        assert!(animates(false, Duration::from_millis(800)));
    }
}
