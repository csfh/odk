//! The window: one theme, a component store, and a list of layers above the page.

use odk_theme::ThemeManager;
use odk_theme::ThemeSnapshot;
use rune::text::TextSystem;
use rune::{App, AppSpec, Cx, El, Env, Runtime, Tx, Waker, el};

use crate::face::Face;
use crate::ink::ink;
use crate::store::Store;

/// The application's model beside the theme and the component store.
pub struct Hosted<T> {
    pub app: T,
    pub ui: Store,
    theme: ThemeSnapshot,
    face: Face,
}

impl<T> Hosted<T> {
    pub fn new(app: T, theme: ThemeSnapshot, face: Face) -> Self {
        Self {
            app,
            ui: Store::default(),
            theme,
            face,
        }
    }

    pub fn theme(&self) -> &ThemeSnapshot {
        &self.theme
    }

    /// Draws with `theme` from the next frame: the user changed the palette, the text size or the
    /// request for reduced motion.
    pub fn set_theme(&mut self, theme: ThemeSnapshot) {
        self.theme = theme;
    }
}

/// What the host does at the start of every frame, on Rune's clock: turn the spinners, and run the
/// view again when an image it was waiting for has settled.
///
/// It asks for nothing while no spinner was in the last view built, so a window that shows
/// none is never woken (checklist item 35). With one, it wakes at the moment the glyph changes,
/// not at every frame, and says the state changed only when it did, so the view runs again
/// `FRAMES` times a turn and not 60 times a second.
fn poll<T>(hosted: &mut Hosted<T>, now: f64, tx: &mut Tx) -> bool {
    // Both run, so a turn of the spinners never hides an image that has settled.
    let turned = crate::spinner::turn(hosted, now, tx);
    let settled = hosted.ui.images.settled();
    let charted = crate::chart::advance(hosted, now, tx);
    let toasted = hosted.ui.step_toasts(now, tx);
    turned | settled | charted | toasted
}

/// Gives the window's images the waker that Rune supplies, so a finished decode wakes it.
fn setup<T>(hosted: &mut Hosted<T>, waker: Waker) {
    hosted.ui.images.connect(waker);
}

/// The page, then `layers`, on the theme's canvas.
///
/// `odkd`'s reduced motion is applied on every commit, which wins over the motion Rune would
/// read from the desktop. A popup is one entry of `layers`, after the page, so a dialog can
/// carry a menu without a separate host.
pub fn frame<T>(
    hosted: &Hosted<T>,
    cx: &mut Cx<Hosted<T>>,
    body: El,
    layers: impl IntoIterator<Item = El>,
) -> El {
    // The page is built, so what it held is what the poll turns, until the next build.
    hosted.ui.end_build();
    cx.reduced_motion(hosted.theme.reduced_motion);
    el().key("odk-root")
        .w_full()
        .h_full()
        .fill(ink(hosted.theme.semantic.surface.canvas))
        .child(body)
        .children(layers)
}

fn spec_init<T>() -> Hosted<T> {
    unreachable!("the host passes the state through App::init")
}

/// Open the window. Rune's `run` panics when the compositor goes away (`app.rs`, `try_run`'s
/// error). This returns that error so the process can exit.
///
/// Rune reads no process arguments here. A caller that wants a headless snapshot passes them
/// through a later, explicit path. Tests drive [`headless`] and do not call this.
pub fn start<T: 'static>(
    hosted: Hosted<T>,
    view: fn(&Hosted<T>, &mut Cx<Hosted<T>>) -> El,
    size: (f32, f32),
) -> Result<(), String> {
    let spec = AppSpec {
        fonts: hosted.face.fonts(),
        families: hosted.face.families(),
        poll: Some(poll::<T>),
        setup: Some(setup::<T>),
        ..AppSpec::new("ODK", size, spec_init::<T>, view)
    };
    App::new(spec)
        .args(None::<String>)
        .init(move || hosted)
        .tab_visits_controls(true)
        .try_run()
}

/// Open the window on the user's theme and keep it up to date: the palette when the theme changes,
/// the text size, and `odkd`'s request for reduced motion, each from the next frame. With no
/// theme it draws the emergency palette (ADR 0041). The font is `face` when the caller names
/// one, and otherwise the family `odkd` reports, found in the fonts the sandbox shows
/// ([`Face::installed`], ADR 0095), and Rune's bundled one when there is none. Rune loads a family
/// from bytes before the window opens and not after, so a font changed while the window is open
/// applies at the next start (`docs/checks.md`).
pub fn run<T: 'static>(
    app: T,
    face: Option<Face>,
    view: fn(&Hosted<T>, &mut Cx<Hosted<T>>) -> El,
    size: (f32, f32),
) -> Result<(), String> {
    let manager = ThemeManager::current();
    let face = face.unwrap_or_else(|| {
        manager
            .font_family()
            .and_then(Face::installed)
            .unwrap_or(Face::bundled())
    });
    let hosted = Hosted::new(app, (*manager.snapshot()).clone(), face);
    let spec = AppSpec {
        fonts: hosted.face.fonts(),
        families: hosted.face.families(),
        poll: Some(poll::<T>),
        setup: Some(setup::<T>),
        ..AppSpec::new("ODK", size, spec_init::<T>, view)
    };
    let app = App::new(spec)
        .args(None::<String>)
        .init(move || hosted)
        .tab_visits_controls(true);
    let handle = app.handle();
    // Kept until the window closes.
    let _follower = crate::follow::follow(manager, move |theme| {
        handle.update(move |hosted: &mut Hosted<T>, _| hosted.set_theme(theme));
    });
    app.try_run()
}

/// One headless frame of `view`, with `hosted`'s font bytes loaded first.
pub fn headless<T: 'static>(
    hosted: Hosted<T>,
    view: fn(&Hosted<T>, &mut Cx<Hosted<T>>) -> El,
    size: (f32, f32),
) -> Runtime<Hosted<T>> {
    let text = TextSystem::new(hosted.face.fonts(), hosted.face.families());
    let mut runtime = Runtime::with_text(hosted, view, size, Env::new(true, 1.0), text);
    runtime.poll = Some(poll::<T>);
    runtime.frame(0.0);
    runtime
}
