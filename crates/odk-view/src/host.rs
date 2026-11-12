//! The window: one theme, a component store, and a list of layers above the page.

use odk_theme::ThemeSnapshot;
use odk_theme::component::ComponentTokens;
use odk_theme::foundation::{FoundationConfig, FoundationTokens};
use odk_theme::semantic::ResolveOptions;
use odk_theme::{ThemeDiagnostics, ThemeManager};
use rune::text::TextSystem;
use rune::{App, AppSpec, Cx, El, Env, Runtime, Tx, Waker, el};

use crate::face::Face;
use crate::ink::ink;
use crate::store::Store;

/// The application's model beside the theme and the component store.
pub struct Hosted<T> {
    pub app: T,
    pub ui: Store,
    /// What is drawn: the previewed theme while one is shown, and the followed theme otherwise.
    theme: ThemeSnapshot,
    /// The theme the application follows, which a preview does not replace.
    followed: ThemeSnapshot,
    previewing: bool,
    config: FoundationConfig,
    options: ResolveOptions,
    diagnostics: Option<ThemeDiagnostics>,
    face: Face,
}

impl<T> Hosted<T> {
    pub fn new(app: T, theme: ThemeSnapshot, face: Face) -> Self {
        Self {
            app,
            ui: Store::default(),
            followed: theme.clone(),
            theme,
            previewing: false,
            config: FoundationConfig::default(),
            options: ResolveOptions::default(),
            diagnostics: None,
            face,
        }
    }

    /// Draws with `config`: the application's density, corner radius and own choice of reduced
    /// motion, which a person's request adds to and never overrides (ADR 0048).
    pub fn with_config(mut self, config: FoundationConfig) -> Self {
        self.config = config;
        self
    }

    /// The application's foundation settings.
    pub fn config(&self) -> &FoundationConfig {
        &self.config
    }

    /// Changes the foundation settings: whatever `change` leaves alone keeps its value. The next
    /// frame draws with them.
    pub fn configure(&mut self, change: impl FnOnce(&mut FoundationConfig)) {
        change(&mut self.config);
    }

    /// The foundation tokens in force: the settings, the user's text size and reduced motion.
    pub fn foundation(&self) -> std::sync::Arc<FoundationTokens> {
        crate::style::paint(self).tokens.foundation().clone()
    }

    /// The component tokens in force, which every component draws from.
    pub fn components(&self) -> ComponentTokens {
        crate::style::paint(self).tokens
    }

    pub fn theme(&self) -> &ThemeSnapshot {
        &self.theme
    }

    /// The theme the application follows from the next frame: the user changed the palette, the
    /// text size or the request for reduced motion. While a theme is previewed it is kept for
    /// when the preview ends, and only the user's request for reduced motion, which is not a
    /// theme's, changes what is drawn.
    pub fn set_theme(&mut self, theme: ThemeSnapshot) {
        if self.previewing {
            self.theme.reduced_motion = theme.reduced_motion;
        } else {
            self.theme = theme.clone();
        }
        self.followed = theme;
    }

    /// Draws `snapshot` in place of the theme the application follows, until
    /// [`Self::end_theme_preview`]: for a tool that shows a theme without choosing it, such as a
    /// theme selector. Omarchy's files and its current theme are not touched, and a change of the
    /// followed theme meanwhile is kept, not drawn. Its request for reduced motion is the
    /// followed theme's, whatever the snapshot says; the foundation (density, corners) is the
    /// application's and stays. The colour options apply to the followed theme only: a preview
    /// is resolved with the options of the manager that loaded it.
    pub fn preview_theme(&mut self, mut snapshot: ThemeSnapshot) {
        snapshot.reduced_motion = self.followed.reduced_motion;
        self.theme = snapshot;
        self.previewing = true;
    }

    /// Draws the followed theme again, as it is now and not as it was when the preview began.
    /// Nothing happens when no theme is previewed.
    pub fn end_theme_preview(&mut self) {
        if self.previewing {
            self.theme = self.followed.clone();
            self.previewing = false;
        }
    }

    /// The theme being previewed, if one is.
    pub fn previewed_theme(&self) -> Option<&ThemeSnapshot> {
        self.previewing.then_some(&self.theme)
    }

    /// The theme the application follows: what is drawn unless a theme is previewed.
    pub fn followed_theme(&self) -> &ThemeSnapshot {
        &self.followed
    }

    /// The colour options the application asked for, or the defaults.
    pub fn colour_options(&self) -> &ResolveOptions {
        &self.options
    }

    /// What the application draws and why: the theme's name and `colors.toml`, its mode, how many
    /// colours it defines and which optional ones it leaves out, whether a fallback of ADR 0041 is
    /// in use, the latest reload's error and the generation. `None` before [`run`] has started.
    pub fn theme_diagnostics(&self) -> Option<&ThemeDiagnostics> {
        self.diagnostics.as_ref()
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
    cx.reduced_motion(hosted.theme.reduced_motion || hosted.config.reduced_motion);
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

/// Changes the application's foundation settings from anywhere, as [`Hosted::configure`] does:
/// density, corner radius and its own choice of reduced motion. Whatever `change` leaves alone keeps
/// its value, and the next frame draws with the result. `Err` when the application did not start
/// through [`run`].
pub fn configure<T: 'static>(
    change: impl FnOnce(&mut FoundationConfig) + Send + 'static,
) -> Result<(), crate::post::NoWindow> {
    crate::post::on_host::<T>(move |host| host.configure(change))
}

/// Turns the application's own reduced motion on or off, for an application that offers its users
/// the choice. It adds to the user's own request, which it never overrides (ADR 0048).
pub fn set_reduced_motion<T: 'static>(reduced: bool) -> Result<(), crate::post::NoWindow> {
    configure::<T>(move |config| config.reduced_motion = reduced)
}

/// What does not depend on the application's state type: the window's theme and what it is asked
/// to show. [`run`] hands the window's host out through it.
pub(crate) trait AnyHost {
    fn remember_colour_options(&mut self, options: ResolveOptions);
    fn preview(&mut self, snapshot: ThemeSnapshot);
    fn end_preview(&mut self);
    fn update(&mut self, update: crate::follow::Update);
}

impl<T> AnyHost for Hosted<T> {
    fn remember_colour_options(&mut self, options: ResolveOptions) {
        self.options = options;
    }

    fn preview(&mut self, snapshot: ThemeSnapshot) {
        self.preview_theme(snapshot);
    }

    fn end_preview(&mut self) {
        self.end_theme_preview();
    }

    fn update(&mut self, update: crate::follow::Update) {
        if let Some(snapshot) = update.snapshot {
            self.set_theme(snapshot);
        }
        self.diagnostics = Some(update.diagnostics);
    }
}

/// Draws `snapshot` in every window in place of the theme the application follows, until
/// [`end_theme_preview`], as [`Hosted::preview_theme`] does, from anywhere. `Err` when the
/// application did not start through [`run`].
pub fn preview_theme(snapshot: ThemeSnapshot) -> Result<(), crate::post::NoWindow> {
    crate::post::on_any_host(move |host| host.preview(snapshot))
}

/// Draws the followed theme again; see [`Hosted::end_theme_preview`].
pub fn end_theme_preview() -> Result<(), crate::post::NoWindow> {
    crate::post::on_any_host(|host| host.end_preview())
}

/// The key of the element that has focus, from inside a key handler: what a component derives from
/// its id (a button's is its id, a table's rows are `<id>-row-<key>`). `None` when nothing has
/// focus. Rune gives no way to ask outside a handler or to move focus to a key, so Tab (which
/// [`run`] makes visit every control) is the only way focus moves (`docs/checks.md`).
pub fn focused_component(tx: &Tx) -> Option<&str> {
    tx.focused()
}

/// How a window opens: its title, its size, its font and its foundation settings.
#[derive(Clone)]
pub struct WindowConfig {
    title: String,
    size: (f32, f32),
    face: Option<Face>,
    foundation: FoundationConfig,
}

impl WindowConfig {
    /// A window `size` points wide and high, on the user's font and the default foundation.
    pub fn new(size: (f32, f32)) -> Self {
        Self {
            title: "ODK".into(),
            size,
            face: None,
            foundation: FoundationConfig::default(),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// The font, instead of the family `odkd` reports.
    pub fn face(mut self, face: Face) -> Self {
        self.face = Some(face);
        self
    }

    /// The application's density, corner radius and own choice of reduced motion.
    pub fn foundation(mut self, foundation: FoundationConfig) -> Self {
        self.foundation = foundation;
        self
    }
}

/// Open the window on the user's theme and keep it up to date: the palette when the theme changes,
/// the text size, and `odkd`'s request for reduced motion, each from the next frame. With no
/// theme it draws the emergency palette (ADR 0041). The font is `config`'s when it names one, and
/// otherwise the family `odkd` reports, found in the fonts the sandbox shows
/// ([`Face::installed`], ADR 0095), and Rune's bundled one when there is none. Rune loads a family
/// from bytes before the window opens and not after, so a font changed while the window is open
/// applies at the next start (`docs/checks.md`).
pub fn run<T: 'static>(
    app: T,
    config: WindowConfig,
    view: fn(&Hosted<T>, &mut Cx<Hosted<T>>) -> El,
) -> Result<(), String> {
    let WindowConfig {
        title,
        size,
        face,
        foundation,
    } = config;
    // Rune keeps the title for the life of the process, and `run` lasts as long.
    let title: &'static str = Box::leak(title.into_boxed_str());
    let manager = ThemeManager::current();
    let face = face.unwrap_or_else(|| {
        manager
            .font_family()
            .and_then(Face::installed)
            .unwrap_or(Face::bundled())
    });
    let mut hosted = Hosted::new(app, (*manager.snapshot()).clone(), face).with_config(foundation);
    hosted.diagnostics = Some(manager.diagnostics());
    let spec = AppSpec {
        fonts: hosted.face.fonts(),
        families: hosted.face.families(),
        poll: Some(poll::<T>),
        setup: Some(setup::<T>),
        ..AppSpec::new(title, size, spec_init::<T>, view)
    };
    let app = App::new(spec)
        .args(None::<String>)
        .init(move || hosted)
        .tab_visits_controls(true);
    let handle = app.handle();
    // What other threads reach the window's host through: an answer from the file chooser, or a
    // change of the foundation settings.
    let feed = handle.clone();
    crate::post::install(std::sync::Arc::new(move |job| {
        feed.update(move |hosted: &mut Hosted<T>, _| job(hosted as &mut dyn std::any::Any));
    }));
    let feed = handle.clone();
    crate::post::install_any(std::sync::Arc::new(move |job| {
        feed.update(move |hosted: &mut Hosted<T>, _| job(hosted as &mut dyn AnyHost));
    }));
    // Kept until the window closes.
    let follower = crate::follow::follow(manager, move |update| {
        handle.update(move |hosted: &mut Hosted<T>, _| AnyHost::update(hosted, update));
    });
    crate::follow::install(follower.commands());
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
