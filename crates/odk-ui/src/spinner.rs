//! An indeterminate activity indicator, on component tokens (`docs/spec/odk-component-tokens.md`):
//! one implementation, standing alone or inside a `Button` or a field.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use gpui::{
    App, ElementId, EntityId, Global, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    Styled, Window, canvas, div, px,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::SpinnerStyle;
use odk_theme::foundation::Density;

use crate::focus::part_name;
use crate::theme::{colour, mono};
use crate::{Size, components};

/// The frames of one turn: a braille dot circling, which suits the monospace look.
const FRAMES: [&str; 8] = ["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];
/// The least a spinner waits for its next glyph. ODK's own, not a design token: a timer that
/// fires before the glyph has changed redraws for nothing, and a clock that lands exactly on a
/// glyph's edge must not ask for a wait of zero.
const LEAST_WAIT: Duration = Duration::from_millis(1);
/// Shown instead of the animation when the user asked for reduced motion.
const STILL: &str = "⣿";

/// An indeterminate activity indicator: work is going on, with no promise of how much is left.
/// A square of the density's icon size, in secondary text standing alone and in the foreground
/// of the control it is in (a `Button`'s or a field's). Turns once per the foundation's `cycle`,
/// and stands still, still visible, when motion is reduced. Its accessible name says what is
/// loading ("Loading" by default), so it never relies on its movement.
#[derive(IntoElement)]
pub struct Spinner {
    id: ElementId,
    size: Size,
    density: Option<Density>,
    /// A size in logical pixels from the control it is in, which wins over `size`: a spinner
    /// inside a control is as large as the control's icons and takes its colour.
    glyph: Option<f32>,
    label: String,
}

impl Spinner {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            size: Size::default(),
            density: None,
            glyph: None,
            label: "Loading".into(),
        }
    }

    /// A step from the density it would otherwise take, as for a `Button`: `Sm` is the icon size
    /// one density tighter, `Lg` one roomier.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Its own density, whatever the application's and whatever its `size`.
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }

    /// Draws it `size` logical pixels high and wide in the colour of the control around it: the
    /// `spinner_size` of the control it is in.
    pub(crate) fn glyph_size(mut self, size: f32) -> Self {
        self.glyph = Some(size);
        self
    }

    /// The accessible name; "Loading" by default.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }
}

/// Whether the spinner turns: not when motion is reduced (checklist item 20).
pub(crate) fn animates(s: &SpinnerStyle) -> bool {
    !s.still && !s.turn.is_zero()
}

/// The frame shown at `delta`, from 0 to 1 through a turn.
fn frame(delta: f32) -> &'static str {
    FRAMES[frame_index(delta)]
}

fn frame_index(delta: f32) -> usize {
    ((delta * FRAMES.len() as f32) as usize).min(FRAMES.len() - 1)
}

/// What every spinner shares: the moment their turns count from, so they all show the same glyph
/// at the same time, and the views that have a redraw waited for.
///
/// A spinner is a function of GPUI's clock, so it needs no frame to find out what to show; it only
/// needs to be drawn again when the glyph changes, `FRAMES.len()` times a turn, not at every frame
/// the display could show (a frame callback re-renders the whole path of views from the window
/// down to the spinner, 60 times a second). One timer per view serves all the spinners in it, and
/// they turn in step, so a page of loading buttons costs the same as one.
#[derive(Default)]
struct Ticks {
    since: Option<Instant>,
    waiting: HashSet<EntityId>,
}

impl Global for Ticks {}

impl Ticks {
    /// How far through a turn of `turn` it is at `now`, from 0 up to but not including 1, and how
    /// long until the glyph after the current one.
    fn at(&mut self, now: Instant, turn: Duration) -> (f32, Duration) {
        let since = *self.since.get_or_insert(now);
        let step = (turn / FRAMES.len() as u32).as_nanos().max(1);
        let into = now.saturating_duration_since(since).as_nanos() % turn.as_nanos().max(1);
        let delta = into as f32 / turn.as_nanos().max(1) as f32;
        let next = Duration::from_nanos((step - into % step) as u64).max(LEAST_WAIT);
        (delta, next)
    }
}

/// Redraws `view` after `after`, unless a redraw of it is already waited for.
fn wake(view: EntityId, after: Duration, cx: &mut App) {
    if !cx.default_global::<Ticks>().waiting.insert(view) {
        return;
    }
    let timer = cx.background_executor().timer(after);
    cx.spawn(async move |cx| {
        timer.await;
        let _ = cx.update(|cx| {
            cx.default_global::<Ticks>().waiting.remove(&view);
            cx.notify(view);
        });
    })
    .detach();
}

impl Describe for Spinner {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Progress, self.label.clone()).busy(true)
    }
}

impl RenderOnce for Spinner {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = components();
        let density = self
            .density
            .unwrap_or_else(|| self.size.density(c.density(None)));
        let s = c.spinner(density);
        #[cfg(test)]
        crate::drawn::record(&crate::focus::id_path(&self.id), s);
        let size = px(self.glyph.unwrap_or(s.size));
        let name = part_name(&self.id, "spinner");
        let mut base = div()
            .debug_selector(|| name)
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .size(size)
            .font_family(mono())
            .text_size(size)
            .line_height(size);
        // Inside a control it keeps the control's foreground.
        if self.glyph.is_none() {
            base = base.text_color(colour(s.colour));
        }
        if !animates(&s) {
            return base.child(STILL).into_any_element();
        }
        let now = cx.background_executor().now();
        let (delta, next) = cx.default_global::<Ticks>().at(now, s.turn);
        // A spinner nobody can see, scrolled or clipped out of view, asks for nothing; scrolling
        // it back in redraws its view and so draws it again. The canvas learns where the spinner
        // is, which only the layout knows.
        let view = window.current_view();
        let seen = canvas(
            move |bounds, window, cx| {
                if window.content_mask().bounds.intersects(&bounds) {
                    wake(view, next, cx);
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        base.relative()
            .child(frame(delta))
            .child(seen)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_the_frames_once_per_turn() {
        assert_eq!(frame(0.0), FRAMES[0]);
        assert_eq!(frame(0.5), FRAMES[4]);
        assert_eq!(frame(1.0), FRAMES[7]);
    }
}
