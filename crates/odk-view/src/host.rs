//! The window: one theme, a component store, and a list of layers above the page.

use odk_theme::ThemeSnapshot;
use rune::text::TextSystem;
use rune::{App, AppSpec, Cx, El, Env, Runtime, el};

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
        ..AppSpec::new("ODK", size, spec_init::<T>, view)
    };
    App::new(spec)
        .args(None::<String>)
        .init(move || hosted)
        .tab_visits_controls(true)
        .try_run()
}

/// One headless frame of `view`, with `hosted`'s font bytes loaded first.
pub fn headless<T: 'static>(
    hosted: Hosted<T>,
    view: fn(&Hosted<T>, &mut Cx<Hosted<T>>) -> El,
    size: (f32, f32),
) -> Runtime<Hosted<T>> {
    let text = TextSystem::new(hosted.face.fonts(), hosted.face.families());
    let mut runtime = Runtime::with_text(hosted, view, size, Env::new(true, 1.0), text);
    runtime.frame(0.0);
    runtime
}
