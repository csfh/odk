use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{App, Rgba, SharedString, rgba};
use odk_theme::component::ComponentTokens;
use odk_theme::foundation::{FontFamily, FontWeight, FoundationConfig, FoundationTokens};
use odk_theme::semantic::{OptionsError, ResolveOptions};
use odk_theme::{Color, TextScale, ThemeDiagnostics, ThemeManager, ThemeSnapshot, ThemeWatcher};

/// What components draw with now: the theme's snapshot, and the component tokens of its
/// semantic colours and the current foundation. One slot, written only by [`publish`], so they never disagree.
#[derive(Clone)]
pub(crate) struct Current {
    snapshot: Arc<ThemeSnapshot>,
    components: Arc<ComponentTokens>,
}

#[cfg(not(test))]
static CURRENT: std::sync::RwLock<Option<Current>> = std::sync::RwLock::new(None);

/// The foundation tokens, with the configuration they were built from: a change of the user's
/// text size builds them again from it (ADR 0043).
#[derive(Clone)]
struct Built {
    config: FoundationConfig,
    user: User,
    tokens: Arc<FoundationTokens>,
}

impl Built {
    fn new(config: FoundationConfig, user: User) -> Self {
        let tokens = Arc::new(build(&config, user));
        Self {
            config,
            user,
            tokens,
        }
    }
}

/// What the user's own `shell.toml` says that the foundation is built from: the text size
/// (ADR 0043) and a request for reduced motion (ADR 0046).
#[derive(Clone, Copy, Default, PartialEq)]
struct User {
    text: TextScale,
    reduced_motion: bool,
}

impl User {
    fn of(snapshot: &ThemeSnapshot) -> Self {
        Self {
            text: snapshot.text,
            reduced_motion: snapshot.reduced_motion,
        }
    }
}

#[cfg(not(test))]
static FOUNDATION: std::sync::LazyLock<std::sync::RwLock<Built>> = std::sync::LazyLock::new(|| {
    std::sync::RwLock::new(Built::new(FoundationConfig::default(), User::default()))
});

// Tests run on parallel threads and GPUI's test platform runs each one on its own thread, so
// a test that changes the theme (for reduced motion, say) must not reach its neighbours.
#[cfg(test)]
thread_local! {
    static CURRENT: std::cell::RefCell<Option<Current>> = const { std::cell::RefCell::new(None) };
    static FOUNDATION: std::cell::RefCell<Built> =
        std::cell::RefCell::new(Built::new(FoundationConfig::default(), User::default()));
}

/// The theme the application draws with now: its palette as the theme wrote it, its name, the
/// `colors.toml` it came from and its generation, or the emergency palette (ADR 0041). Reading
/// it never touches the disk. Components draw [`components`], not the palette
/// (`docs/spec/odk-source-palette.md`, "Rules"); code that draws something of its own takes its
/// colours from the snapshot's `semantic`.
pub fn theme_snapshot() -> Arc<ThemeSnapshot> {
    current()
        .expect("odk_ui::run has not loaded a theme")
        .snapshot
}

/// The component tokens of ADR 0040 (`docs/spec/odk-component-tokens.md`): every component's
/// colours by variant and state, from the theme's semantic colours, and its geometry by density,
/// from the foundation. Rebuilt with the theme or the configuration, never per frame; read it
/// inside `render`. Its generation is always [`theme_snapshot`]'s. Every component draws it.
pub fn components() -> Arc<ComponentTokens> {
    current()
        .expect("odk_ui::run has not loaded a theme")
        .components
}

pub(crate) fn current() -> Option<Current> {
    #[cfg(not(test))]
    let current = CURRENT.read().unwrap().clone();
    #[cfg(test)]
    let current = CURRENT.with(|c| c.borrow().clone());
    current
}

/// Makes `snapshot` what components draw, with reduced motion as the foundation tokens have
/// it, so the application's choice outlives a theme reload. The foundation is left alone
/// unless the user's text size changed (ADR 0043): then it is built again from the
/// application's configuration at the new size. The component tokens share the snapshot's
/// semantic colours and the foundation's snapshot rather than copying either, so a change of
/// density never looks at a colour and a theme change that keeps the text size never rebuilds
/// the foundation.
pub(crate) fn publish(snapshot: Arc<ThemeSnapshot>) {
    let user = User::of(&snapshot);
    if built().user != user {
        let config = built().config;
        store(Built::new(config, user));
    }
    let foundation = foundation();
    let components =
        ComponentTokens::new(foundation, snapshot.semantic.clone(), snapshot.generation);
    let current = Current {
        snapshot,
        components: Arc::new(components),
    };
    #[cfg(not(test))]
    {
        *CURRENT.write().unwrap() = Some(current);
    }
    #[cfg(test)]
    CURRENT.with(|c| *c.borrow_mut() = Some(current));
}

/// What the background task that owns the theme manager is asked to do.
enum Request {
    /// Read the theme again.
    Reload,
    /// Resolve the semantic colours again with these options, from the palette already read.
    Options(ResolveOptions),
}

/// The queue of requests, what the latest reload said, so diagnostics never wait for a reload
/// in progress, and the colour options the application asked for.
struct Themes {
    requests: async_channel::Sender<Request>,
    diagnostics: ThemeDiagnostics,
    options: ResolveOptions,
}

impl gpui::Global for Themes {}

/// Follows `manager`'s theme from now on: publishes its snapshot, and reloads whenever the
/// watcher (when `watch` is set) or [`reload_theme`] asks, or resolves its semantic colours
/// again when [`set_colour_options`] asks. Requests run one at a time on a background thread;
/// only a snapshot that changed is published, and then every window redraws on the next frame.
pub(crate) fn follow(cx: &mut App, manager: ThemeManager, watch: bool) {
    publish(manager.snapshot());
    let (sender, requests) = async_channel::unbounded::<Request>();
    // If the watcher cannot start, the theme is followed only on request.
    let watcher = match (watch, manager.source()) {
        (true, Some(source)) => {
            let sender = sender.clone();
            ThemeWatcher::follow_with(source, &manager.user_files(), move || {
                let _ = sender.try_send(Request::Reload);
            })
            .ok()
        }
        _ => None,
    };
    // The compositor's own word on reloading its configuration says when its animation setting
    // may have changed (ADR 0048); without it the setting is read again at the next reload of
    // anything else.
    let compositor_reloads = match (watch, manager.compositor()) {
        (true, Some(compositor)) => {
            let sender = sender.clone();
            compositor.watch_reloads(move || {
                let _ = sender.try_send(Request::Reload);
            })
        }
        _ => None,
    };
    cx.set_global(Themes {
        requests: sender,
        diagnostics: manager.diagnostics(),
        options: manager.options().clone(),
    });
    let mut fonts = FontFollower::new(manager.user_fonts().map(Path::to_owned));
    cx.spawn(async move |cx| {
        let _watchers = (watcher, compositor_reloads);
        let mut manager = manager;
        while let Ok(first) = requests.recv().await {
            // Requests that queued up meanwhile are answered together: one reload, with the
            // latest options.
            let (mut reload, mut options) = (false, None);
            let mut take = |request| match request {
                Request::Reload => reload = true,
                Request::Options(o) => options = Some(o),
            };
            take(first);
            while let Ok(request) = requests.try_recv() {
                take(request);
            }
            // The manager goes to a background thread to read the files or resolve, and comes
            // back.
            let (back, back_fonts, changed, diagnostics, font) = cx
                .background_executor()
                .spawn(async move {
                    let before = manager.snapshot();
                    if let Some(options) = options {
                        // Validated when asked for, so this never refuses them.
                        let _ = manager.set_options(options);
                    }
                    if reload {
                        manager.reload();
                    }
                    // Whatever was asked, the newest snapshot is published once, if it changed.
                    let after = manager.snapshot();
                    let changed = (!Arc::ptr_eq(&before, &after)).then_some(after);
                    let diagnostics = manager.diagnostics();
                    // The font is looked up again only when `fonts.conf` itself changed: it
                    // scans every font, which a theme switch does not need.
                    let font = if reload {
                        fonts.changed(resolve_monospace)
                    } else {
                        None
                    };
                    (manager, fonts, changed, diagnostics, font)
                })
                .await;
            manager = back;
            fonts = back_fonts;
            let updated = cx.update(|cx| {
                cx.global_mut::<Themes>().diagnostics = diagnostics;
                let font_changed = font.is_some_and(|family| {
                    if !text_system_knows(cx, &family) {
                        log::info!(
                            "the font {family:?} was installed after this application started, so it keeps the old one until it is restarted"
                        );
                        return false;
                    }
                    set_system_mono(family)
                });
                // While a theme is previewed the system's is kept for when the preview ends,
                // and is not drawn over it.
                let theme_changed = match changed {
                    Some(snapshot) if previewing(cx) => {
                        // The user's request for reduced motion is not the previewed theme's:
                        // it follows the system's, so it changes under a preview too.
                        let shown = cx.global::<Preview>().shown.clone();
                        let motion_moved = shown
                            .as_ref()
                            .is_some_and(|s| s.reduced_motion != snapshot.reduced_motion);
                        if let (true, Some(shown)) = (motion_moved, shown) {
                            let shown = with_motion_of(&snapshot, shown);
                            cx.global_mut::<Preview>().shown = Some(shown.clone());
                            publish(shown);
                        }
                        cx.global_mut::<Preview>().system = snapshot;
                        motion_moved
                    }
                    Some(snapshot) => {
                        publish(snapshot);
                        true
                    }
                    None => false,
                };
                if theme_changed || font_changed {
                    cx.refresh_windows();
                }
            });
            if updated.is_err() {
                break;
            }
        }
    })
    .detach();
}

/// The theme shown in place of the one the application follows, and the one it follows,
/// kept up to date while the preview lasts.
struct Preview {
    shown: Option<Arc<ThemeSnapshot>>,
    /// The followed theme as of its latest reload, drawn again when the preview ends.
    system: Arc<ThemeSnapshot>,
}

impl gpui::Global for Preview {}

fn previewing(cx: &App) -> bool {
    cx.try_global::<Preview>()
        .is_some_and(|p| p.shown.is_some())
}

/// Draws `snapshot` in every window in place of the theme the application follows, until
/// [`end_theme_preview`]. For a tool that shows a theme without choosing it, such as a design
/// system's gallery: Omarchy's files and its current theme are not touched, and a theme change
/// the watcher sees meanwhile is kept, not drawn. The snapshot should come from a
/// [`ThemeManager`] built with [`ThemeManager::with_users_files`], so that it carries the user's
/// text size; its request for reduced motion is the followed theme's, whatever it says; the
/// foundation (density, corners) is the
/// application's and stays. The colour options of the followed theme
/// ([`set_colour_options`]) apply to the followed theme only: a preview is resolved with the
/// options of the manager that loaded it.
pub fn preview_theme(cx: &mut App, snapshot: Arc<ThemeSnapshot>) {
    let system = match cx.try_global::<Preview>() {
        Some(p) if p.shown.is_some() => p.system.clone(),
        _ => theme_snapshot(),
    };
    let snapshot = with_motion_of(&system, snapshot);
    cx.set_global(Preview {
        shown: Some(snapshot.clone()),
        system,
    });
    publish(snapshot);
    cx.refresh_windows();
}

/// `snapshot`, with the user's request for reduced motion that `system` has: a request that
/// the compositor makes (ADR 0048) is not a theme's, so previewing one never changes it.
fn with_motion_of(system: &ThemeSnapshot, snapshot: Arc<ThemeSnapshot>) -> Arc<ThemeSnapshot> {
    if snapshot.reduced_motion == system.reduced_motion {
        return snapshot;
    }
    let mut own = (*snapshot).clone();
    own.reduced_motion = system.reduced_motion;
    Arc::new(own)
}

/// Draws the followed theme again, as it is now and not as it was when the preview began.
/// Nothing happens when no theme is previewed.
pub fn end_theme_preview(cx: &mut App) {
    if !previewing(cx) {
        return;
    }
    let system = cx.global::<Preview>().system.clone();
    cx.set_global(Preview {
        shown: None,
        system: system.clone(),
    });
    publish(system);
    cx.refresh_windows();
}

/// The theme being previewed, if one is.
pub fn previewed_theme(cx: &App) -> Option<Arc<ThemeSnapshot>> {
    cx.try_global::<Preview>().and_then(|p| p.shown.clone())
}

/// The theme the application follows: what is drawn unless a theme is previewed.
pub fn followed_theme(cx: &App) -> Arc<ThemeSnapshot> {
    match cx.try_global::<Preview>() {
        Some(p) if p.shown.is_some() => p.system.clone(),
        _ => theme_snapshot(),
    }
}

/// Asks for the theme to be read again, as a change on disk would. The result shows on a
/// later frame, and only if the theme changed. For an application's own "reload theme"
/// command; Omarchy's theme changes are already followed without it.
pub fn reload_theme(cx: &App) {
    if let Some(themes) = cx.try_global::<Themes>() {
        let _ = themes.requests.try_send(Request::Reload);
    }
}

/// Changes how the semantic colours of ADR 0040 are resolved: contrast minimums, strategy and
/// the palette keys each role prefers (`docs/spec/odk-semantic-tokens.md`, "Options"). The
/// theme's files are not read again. The result shows on a later frame, in
/// [`theme_snapshot`]'s `semantic`; if the theme's colours cannot meet these options, the last
/// snapshot stays and [`theme_diagnostics`] says why. Options that would weaken WCAG's
/// minimums are refused here. Separate from [`configure`], which never touches a colour.
pub fn set_colour_options(cx: &mut App, options: ResolveOptions) -> Result<(), OptionsError> {
    options.validate()?;
    if cx.has_global::<Themes>() {
        let themes = cx.global_mut::<Themes>();
        themes.options = options.clone();
        let _ = themes.requests.try_send(Request::Options(options));
    }
    Ok(())
}

/// The colour options the application asked for, or the defaults.
pub fn colour_options(cx: &App) -> ResolveOptions {
    cx.try_global::<Themes>()
        .map(|t| t.options.clone())
        .unwrap_or_default()
}

/// What the application draws and why: the theme's name and `colors.toml`, its mode, how many
/// colours it defines and which optional ones it leaves out, whether a fallback of ADR 0041 is
/// in use, the latest reload's error and the generation. `Display` writes it as a few lines,
/// for a developer tool such as the gallery. `None` before [`crate::run`] has started.
pub fn theme_diagnostics(cx: &App) -> Option<ThemeDiagnostics> {
    cx.try_global::<Themes>().map(|t| t.diagnostics.clone())
}

/// The foundation tokens of ADR 0040 (`docs/spec/odk-foundation-tokens.md`): ODK's own
/// geometry, density, spacing, type and motion, as the application configured them. They are
/// built once per change of configuration, so reading them is cheap; read them inside
/// `render`. `Button` draws them; the other components take only the monospace family.
pub fn foundation() -> Arc<FoundationTokens> {
    // Only the tokens are cloned, not the configuration they were built from: `Space` reads
    // this once per length it gives.
    #[cfg(not(test))]
    let tokens = FOUNDATION.read().unwrap().tokens.clone();
    #[cfg(test)]
    let tokens = FOUNDATION.with(|f| f.borrow().tokens.clone());
    tokens
}

fn built() -> Built {
    #[cfg(not(test))]
    let built = FOUNDATION.read().unwrap().clone();
    #[cfg(test)]
    let built = FOUNDATION.with(|f| f.borrow().clone());
    built
}

fn store(built: Built) {
    #[cfg(not(test))]
    {
        *FOUNDATION.write().unwrap() = built;
    }
    #[cfg(test)]
    FOUNDATION.with(|f| *f.borrow_mut() = built);
}

/// The application's own foundation settings (`configure`), kept as a global so they outlive
/// theme reloads.
#[derive(Default)]
struct Config(FoundationConfig);

impl gpui::Global for Config {}

/// The foundation settings the application has chosen, or the defaults if it has chosen none.
pub fn foundation_config(cx: &App) -> FoundationConfig {
    cx.try_global::<Config>()
        .map(|c| c.0.clone())
        .unwrap_or_default()
}

/// Changes the application's foundation settings: density, corner radius, reduced motion and
/// fonts. Whatever `change` leaves alone keeps its current value. Every window redraws on the
/// next frame with the new tokens.
pub fn configure(cx: &mut App, change: impl FnOnce(&mut FoundationConfig)) {
    let mut config = foundation_config(cx);
    change(&mut config);
    apply(&config);
    cx.set_global(Config(config));
    cx.refresh_windows();
}

/// Turns reduced motion on or off for the whole application, for an application that offers
/// its users the choice: overlays then show and hide without transitions and spinners stand
/// still. It adds to the user's own request, the compositor's (ADR 0046, ADR 0048), and never overrides it:
/// when the user asks for reduced motion, turning it off here keeps it on. Takes effect on the next frame.
pub fn set_reduced_motion(cx: &mut App, reduced: bool) {
    configure(cx, |c| c.reduced_motion = reduced);
}

/// Builds the foundation tokens for `config` and makes them current, with the tokens' reduced
/// motion following. The one path from a configuration to the tokens.
pub(crate) fn apply(config: &FoundationConfig) {
    let user = current().map_or_else(User::default, |c| User::of(&c.snapshot));
    store(Built::new(config.clone(), user));
    if let Some(current) = current() {
        publish(current.snapshot);
    }
}

/// The foundation tokens for `config` at the user's text size, with the user's request for
/// reduced motion added to the application's (ADR 0046).
fn build(config: &FoundationConfig, user: User) -> FoundationTokens {
    let mut effective = config.clone();
    effective.reduced_motion |= user.reduced_motion;
    FoundationTokens::scaled(&effective, user.text)
}

pub(crate) fn colour(c: Color) -> Rgba {
    rgba(c.to_rgba_u32())
}

/// GPUI's weight for a foundation weight.
pub(crate) fn weight(w: FontWeight) -> gpui::FontWeight {
    gpui::FontWeight(w.value())
}

/// The family to set text in: the application's choice; else Omarchy's font, for UI text as
/// well as monospace (ADR 0043).
pub(crate) fn family(f: &FoundationTokens, family: FontFamily) -> SharedString {
    match f.typography.family(family) {
        Some(name) => SharedString::from(name.to_owned()),
        None => system_mono(),
    }
}

/// A dark test palette.
#[cfg(any(test, feature = "test-support"))]
pub(crate) const TEST_COLORS: &str = "mode = \"dark\"\naccent = \"#ffffff\"\nmuted = \"#888888\"\nbackground = \"#000000\"\ndark_background = \"#000000\"\nlighter_background = \"#111111\"\nforeground = \"#ffffff\"\nred = \"#ff0000\"\nyellow = \"#ffff00\"\ngreen = \"#00ff00\"\nblue = \"#0000ff\"\n";

/// Loads a fixed palette so component code that reads tokens can run in unit tests.
#[cfg(any(test, feature = "test-support"))]
pub(crate) fn install_test_theme() {
    publish(Arc::new(
        ThemeSnapshot::from_toml(TEST_COLORS, None).unwrap(),
    ));
}

/// The monospace face for keys, marks and spinners: the application's choice if it made one,
/// else Omarchy's.
pub(crate) fn mono() -> SharedString {
    family(&foundation(), FontFamily::Mono)
}

/// The font Omarchy has set as the system's monospace font (`omarchy font set`), which
/// fontconfig's `monospace` alias names. GPUI looks fonts up by family name without
/// fontconfig's aliases, so the alias is resolved here: at first use, and again whenever the
/// user's `fonts.conf` changes (ADR 0043), by [`follow`]. When fontconfig has no answer, the
/// alias itself is passed on and GPUI falls back to its default face.
fn system_mono() -> SharedString {
    if let Some(family) = SYSTEM_MONO.read().unwrap().clone() {
        return family;
    }
    let family = resolve_monospace().unwrap_or_else(|| "monospace".into());
    SYSTEM_MONO.write().unwrap().get_or_insert(family).clone()
}

/// The family the monospace alias resolved to, once it has. Process-wide, since the font is
/// the machine's and not an application's choice.
static SYSTEM_MONO: std::sync::RwLock<Option<SharedString>> = std::sync::RwLock::new(None);

/// Makes `family` the system's monospace font. True when it is not the one already.
fn set_system_mono(family: SharedString) -> bool {
    let mut stored = SYSTEM_MONO.write().unwrap();
    let changed = stored.as_ref() != Some(&family);
    *stored = Some(family);
    changed
}

/// Whether GPUI can set text in `family`. Its font database is read when the application
/// starts, so a font installed since is not in it, and naming one would set all the text in
/// GPUI's fallback face, which is worse than keeping the old font. `resolve_font` falls back
/// without saying so; the font it gives is the family's own only when GPUI has the family.
fn text_system_knows(cx: &App, family: &SharedString) -> bool {
    let text = cx.text_system();
    let id = text.resolve_font(&gpui::font(family.clone()));
    text.get_font_for_id(id)
        .is_some_and(|found| found.family == *family)
}

/// What of a file says it may have changed: when it was written, how long it is and which file
/// it is, which a rename over it changes. `None` when there is no such file.
#[derive(Debug, Clone, PartialEq)]
struct Stamp(Option<(std::time::SystemTime, u64, u64)>);

impl Stamp {
    fn of(path: &Path) -> Self {
        use std::os::unix::fs::MetadataExt;
        Self(std::fs::metadata(path).ok().map(|m| {
            (
                m.modified().unwrap_or(std::time::UNIX_EPOCH),
                m.len(),
                m.ino(),
            )
        }))
    }
}

/// Notices that the user's `fonts.conf` changed, so the font is looked up again for that and
/// not for every theme change.
struct FontFollower {
    file: Option<PathBuf>,
    seen: Stamp,
}

impl FontFollower {
    fn new(file: Option<PathBuf>) -> Self {
        let seen = file.as_deref().map_or(Stamp(None), Stamp::of);
        Self { file, seen }
    }

    /// `resolve`'s answer when the file is not the one last seen, else `None`.
    fn changed(&mut self, resolve: impl FnOnce() -> Option<SharedString>) -> Option<SharedString> {
        let now = Stamp::of(self.file.as_deref()?);
        if now == self.seen {
            return None;
        }
        self.seen = now;
        resolve()
    }
}

/// The family fontconfig gives for `monospace`, with the user's own configuration applied, as
/// it is now: `fc-match` is run afresh, because a fontconfig loaded into this process keeps
/// the configuration it started with (tested), and the user's `fonts.conf` can change while
/// the application runs (ADR 0043). The font file it names is opened for its family name, the
/// name GPUI's font lookup knows it by. Without `fc-match`, the answer of the fontconfig loaded
/// into the process, which is as current as the application's start.
fn resolve_monospace() -> Option<SharedString> {
    resolve_with_fc_match(None).or_else(resolve_in_process)
}

/// `fc-match monospace`, in a process of its own, for the user whose home is `home`, or this
/// process's.
fn resolve_with_fc_match(home: Option<&Path>) -> Option<SharedString> {
    let mut command = std::process::Command::new("fc-match");
    command.args(["monospace", "-f", "%{file}\n%{index}"]);
    if let Some(home) = home {
        command.env("HOME", home).env_remove("XDG_CONFIG_HOME");
    }
    let output = command.output().ok().filter(|o| o.status.success())?;
    let text = String::from_utf8(output.stdout).ok()?;
    let (file, index) = text.split_once('\n')?;
    let font = font_kit::font::Font::from_path(file, index.trim().parse().ok()?).ok()?;
    Some(font.family_name().into())
}

/// The fontconfig loaded into this process, through font-kit.
fn resolve_in_process() -> Option<SharedString> {
    use font_kit::family_name::FamilyName;
    use font_kit::properties::Properties;
    use font_kit::source::SystemSource;
    let handle = SystemSource::new()
        .select_best_match(&[FamilyName::Monospace], &Properties::new())
        .ok()?;
    Some(handle.load().ok()?.family_name().into())
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::fs;
    use std::rc::Rc;
    use std::sync::Arc;

    use gpui::{Styled, TestAppContext, VisualTestContext, div};
    use odk_theme::component::{ButtonLayout, ButtonVariant, States};
    use odk_theme::foundation::{CornerRadius, Density, FontConfig, TypeRole};
    use odk_theme::semantic::{ResolveOptions, Strategy};
    use odk_theme::{Fallback, ThemeManager, ThemeSnapshot, ThemeSource};
    use tempfile::TempDir;

    use crate::StyledExt;
    use crate::checklist::open;

    /// The system font is one value for the whole process, so the tests that read or change it
    /// take turns.
    static SYSTEM_FONT: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// On a machine with fontconfig and a monospace font, the alias resolves to a real family,
    /// which GPUI can find by name. Skipped where fontconfig has no answer.
    #[test]
    fn the_monospace_alias_resolves_to_a_real_family() {
        let _turn = SYSTEM_FONT.lock().unwrap_or_else(|e| e.into_inner());
        let Some(family) = super::resolve_monospace() else {
            return;
        };
        super::set_system_mono(family.clone());
        assert_ne!(family.as_ref(), "monospace");
        assert_eq!(super::mono(), family);
        eprintln!("monospace is {family}");
    }

    /// A font change of the user's (ADR 0043) reaches views on the next frame, in the monospace
    /// face and in the UI text that follows it, and an application's own choice still wins.
    #[gpui::test]
    fn a_new_system_font_reaches_views_on_the_next_frame(cx: &mut TestAppContext) {
        let _turn = SYSTEM_FONT.lock().unwrap_or_else(|e| e.into_inner());
        let before = super::system_mono();
        super::apply(&odk_theme::foundation::FoundationConfig::default());
        let seen = Rc::new(RefCell::new(Vec::new()));
        let record = seen.clone();
        let cx = open(cx, move |_, _| {
            let f = crate::foundation();
            record.borrow_mut().push((
                super::mono(),
                super::family(&f, odk_theme::foundation::FontFamily::Ui),
            ));
            div()
        });
        assert_eq!(seen.borrow().last().unwrap().0, before);

        assert!(super::set_system_mono("Lilex".into()));
        assert!(
            !super::set_system_mono("Lilex".into()),
            "the same font is no change"
        );
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let (mono, ui) = seen.borrow().last().unwrap().clone();
        assert_eq!((mono.as_ref(), ui.as_ref()), ("Lilex", "Lilex"));

        super::set_system_mono(before);
    }

    /// The font is looked up again when `fonts.conf` is written, replaced or removed, and not
    /// when it is not.
    /// `fonts.conf` as `omarchy font set` writes it, naming `family`.
    fn fonts_conf(family: &str) -> String {
        format!(
            "<?xml version=\"1.0\"?>\n<!DOCTYPE fontconfig SYSTEM \"fonts.dtd\">\n<fontconfig>\n  <match target=\"pattern\">\n    <test name=\"family\" qual=\"any\">\n      <string>monospace</string>\n    </test>\n    <edit name=\"family\" mode=\"prepend_first\" binding=\"strong\">\n      <string>{family}</string>\n    </edit>\n  </match>\n</fontconfig>\n"
        )
    }

    /// The font is the one `fonts.conf` names *now*, however often it changes: a fontconfig
    /// loaded into the process would keep the first answer, which is what looking it up
    /// afresh is for (ADR 0043). Needs `fc-match` and two monospace fonts; skipped without.
    #[test]
    fn the_font_follows_fonts_conf_as_it_is_rewritten() {
        let home = tempfile::tempdir().unwrap();
        let conf = home.path().join(".config/fontconfig/fonts.conf");
        std::fs::create_dir_all(conf.parent().unwrap()).unwrap();
        let Some(default) = super::resolve_with_fc_match(Some(home.path())) else {
            return;
        };
        let listed = std::process::Command::new("fc-list")
            .args([":spacing=100", "-f", "%{family[0]}\\n"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        let mut families: Vec<&str> = listed.lines().filter(|f| !f.is_empty()).collect();
        families.sort();
        families.dedup();
        let listed_count = families.len();
        let named = |family: &str| {
            std::fs::write(&conf, fonts_conf(family)).unwrap();
            super::resolve_with_fc_match(Some(home.path())).unwrap()
        };
        // Fonts the default is not, by what they resolve to.
        let mut others = Vec::new();
        for family in families {
            let found = named(family);
            if found != default && !others.iter().any(|(_, f)| *f == found) {
                others.push((family, found));
            }
        }
        // Skipped only for what the machine lacks, not for what the code failed to do.
        if listed_count < 3 {
            return;
        }
        assert!(
            others.len() >= 2,
            "no font made a difference to the answer: {others:?} of {listed_count}"
        );
        let ((a, found_a), (b, found_b)) = (&others[0], &others[1]);
        assert_eq!(named(a), *found_a);
        assert_eq!(named(b), *found_b);
        assert_eq!(named(a), *found_a, "and back");
        std::fs::remove_file(&conf).unwrap();
        assert_eq!(
            super::resolve_with_fc_match(Some(home.path())).unwrap(),
            default
        );
    }

    #[test]
    fn the_font_is_looked_up_again_only_when_fonts_conf_changes() {
        let dir = tempfile::tempdir().unwrap();
        let conf = dir.path().join("fonts.conf");
        std::fs::write(&conf, "<fontconfig/>").unwrap();
        let mut fonts = super::FontFollower::new(Some(conf.clone()));
        let found = |name: &str| Some(gpui::SharedString::from(name.to_owned()));

        assert_eq!(fonts.changed(|| panic!("nothing changed")), None);

        std::fs::write(&conf, "<fontconfig><match/></fontconfig>").unwrap();
        assert_eq!(fonts.changed(|| found("A")), found("A"));
        assert_eq!(fonts.changed(|| panic!("seen already")), None);

        // As `omarchy font set` does not, and as other tools do: a rename over it, here of a file
        // of the same length and the same time, so that only which file it is says it changed.
        let new = dir.path().join("new");
        std::fs::write(&new, "<fontconfig><match/></fontconfig>").unwrap();
        let written = std::fs::metadata(&conf).unwrap().modified().unwrap();
        std::fs::File::options()
            .write(true)
            .open(&new)
            .unwrap()
            .set_modified(written)
            .unwrap();
        std::fs::rename(&new, &conf).unwrap();
        assert_eq!(fonts.changed(|| found("B")), found("B"));

        std::fs::remove_file(&conf).unwrap();
        assert_eq!(fonts.changed(|| found("C")), found("C"));
        assert_eq!(fonts.changed(|| panic!("seen already")), None);

        // No file to follow, as without a home: nothing is ever looked up.
        let mut none = super::FontFollower::new(None);
        assert_eq!(none.changed(|| panic!("no file")), None);
    }

    #[test]
    fn an_application_s_monospace_family_wins_over_omarchy_s() {
        super::apply(&odk_theme::foundation::FoundationConfig {
            fonts: FontConfig {
                mono: Some("Lilex".into()),
                ui: None,
            },
            ..Default::default()
        });
        assert_eq!(super::mono().as_ref(), "Lilex");
    }

    /// A view reads the foundation inside `render`, and a change of configuration reaches it
    /// on the next frame without the application redrawing anything itself.
    #[gpui::test]
    fn a_configuration_change_reaches_views_on_the_next_frame(cx: &mut TestAppContext) {
        let seen = Rc::new(Cell::new(0.0));
        let record = seen.clone();
        let cx = open(cx, move |_, _| {
            record.set(crate::foundation().controls.metrics().height);
            div()
        });
        assert_eq!(
            seen.get(),
            34.0,
            "the default density before any configuration"
        );

        cx.update(|_, cx| crate::configure(cx, |c| c.density = Density::Compact));
        cx.run_until_parked();
        assert_eq!(seen.get(), 28.0);
    }

    /// Changing one setting keeps the others, so choosing a density cannot undo an earlier
    /// choice of reduced motion; and none of them touches a colour.
    #[gpui::test]
    fn configure_changes_only_what_it_is_asked_to(cx: &mut TestAppContext) {
        let cx = open(cx, |_, _| div());
        let colours = crate::theme_snapshot();
        cx.update(|_, cx| crate::set_reduced_motion(cx, true));
        cx.update(|_, cx| {
            crate::configure(cx, |c| {
                c.density = Density::Comfortable;
                c.corner_radius = CornerRadius::Square;
            })
        });
        let f = crate::foundation();
        assert!(f.motion.reduced, "reduced motion survives a density change");
        assert_eq!(f.controls.metrics().height, 40.0);
        assert_eq!(f.geometry.radius.default, 0.0);
        cx.update(|_, cx| {
            let config = crate::foundation_config(cx);
            assert_eq!(config.density, Density::Comfortable);
            assert!(config.reduced_motion);
        });
        assert_eq!(crate::theme_snapshot().semantic, colours.semantic);
    }

    /// `text_role` sets a role's size, line height and weight, and its family: the
    /// application's where it names one, else Omarchy's font.
    #[test]
    fn text_role_sets_the_role_s_type() {
        let _turn = SYSTEM_FONT.lock().unwrap_or_else(|e| e.into_inner());
        let mut heading = div().text_role(TypeRole::Heading);
        let style = heading.text_style().clone().unwrap();
        assert_eq!(style.font_size, Some(gpui::px(18.0).into()));
        assert_eq!(style.line_height, Some(gpui::px(26.0).into()));
        assert_eq!(style.font_weight, Some(gpui::FontWeight::SEMIBOLD));
        assert_eq!(
            style.font_family,
            Some(super::mono()),
            "UI text is set in Omarchy's font too (ADR 0043)"
        );

        let mut mono = div().text_role(TypeRole::Mono);
        let family = mono.text_style().clone().unwrap().font_family;
        assert_eq!(family, Some(super::mono()));

        super::apply(&odk_theme::foundation::FoundationConfig {
            fonts: FontConfig {
                ui: Some("Inter".into()),
                mono: None,
            },
            ..Default::default()
        });
        let mut body = div().text_role(TypeRole::Body);
        let family = body.text_style().clone().unwrap().font_family;
        assert_eq!(family.as_ref().map(|f| f.as_ref()), Some("Inter"));

        // The way back to GPUI's own face.
        super::apply(&odk_theme::foundation::FoundationConfig {
            fonts: FontConfig {
                ui: Some(".SystemUIFont".into()),
                mono: None,
            },
            ..Default::default()
        });
        let mut body = div().text_role(TypeRole::Body);
        let family = body.text_style().clone().unwrap().font_family;
        assert_eq!(family.as_ref().map(|f| f.as_ref()), Some(".SystemUIFont"));
    }

    fn snapshot_at(base_size: u32) -> Arc<ThemeSnapshot> {
        let shell = format!("[font]\nbase-size = {base_size}\n");
        Arc::new(ThemeSnapshot::from_toml(super::TEST_COLORS, Some(&shell)).unwrap())
    }

    /// The user's text size reaches the foundation when the theme is published (ADR 0043), keeps
    /// the application's own configuration, and a theme that keeps the size does not build the
    /// foundation again.
    #[test]
    fn a_new_text_size_rebuilds_the_foundation_from_the_applications_configuration() {
        super::apply(&odk_theme::foundation::FoundationConfig {
            density: Density::Compact,
            corner_radius: CornerRadius::Square,
            ..Default::default()
        });
        super::publish(snapshot_at(12));
        let plain = crate::foundation();
        assert_eq!(plain.controls.metrics().height, 28.0);

        super::publish(snapshot_at(18));
        let big = crate::foundation();
        assert_eq!(big.controls.metrics().height, 42.0, "28 px at 1.5 times");
        assert_eq!(big.typography.style(TypeRole::Body).size, 20.0);
        assert_eq!(
            big.controls.density,
            Density::Compact,
            "the application's choice"
        );
        assert_eq!(big.geometry.radius.default, 0.0, "the application's choice");
        assert_eq!(big.geometry.border_width, plain.geometry.border_width);
        assert_eq!(
            crate::components().foundation().controls.metrics().height,
            42.0,
            "the component tokens are built on the foundation just made"
        );

        super::publish(snapshot_at(18));
        assert!(
            Arc::ptr_eq(&big, &crate::foundation()),
            "same size, same tokens"
        );

        // A later `configure` keeps the size.
        super::apply(&odk_theme::foundation::FoundationConfig::default());
        assert_eq!(crate::foundation().controls.metrics().height, 51.0);
    }

    const DARK: &str = super::TEST_COLORS;

    fn light() -> String {
        DARK.replace("\"dark\"", "\"light\"")
            .replace("background = \"#000000\"", "background = \"#fafafa\"")
            .replace("foreground = \"#ffffff\"", "foreground = \"#101010\"")
    }

    /// What a view saw in its last `render`, and how many times it rendered.
    #[derive(Clone, Default)]
    struct Seen {
        renders: Rc<Cell<u32>>,
        background: Rc<RefCell<String>>,
        generation: Rc<Cell<u64>>,
    }

    /// Opens a view that reads the theme in `render`, and follows a theme directory holding
    /// `colors` the way `run` follows Omarchy's, without the watcher: reloads come from
    /// `reload_theme`.
    fn follow_dir<'a>(
        cx: &'a mut TestAppContext,
        colors: &str,
    ) -> (&'a mut VisualTestContext, TempDir, Seen) {
        let seen = Seen::default();
        let record = seen.clone();
        let cx = open(cx, move |_, _| {
            record.renders.set(record.renders.get() + 1);
            *record.background.borrow_mut() =
                crate::theme_snapshot().semantic.surface.canvas.to_hex();
            record.generation.set(crate::theme_snapshot().generation);
            div()
        });
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("colors.toml"), colors).unwrap();
        let mut manager = ThemeManager::new(ThemeSource::Dir(dir.path().to_owned()));
        manager.reload();
        cx.update(|window, cx| {
            super::follow(cx, manager, false);
            window.refresh();
        });
        cx.run_until_parked();
        (cx, dir, seen)
    }

    fn reload(cx: &mut VisualTestContext) {
        cx.update(|_, cx| crate::reload_theme(cx));
        cx.run_until_parked();
    }

    /// A theme change on disk reaches a view that reads the theme in `render`, on the next
    /// frame, without the application doing anything.
    #[gpui::test]
    fn a_theme_change_reaches_views_on_the_next_frame(cx: &mut TestAppContext) {
        let (cx, dir, seen) = follow_dir(cx, DARK);
        assert_eq!(*seen.background.borrow(), "#000000");
        assert_eq!(seen.generation.get(), 1);

        fs::write(dir.path().join("colors.toml"), light()).unwrap();
        reload(cx);
        assert_eq!(*seen.background.borrow(), "#fafafa");
        assert_eq!(seen.generation.get(), 2);
        let snapshot = crate::theme_snapshot();
        assert_eq!(snapshot.palette.mode, odk_theme::Mode::Light);
        assert_eq!(snapshot.colors, Some(dir.path().join("colors.toml")));
    }

    /// A previewed theme is drawn in place of the followed one and stays when the followed one
    /// changes underneath it or the application's configuration does; ending the preview draws
    /// the followed theme as it is then, not as it was when the preview began. The theme's files
    /// are never written.
    #[gpui::test]
    fn a_previewed_theme_is_drawn_until_it_ends(cx: &mut TestAppContext) {
        let (cx, dir, seen) = follow_dir(cx, DARK);
        let colors = dir.path().join("colors.toml");
        let preview = Arc::new(ThemeSnapshot::from_toml(&light(), None).unwrap());
        assert!(cx.update(|_, cx| crate::previewed_theme(cx)).is_none());

        cx.update(|_, cx| crate::preview_theme(cx, preview.clone()));
        cx.run_until_parked();
        assert_eq!(*seen.background.borrow(), "#fafafa");
        assert_eq!(crate::theme_snapshot().palette.mode, odk_theme::Mode::Light);
        assert_eq!(
            fs::read_to_string(&colors).unwrap(),
            DARK,
            "the theme on disk is not the preview's business"
        );

        // The followed theme changes meanwhile: the preview stays, the change is kept.
        let third = DARK.replace("background = \"#000000\"", "background = \"#102030\"");
        fs::write(&colors, &third).unwrap();
        reload(cx);
        assert_eq!(
            *seen.background.borrow(),
            "#fafafa",
            "drawn over by a reload"
        );
        cx.update(|_, cx| {
            assert_eq!(
                crate::followed_theme(cx).palette.background.to_hex(),
                "#102030"
            );
            assert!(crate::previewed_theme(cx).is_some());
            crate::configure(cx, |c| c.density = Density::Compact);
        });
        cx.run_until_parked();
        assert_eq!(
            *seen.background.borrow(),
            "#fafafa",
            "drawn over by configure"
        );
        assert_eq!(crate::foundation().controls.metrics().height, 28.0);

        // A second preview replaces the first and still remembers the same followed theme.
        let other = Arc::new(ThemeSnapshot::from_toml(DARK, None).unwrap());
        cx.update(|_, cx| crate::preview_theme(cx, other));
        cx.run_until_parked();
        assert_eq!(*seen.background.borrow(), "#000000");

        cx.update(|_, cx| crate::end_theme_preview(cx));
        cx.run_until_parked();
        assert_eq!(
            *seen.background.borrow(),
            "#102030",
            "the latest followed theme"
        );
        cx.update(|_, cx| assert!(crate::previewed_theme(cx).is_none()));
        cx.update(|_, cx| crate::end_theme_preview(cx));
    }

    /// A reload that finds the same theme publishes nothing, so no window redraws.
    #[gpui::test]
    fn an_unchanged_theme_does_not_redraw(cx: &mut TestAppContext) {
        let (cx, _dir, seen) = follow_dir(cx, DARK);
        let renders = seen.renders.get();
        let before = crate::theme_snapshot();
        reload(cx);
        reload(cx);
        assert_eq!(seen.renders.get(), renders);
        assert!(Arc::ptr_eq(&before, &crate::theme_snapshot()));
    }

    /// A broken theme leaves the last good one drawn, and diagnostics say why; once fixed, it
    /// is drawn.
    #[gpui::test]
    fn a_broken_theme_keeps_the_last_good_one(cx: &mut TestAppContext) {
        let (cx, dir, seen) = follow_dir(cx, DARK);
        let colors = dir.path().join("colors.toml");
        fs::write(&colors, light().replace("#fafafa", "#fafaf")).unwrap();
        reload(cx);
        assert_eq!(*seen.background.borrow(), "#000000");
        assert_eq!(
            crate::theme_snapshot().semantic.surface.canvas.to_hex(),
            "#000000"
        );
        let diagnostics = cx.update(|_, cx| crate::theme_diagnostics(cx)).unwrap();
        assert_eq!(diagnostics.fallback, Some(Fallback::LastGood));
        let failure = diagnostics.failure.clone().unwrap();
        assert!(matches!(
            *failure.error,
            odk_theme::Error::BadColor {
                key: "background",
                ..
            }
        ));
        assert!(diagnostics.to_string().contains("`background`"));

        fs::write(&colors, light()).unwrap();
        reload(cx);
        assert_eq!(*seen.background.borrow(), "#fafafa");
        let diagnostics = cx.update(|_, cx| crate::theme_diagnostics(cx)).unwrap();
        assert_eq!(diagnostics.fallback, None);
        assert!(diagnostics.failure.is_none());
    }

    /// The user's request for reduced motion (ADR 0046) reaches the tokens when its snapshot is
    /// published, adds to the application's choice and never replaces it, and is gone when the
    /// user takes it back, at once and without a new theme.
    /// ADR 0048: the user's request for reduced motion is the system's, not a theme's, so a
    /// previewed theme neither loses it nor gains one.
    #[gpui::test]
    fn a_previewed_theme_carries_the_systems_request_for_reduced_motion(cx: &mut TestAppContext) {
        let (cx, _dir, _seen) = follow_dir(cx, DARK);
        let followed = crate::theme_snapshot();
        let asked = |on: bool| {
            Arc::new(ThemeSnapshot {
                reduced_motion: on,
                ..(*followed).clone()
            })
        };
        let preview = Arc::new(ThemeSnapshot::from_toml(&light(), None).unwrap());
        assert!(!preview.reduced_motion);

        // The system asks: a preview that does not is drawn with it.
        crate::theme::publish(asked(true));
        cx.update(|_, cx| crate::preview_theme(cx, preview.clone()));
        assert_eq!(crate::theme_snapshot().palette.mode, odk_theme::Mode::Light);
        assert!(crate::theme_snapshot().reduced_motion);
        assert!(crate::foundation().motion.reduced);
        cx.update(|_, cx| crate::end_theme_preview(cx));
        assert!(
            crate::foundation().motion.reduced,
            "back on the followed theme"
        );

        // The system does not: a preview that does is drawn without it.
        crate::theme::publish(asked(false));
        let asking = Arc::new(ThemeSnapshot {
            reduced_motion: true,
            ..(*preview).clone()
        });
        cx.update(|_, cx| crate::preview_theme(cx, asking));
        assert!(!crate::theme_snapshot().reduced_motion);
        assert!(!crate::foundation().motion.reduced);
    }

    #[gpui::test]
    fn the_users_request_for_reduced_motion_adds_to_the_applications(cx: &mut TestAppContext) {
        let (cx, _dir, _seen) = follow_dir(cx, DARK);
        let snapshot = crate::theme_snapshot();
        let asked = |on: bool| {
            Arc::new(ThemeSnapshot {
                reduced_motion: on,
                ..(*snapshot).clone()
            })
        };
        assert!(!crate::foundation().motion.reduced);

        crate::theme::publish(asked(true));
        assert!(crate::foundation().motion.reduced);
        assert_eq!(
            crate::foundation()
                .motion
                .transition(odk_theme::foundation::MotionDuration::Normal),
            std::time::Duration::ZERO
        );

        crate::theme::publish(asked(false));
        assert!(!crate::foundation().motion.reduced, "taken back");

        // The application's choice stays on when the user's goes.
        cx.update(|_, cx| crate::set_reduced_motion(cx, true));
        crate::theme::publish(asked(true));
        crate::theme::publish(asked(false));
        assert!(crate::foundation().motion.reduced);
        // and the user's stays on when the application's goes.
        crate::theme::publish(asked(true));
        cx.update(|_, cx| crate::set_reduced_motion(cx, false));
        assert!(crate::foundation().motion.reduced);
    }

    /// A theme carries colours only (ADR 0035): a change of theme leaves the foundation tokens
    /// exactly as they were, and keeps the application's reduced motion.
    #[gpui::test]
    fn a_theme_change_leaves_the_foundation_alone(cx: &mut TestAppContext) {
        let (cx, dir, _seen) = follow_dir(cx, DARK);
        cx.update(|_, cx| {
            crate::configure(cx, |c| {
                c.density = Density::Compact;
                c.reduced_motion = true;
            })
        });
        let foundation = crate::foundation();

        fs::write(dir.path().join("colors.toml"), light()).unwrap();
        reload(cx);
        assert_eq!(
            crate::theme_snapshot().semantic.surface.canvas.to_hex(),
            "#fafafa"
        );
        assert!(
            Arc::ptr_eq(&foundation, &crate::foundation()),
            "not even rebuilt"
        );
        assert_eq!(crate::foundation().controls.metrics().height, 28.0);
        assert!(
            crate::foundation().motion.reduced,
            "reduced motion outlives the theme change"
        );
    }

    /// A change of colour options resolves the semantic colours again from the palette already
    /// read, with the theme's files gone, publishes them on the next frame, and leaves the
    /// foundation alone.
    #[gpui::test]
    fn colour_options_reach_the_snapshot_without_reading_files(cx: &mut TestAppContext) {
        let (cx, dir, seen) = follow_dir(cx, DARK);
        let before = crate::theme_snapshot();
        let foundation = crate::foundation();
        fs::remove_dir_all(dir.path()).unwrap();
        let strict = ResolveOptions {
            text_contrast: 7.0,
            strategy: Strategy::Strict,
            ..Default::default()
        };
        cx.update(|_, cx| crate::set_colour_options(cx, strict.clone()).unwrap());
        cx.run_until_parked();
        let after = crate::theme_snapshot();
        assert_eq!(after.generation, before.generation + 1);
        assert_eq!(after.resolution.options, strict);
        assert!(Arc::ptr_eq(&after.palette, &before.palette));
        assert!(Arc::ptr_eq(&foundation, &crate::foundation()));
        assert_eq!(seen.generation.get(), after.generation, "views drew it");
        cx.update(|_, cx| assert_eq!(crate::colour_options(cx), strict));
        let diagnostics = cx.update(|_, cx| crate::theme_diagnostics(cx)).unwrap();
        assert!(diagnostics.failure.is_none());
        assert_eq!(diagnostics.strategy, Strategy::Strict);
    }

    /// Options the theme cannot meet change nothing that is drawn, and diagnostics say why.
    #[gpui::test]
    fn colour_options_the_theme_cannot_meet_keep_what_is_drawn(cx: &mut TestAppContext) {
        let (cx, _dir, _seen) = follow_dir(cx, DARK);
        let before = crate::theme_snapshot();
        let impossible = ResolveOptions {
            text_contrast: 21.0,
            ..Default::default()
        };
        cx.update(|_, cx| crate::set_colour_options(cx, impossible).unwrap());
        cx.run_until_parked();
        assert!(Arc::ptr_eq(&before, &crate::theme_snapshot()));
        let diagnostics = cx.update(|_, cx| crate::theme_diagnostics(cx)).unwrap();
        assert_eq!(diagnostics.fallback, Some(Fallback::LastGood));
        assert!(matches!(
            *diagnostics.failure.unwrap().error,
            odk_theme::Error::Resolve(_)
        ));
        // Options below WCAG's minimums never reach the manager.
        let weak = ResolveOptions {
            text_contrast: 3.0,
            ..Default::default()
        };
        cx.update(|_, cx| assert!(crate::set_colour_options(cx, weak).is_err()));
    }

    /// The semantic colours are resolved when the theme or the options change, never while
    /// drawing: redraws and a change of density leave the very same snapshot in place.
    #[gpui::test]
    fn rendering_never_resolves_colours(cx: &mut TestAppContext) {
        let (cx, _dir, seen) = follow_dir(cx, DARK);
        let semantic = crate::theme_snapshot().semantic.clone();
        let renders = seen.renders.get();
        for _ in 0..3 {
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
        }
        cx.update(|_, cx| crate::configure(cx, |c| c.density = Density::Compact));
        cx.run_until_parked();
        assert!(seen.renders.get() > renders);
        assert!(Arc::ptr_eq(&semantic, &crate::theme_snapshot().semantic));
    }

    /// The component tokens are published with the snapshot they read, so a view never sees
    /// colours of one theme generation with another: a theme change brings new colours and keeps
    /// the foundation; a change of density keeps the very same colours; a redraw rebuilds nothing.
    #[gpui::test]
    fn component_tokens_follow_the_theme_and_the_foundation(cx: &mut TestAppContext) {
        let (cx, dir, _seen) = follow_dir(cx, DARK);
        let first = crate::components();
        let snapshot = crate::theme_snapshot();
        assert_eq!(first.generation(), snapshot.generation);
        assert!(Arc::ptr_eq(first.semantic(), &snapshot.semantic));
        assert!(Arc::ptr_eq(first.foundation(), &crate::foundation()));

        fs::write(dir.path().join("colors.toml"), light()).unwrap();
        reload(cx);
        let themed = crate::components();
        let snapshot = crate::theme_snapshot();
        assert_eq!(themed.generation(), snapshot.generation);
        assert!(Arc::ptr_eq(themed.semantic(), &snapshot.semantic));
        assert!(
            Arc::ptr_eq(themed.foundation(), first.foundation()),
            "not rebuilt"
        );
        let primary = |c: &odk_theme::component::ComponentTokens| {
            c.button(
                ButtonVariant::Primary,
                Density::Default,
                ButtonLayout::Text,
                States::NONE,
            )
        };
        assert_ne!(primary(&themed).current, primary(&first).current);

        cx.update(|_, cx| crate::configure(cx, |c| c.density = Density::Compact));
        cx.run_until_parked();
        let dense = crate::components();
        assert!(
            Arc::ptr_eq(dense.semantic(), themed.semantic()),
            "no colour looked at"
        );
        assert_eq!(dense.generation(), themed.generation());
        assert_eq!(dense.density(None), Density::Compact);
        assert_eq!(primary(&dense).colours, primary(&themed).colours);

        for _ in 0..3 {
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
        }
        assert!(Arc::ptr_eq(&dense, &crate::components()));
    }

    /// `treatment` and `control_geometry` put a component token on a GPUI element as it is,
    /// 2 px corners included.
    #[test]
    fn component_tokens_go_onto_gpui_elements_as_they_are() {
        super::install_test_theme();
        let style = crate::components().button(
            ButtonVariant::Primary,
            Density::Default,
            ButtonLayout::Text,
            States::NONE,
        );
        let mut el = div()
            .treatment(&style.current)
            .control_geometry(&style.geometry);
        let s = el.style().clone();
        assert_eq!(
            s.background,
            Some(gpui::Fill::from(super::colour(style.current.background)))
        );
        assert_eq!(
            s.border_color,
            Some(super::colour(style.current.border).into())
        );
        assert_eq!(
            s.text.as_ref().and_then(|t| t.color),
            Some(super::colour(style.current.foreground).into())
        );
        assert_eq!(s.size.height, Some(gpui::px(34.0).into()));
        assert_eq!(s.corner_radii.top_left, Some(gpui::px(2.0).into()));
        assert_eq!(s.border_widths.left, Some(gpui::px(1.0).into()));
        assert_eq!(s.padding.top, Some(gpui::px(7.0).into()));

        // `.hover()` and `.active()` hand their closures a `StyleRefinement`, which takes the
        // same calls.
        let hover = style.colours.hover;
        let refined = gpui::StyleRefinement::default().treatment(&hover);
        assert_eq!(
            refined.background,
            Some(gpui::Fill::from(super::colour(hover.background)))
        );
        assert_eq!(
            refined.border_color,
            Some(super::colour(hover.border).into())
        );
    }

    /// A tone maps onto the component layer's Role, and its mark is the status mark it always was.
    #[test]
    fn tones_map_onto_the_component_layer() {
        use odk_theme::semantic::Role;

        use crate::Tone;
        for (tone, role, mark) in [
            (Tone::Info, Role::Info, "i"),
            (Tone::Success, Role::Success, "✓"),
            (Tone::Warning, Role::Warning, "!"),
            (Tone::Danger, Role::Danger, "×"),
        ] {
            assert_eq!(Role::from(tone), role);
            assert_eq!(tone.mark(), mark);
        }
    }

    /// `render` reads the published theme, never the files: with the theme's directory gone,
    /// views still draw it, and no reload is attempted.
    #[gpui::test]
    fn render_never_reads_the_theme_files(cx: &mut TestAppContext) {
        let (cx, dir, seen) = follow_dir(cx, DARK);
        let renders = seen.renders.get();
        fs::remove_dir_all(dir.path()).unwrap();
        for _ in 0..3 {
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
        }
        assert!(seen.renders.get() > renders);
        assert_eq!(*seen.background.borrow(), "#000000");
        let diagnostics = cx.update(|_, cx| crate::theme_diagnostics(cx)).unwrap();
        assert!(diagnostics.failure.is_none());
    }
}
