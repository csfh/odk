//! The theme manager on real directories: switching the way `omarchy-theme-set` does, broken
//! themes, broken links, and when a reload counts as a change.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use odk_theme::semantic::{ResolveError, ResolveOptions, Strategy, resolve};
use odk_theme::{
    Error, Fallback, Hyprland, Mode, Palette, Reload, ThemeManager, ThemeSnapshot, ThemeSource,
    foundation,
};

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .join("colors.toml");
    fs::read_to_string(path).unwrap()
}

/// A private `$XDG_STATE_HOME/omarchy/current`.
struct Current {
    _root: tempfile::TempDir,
    current: PathBuf,
}

impl Current {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let current = root.path().join("omarchy/current");
        fs::create_dir_all(&current).unwrap();
        Self {
            _root: root,
            current,
        }
    }

    fn source(&self) -> ThemeSource {
        ThemeSource::Current {
            current: self.current.clone(),
        }
    }

    /// Switches theme the way `omarchy-theme-set` does: stage `next-theme`, delete `theme`,
    /// move `next-theme` into place, then write `theme.name`.
    fn switch(&self, name: &str, colors: &str) {
        let next = self.current.join("next-theme");
        fs::create_dir_all(&next).unwrap();
        fs::write(next.join("colors.toml"), colors).unwrap();
        let theme = self.current.join("theme");
        if theme.exists() {
            fs::remove_dir_all(&theme).unwrap();
        }
        fs::rename(&next, &theme).unwrap();
        fs::write(self.current.join("theme.name"), format!("{name}\n")).unwrap();
    }

    fn colors(&self) -> PathBuf {
        self.current.join("theme/colors.toml")
    }
}

fn changed(reload: Reload) -> Arc<ThemeSnapshot> {
    match reload {
        Reload::Changed(snapshot) => snapshot,
        other => panic!("expected a change, got {other:?}"),
    }
}

fn failed(reload: Reload) -> Arc<Error> {
    match reload {
        Reload::Failed(error) => error,
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[test]
fn switching_between_valid_themes_publishes_each_once() {
    let c = Current::new();
    let mut m = ThemeManager::new(c.source());

    c.switch("tokyo-night", &fixture("tokyo-night"));
    let first = changed(m.reload());
    assert_eq!(first.name.as_deref(), Some("tokyo-night"));
    assert_eq!(first.colors.as_deref(), Some(c.colors().as_path()));
    assert_eq!(first.generation, 1);
    assert!(!first.emergency);
    assert_eq!(first.palette.background.to_hex(), "#1a1b26");

    c.switch("catppuccin-latte", &fixture("catppuccin-latte"));
    let second = changed(m.reload());
    assert_eq!(second.name.as_deref(), Some("catppuccin-latte"));
    assert_eq!(second.palette.mode, Mode::Light);
    assert_eq!(second.generation, 2);
    assert_eq!(
        first.palette.background.to_hex(),
        "#1a1b26",
        "an earlier snapshot never changes"
    );
}

/// The watcher reports every theme change several times, and sometimes nothing changed at all.
#[test]
fn reloading_an_unchanged_theme_publishes_nothing() {
    let c = Current::new();
    let mut m = ThemeManager::new(c.source());
    c.switch("nord", &fixture("nord"));
    let first = changed(m.reload());
    for _ in 0..3 {
        assert!(matches!(m.reload(), Reload::Unchanged));
    }
    assert!(Arc::ptr_eq(&m.snapshot(), &first));
    assert_eq!(m.snapshot().generation, 1);
    assert_eq!(m.diagnostics().fallback, None);
}

#[test]
fn editing_the_palette_in_place_is_a_change() {
    let c = Current::new();
    let mut m = ThemeManager::new(c.source());
    c.switch("tokyo-night", &fixture("tokyo-night"));
    changed(m.reload());
    let edited = fixture("tokyo-night").replace("#f7768e", "#ff0000");
    fs::write(c.colors(), edited).unwrap();
    let snapshot = changed(m.reload());
    assert_eq!(snapshot.palette.red.to_hex(), "#ff0000");
    assert_eq!(snapshot.generation, 2);
}

/// `shell.toml` changes what is drawn without touching the palette, through its text size and
/// nothing else: the control colours it used to carry are not read (ADR 0047), so an edit
/// to them alone publishes nothing.
#[test]
fn editing_shell_toml_alone_is_a_change_only_when_it_changes_what_is_drawn() {
    let c = Current::new();
    let mut m = ThemeManager::new(c.source());
    c.switch("nord", &fixture("nord"));
    let before = changed(m.reload());
    let shell = c.current.join("theme/shell.toml");
    fs::write(&shell, "[controls]\npressed-fill-alpha = 0.5\n").unwrap();
    assert!(matches!(m.reload(), Reload::Unchanged));
    fs::write(&shell, "[font]\nbase-size = 14\n").unwrap();
    let after = changed(m.reload());
    assert_eq!(after.palette, before.palette);
    assert_eq!(after.semantic, before.semantic);
    assert_eq!(after.text.base_size(), 14.0);
}

#[test]
fn a_malformed_theme_keeps_the_last_good_one_until_it_is_fixed() {
    let c = Current::new();
    let mut m = ThemeManager::new(c.source());
    c.switch("tokyo-night", &fixture("tokyo-night"));
    let good = changed(m.reload());

    c.switch("custom-malformed", &fixture("custom-malformed"));
    let error = failed(m.reload());
    assert!(matches!(*error, Error::BadColor { key: "red", .. }));
    assert!(
        Arc::ptr_eq(&m.snapshot(), &good),
        "nothing partial is drawn"
    );
    let d = m.diagnostics();
    assert_eq!(d.fallback, Some(Fallback::LastGood));
    assert_eq!(d.name.as_deref(), Some("tokyo-night"));
    let failure = d.failure.clone().unwrap();
    assert_eq!(failure.colors.as_deref(), Some(c.colors().as_path()));
    let text = d.to_string();
    assert!(text.contains("tokyo-night"), "{text}");
    assert!(text.contains("`red`"), "{text}");
    assert!(text.contains("last theme that loaded is kept"), "{text}");

    // The author fixes the theme in place.
    fs::write(
        c.colors(),
        fixture("custom-malformed").replace("#f7768\"", "#f7768e\""),
    )
    .unwrap();
    let fixed = changed(m.reload());
    assert_eq!(fixed.name.as_deref(), Some("custom-malformed"));
    assert_eq!(fixed.generation, 2);
    assert_eq!(m.diagnostics().fallback, None);
    assert!(m.diagnostics().failure.is_none());
}

/// A reload can land between Omarchy's delete and move, when there is no theme at all.
#[test]
fn a_reload_mid_switch_fails_quietly_and_the_next_one_succeeds() {
    let c = Current::new();
    let mut m = ThemeManager::new(c.source());
    c.switch("nord", &fixture("nord"));
    let nord = changed(m.reload());

    fs::remove_dir_all(c.current.join("theme")).unwrap();
    let error = failed(m.reload());
    assert!(
        matches!(&*error, Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound)
    );
    assert!(Arc::ptr_eq(&m.snapshot(), &nord));

    c.switch("vantablack", &fixture("vantablack"));
    assert_eq!(changed(m.reload()).name.as_deref(), Some("vantablack"));
}

/// `omarchy-theme-set` writes `theme.name` after the move, so a reload can see the new colours
/// under the old name; the next one sees the name change, and that is published too.
#[test]
fn the_name_arriving_after_the_colours_is_a_change() {
    let c = Current::new();
    let mut m = ThemeManager::new(c.source());
    c.switch("nord", &fixture("nord"));
    changed(m.reload());
    fs::write(c.colors(), fixture("vantablack")).unwrap();
    let colours_first = changed(m.reload());
    assert_eq!(colours_first.name.as_deref(), Some("nord"));
    fs::write(c.current.join("theme.name"), "vantablack\n").unwrap();
    let named = changed(m.reload());
    assert_eq!(named.name.as_deref(), Some("vantablack"));
    assert_eq!(named.palette, colours_first.palette);
}

#[test]
fn a_broken_theme_symlink_is_a_failure_not_a_crash() {
    let c = Current::new();
    let mut m = ThemeManager::new(c.source());
    c.switch("nord", &fixture("nord"));
    let nord = changed(m.reload());
    fs::remove_dir_all(c.current.join("theme")).unwrap();
    symlink(c.current.join("gone"), c.current.join("theme")).unwrap();
    failed(m.reload());
    assert!(Arc::ptr_eq(&m.snapshot(), &nord));
}

#[test]
fn a_theme_the_application_may_not_read_is_a_permission_error() {
    use std::os::unix::fs::PermissionsExt;
    let c = Current::new();
    let mut m = ThemeManager::new(c.source());
    c.switch("nord", &fixture("nord"));
    changed(m.reload());
    fs::set_permissions(c.colors(), fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(c.colors()).is_ok() {
        eprintln!("skipped: this user can read a file with mode 000");
        return;
    }
    let error = failed(m.reload());
    assert!(
        matches!(&*error, Error::Io { path, source }
            if source.kind() == std::io::ErrorKind::PermissionDenied && *path == c.colors()),
        "{error:?}"
    );
}

#[test]
fn with_no_theme_ever_loaded_the_emergency_palette_is_drawn() {
    let c = Current::new();
    let mut m = ThemeManager::new(c.source());
    failed(m.reload());
    let s = m.snapshot();
    assert!(s.emergency);
    assert_eq!(s.generation, 0);
    let d = m.diagnostics();
    assert_eq!(d.fallback, Some(Fallback::Emergency));
    assert!(d.to_string().contains("emergency palette"));

    c.switch("nord", &fixture("nord"));
    let nord = changed(m.reload());
    assert!(!nord.emergency);
    assert_eq!(nord.generation, 1);
}

#[test]
fn a_named_theme_and_a_directory_load_too() {
    let root = tempfile::tempdir().unwrap();
    let (user, stock) = (root.path().join("user"), root.path().join("stock"));
    fs::create_dir_all(stock.join("nord")).unwrap();
    fs::write(stock.join("nord/colors.toml"), fixture("nord")).unwrap();
    fs::create_dir_all(&user).unwrap();
    // A user theme that is a symlink to a working copy, as many on the development machine are.
    let copy = root.path().join("my-theme-checkout");
    fs::create_dir_all(&copy).unwrap();
    fs::write(copy.join("colors.toml"), fixture("custom-incomplete")).unwrap();
    symlink(&copy, user.join("mine")).unwrap();

    let named = |name: &str| ThemeSource::Named {
        name: name.into(),
        user_themes: user.clone(),
        stock_themes: stock.clone(),
    };
    let nord = changed(ThemeManager::new(named("Nord")).reload());
    assert_eq!(nord.name.as_deref(), Some("nord"));
    let mine = changed(ThemeManager::new(named("mine")).reload());
    assert_eq!(mine.palette.mode, Mode::Light);
    let error = failed(ThemeManager::new(named("missing")).reload());
    assert!(matches!(&*error, Error::NoTheme { .. }));
    let error = failed(ThemeManager::new(named("../stock/nord")).reload());
    assert!(matches!(&*error, Error::BadThemeName(_)));

    let dir = changed(ThemeManager::new(ThemeSource::Dir(stock.join("nord"))).reload());
    assert_eq!(dir.palette, nord.palette);
}

/// A theme carries colours and, at most, the text size (ADR 0043): whichever theme loads, and
/// whatever its `shell.toml` says about spacing and the type scale's own tokens, the sizes and
/// durations components draw are the same, because the foundation takes no theme.
#[test]
fn switching_themes_changes_no_size_or_duration() {
    let c = Current::new();
    let mut m = ThemeManager::new(c.source());
    c.switch("tokyo-night", &fixture("tokyo-night"));
    let first = changed(m.reload());
    for name in ["catppuccin-latte", "vantablack"] {
        c.switch(name, &fixture(name));
        fs::write(
            c.current.join("theme/shell.toml"),
            "[spacing]\nscale = 2.0\n[font]\nbody = 40\n[controls]\nnormal-border-width = 4\n",
        )
        .unwrap();
        let text = changed(m.reload()).text;
        assert_eq!(text, first.text, "{name}");
    }
    // The foundation layer takes no theme at all: only the application's configuration.
    let f = foundation::FoundationTokens::from_config(&foundation::FoundationConfig::default());
    assert_eq!(f.controls.metrics().height, 34.0);
    assert_eq!(f.geometry.border_width, 1.0);
}

/// The text size alone reaches the sizes (ADR 0043): the user's own file over the theme's, a
/// change to it alone is a change that is published, and neither a border nor a duration moves.
#[test]
fn the_text_size_is_followed_and_nothing_else_moves() {
    let c = Current::new();
    let user = c.current.join("user-shell.toml");
    let mut m = ThemeManager::new(c.source()).with_user_shell(user.clone());
    c.switch("tokyo-night", &fixture("tokyo-night"));
    let plain = changed(m.reload());
    assert_eq!(plain.text.base_size(), 12.0);

    // The theme's own size.
    fs::write(
        c.current.join("theme/shell.toml"),
        "[font]\nbase-size = 15\n",
    )
    .unwrap();
    let themed = changed(m.reload());
    assert_eq!(themed.text.base_size(), 15.0);

    // The user's wins, and its arrival alone is a change.
    fs::write(&user, "[font]\nbase-size = 18\n").unwrap();
    let mine = changed(m.reload());
    assert_eq!(mine.text.base_size(), 18.0);
    let config = foundation::FoundationConfig::default();
    let (at_12, at_18) = (
        foundation::FoundationTokens::scaled(&config, plain.text),
        foundation::FoundationTokens::scaled(&config, mine.text),
    );
    assert_eq!((at_12.spacing.s3, at_18.spacing.s3), (12.0, 18.0));
    assert_eq!(at_12.geometry, at_18.geometry);
    assert_eq!(at_12.motion, at_18.motion);
    assert_eq!(mine.palette, plain.palette);
    assert!(matches!(m.reload(), Reload::Unchanged));

    // Taking it away goes back to the theme's, and a file that is not TOML never fails a reload.
    fs::write(&user, "[font").unwrap();
    assert_eq!(changed(m.reload()).text.base_size(), 15.0);
    assert!(m.diagnostics().to_string().contains("text size 15 px"));
}

fn options(change: impl FnOnce(&mut ResolveOptions)) -> ResolveOptions {
    let mut options = ResolveOptions::default();
    change(&mut options);
    options
}

/// Every published snapshot carries the semantic colours of its own palette: they arrive
/// together, never one without the other.
#[test]
fn a_theme_switch_publishes_its_semantic_colours_with_its_palette() {
    let c = Current::new();
    let mut m = ThemeManager::new(c.source());
    for name in ["tokyo-night", "catppuccin-latte", "vantablack"] {
        c.switch(name, &fixture(name));
        let s = changed(m.reload());
        let expected = resolve(&s.palette, &ResolveOptions::default()).unwrap();
        assert_eq!(*s.semantic, expected.colours, "{name}");
        assert_eq!(*s.resolution, expected.report, "{name}");
        assert_eq!(s.semantic.mode, s.palette.mode, "{name}");
    }
}

/// The watcher reports each switch several times; an unchanged theme is not resolved again.
#[test]
fn an_unchanged_theme_is_not_resolved_again() {
    let c = Current::new();
    let mut m = ThemeManager::new(c.source());
    c.switch("nord", &fixture("nord"));
    let first = changed(m.reload());
    // A change to shell.toml alone (its text size) reloads, but the palette is the same: the
    // semantic colours are reused, not resolved again.
    fs::write(
        c.current.join("theme/shell.toml"),
        "[font]\nbase-size = 14\n",
    )
    .unwrap();
    let second = changed(m.reload());
    assert!(Arc::ptr_eq(&first.semantic, &second.semantic));
    assert!(Arc::ptr_eq(&first.resolution, &second.resolution));
}

/// Options change the semantic colours without reading the theme again: with the theme's
/// directory gone, a change of options still publishes, one generation on.
#[test]
fn changing_options_resolves_again_without_reading_files() {
    let c = Current::new();
    let mut m = ThemeManager::new(c.source());
    c.switch("tokyo-night", &fixture("tokyo-night"));
    let before = changed(m.reload());
    fs::remove_dir_all(c.current.join("theme")).unwrap();

    let stricter = options(|o| {
        o.text_contrast = 7.0;
        o.strategy = Strategy::Strict;
    });
    let after = changed(m.set_options(stricter.clone()).unwrap());
    assert_eq!(after.generation, before.generation + 1);
    assert_eq!(after.resolution.options, stricter);
    assert_ne!(after.semantic, before.semantic);
    assert!(
        Arc::ptr_eq(&after.palette, &before.palette),
        "the palette is kept"
    );
    assert_eq!(after.text, before.text, "and the text size");
    assert_eq!(m.options(), &stricter);
    let text = m.diagnostics().to_string();
    assert!(text.contains("semantic colours: strict strategy"), "{text}");

    // The same options again change nothing.
    assert!(matches!(
        m.set_options(stricter).unwrap(),
        Reload::Unchanged
    ));
    // Options that weaken WCAG are refused and change nothing.
    let weak = options(|o| o.text_contrast = 3.0);
    assert!(m.set_options(weak).is_err());
    assert_eq!(m.snapshot().generation, after.generation);
    assert_eq!(m.options().text_contrast, 7.0);
}

/// Options the theme's colours cannot meet keep the last snapshot; going back to options it
/// can meet clears the failure, and options that change the colours publish them.
#[test]
fn options_that_cannot_be_met_keep_the_last_snapshot() {
    let c = Current::new();
    let mut m = ThemeManager::new(c.source());
    c.switch("tokyo-night", &fixture("tokyo-night"));
    let good = changed(m.reload());

    let impossible = options(|o| o.text_contrast = 21.0);
    let error = failed(m.set_options(impossible).unwrap());
    assert!(matches!(
        &*error,
        Error::Resolve(ResolveError::Unreachable { .. })
    ));
    assert!(
        Arc::ptr_eq(&m.snapshot(), &good),
        "nothing partial is drawn"
    );
    let d = m.diagnostics();
    assert_eq!(d.fallback, Some(Fallback::LastGood));
    assert!(d.to_string().contains("cannot be resolved"), "{d}");

    // The failed options were never drawn, so the defaults change nothing that is drawn.
    assert!(matches!(
        m.set_options(ResolveOptions::default()).unwrap(),
        Reload::Unchanged
    ));
    assert!(m.diagnostics().failure.is_none());
    assert!(Arc::ptr_eq(&m.snapshot(), &good));
    changed(m.set_options(options(|o| o.text_contrast = 7.0)).unwrap());
}

/// A theme that parses but whose colours no text can be read on: a mid-grey canvas, where
/// only black text reaches 4.5:1, under near-black panels, where only white text does.
const UNREADABLE: &str = r##"
mode = "dark"
accent = "#7aa2f7"
muted = "#888888"
background = "#777777"
dark_background = "#202020"
lighter_background = "#202020"
foreground = "#ffffff"
red = "#f7768e"
yellow = "#e0af68"
green = "#9ece6a"
blue = "#7aa2f7"
"##;

/// A theme whose semantic colours cannot be resolved is a failed reload like any other: the
/// last good theme stays, palette and text size and all, and switching back to a theme that
/// resolves clears the failure.
#[test]
fn a_theme_whose_colours_cannot_be_resolved_keeps_the_last_good_one() {
    let c = Current::new();
    let mut m = ThemeManager::new(c.source());
    c.switch("vantablack", &fixture("vantablack"));
    let vantablack = changed(m.reload());

    c.switch("unreadable", UNREADABLE);
    assert!(Palette::parse(UNREADABLE).is_ok(), "it parses");
    let error = failed(m.reload());
    assert!(matches!(&*error, Error::Resolve(_)), "{error}");
    assert!(Arc::ptr_eq(&m.snapshot(), &vantablack));
    let d = m.diagnostics();
    assert_eq!(d.fallback, Some(Fallback::LastGood));
    assert_eq!(
        d.failure.unwrap().colors.as_deref(),
        Some(c.colors().as_path())
    );

    c.switch("vantablack", &fixture("vantablack"));
    assert!(matches!(m.reload(), Reload::Unchanged));
    assert!(m.diagnostics().failure.is_none());
}

/// The emergency palette resolves under the defaults; options it cannot meet keep that
/// resolution and say so, and never panic.
#[test]
fn the_emergency_palette_always_has_semantic_colours() {
    let emergency = ThemeSnapshot::emergency();
    assert_eq!(*emergency.resolution, {
        let palette = Palette::clone(&emergency.palette);
        resolve(&palette, &ResolveOptions::default())
            .unwrap()
            .report
    });
    assert!(emergency.resolution.monochrome, "it adds no hue");

    let mut m = ThemeManager::new(ThemeSource::Dir(PathBuf::from("/nonexistent")));
    let start = m.snapshot();
    failed(m.set_options(options(|o| o.text_contrast = 21.0)).unwrap());
    assert!(Arc::ptr_eq(&m.snapshot(), &start));
    assert_eq!(m.diagnostics().fallback, Some(Fallback::Emergency));

    // Options it can meet are applied to it, and it stays the emergency palette.
    let s = changed(m.set_options(options(|o| o.text_contrast = 7.0)).unwrap());
    assert!(s.emergency);
    assert_eq!(s.resolution.options.text_contrast, 7.0);
}

/// A theme reloaded from unchanged content gives the same semantic colours, in a new manager
/// as in the old.
#[test]
fn reloading_the_same_content_gives_the_same_colours() {
    let c = Current::new();
    c.switch("catppuccin", &fixture("catppuccin"));
    let a = changed(ThemeManager::new(c.source()).reload());
    let b = changed(ThemeManager::new(c.source()).reload());
    assert_eq!(a.semantic, b.semantic);
    assert_eq!(a.resolution, b.resolution);
}

/// A compositor of the test's own: `hypr/test/.socket.sock` in a private runtime directory,
/// answering every question with whatever `animated` says now. The machine's compositor is never
/// asked.
struct Compositor {
    _runtime: tempfile::TempDir,
    hyprland: Hyprland,
    animated: Arc<std::sync::atomic::AtomicBool>,
}

impl Compositor {
    fn new(animated: bool) -> Self {
        let runtime = tempfile::tempdir().unwrap();
        let flag = Self::serve_in(runtime.path(), "test", animated);
        Self {
            hyprland: Hyprland::at(runtime.path(), Some("test".into())),
            _runtime: runtime,
            animated: flag,
        }
    }

    /// A compositor of instance `signature` under `runtime`, answering with what the flag it
    /// returns says now.
    fn serve_in(
        runtime: &Path,
        signature: &str,
        animated: bool,
    ) -> Arc<std::sync::atomic::AtomicBool> {
        use std::io::{Read, Write};
        use std::os::unix::net::UnixListener;
        use std::sync::atomic::{AtomicBool, Ordering};
        let dir = runtime.join("hypr").join(signature);
        fs::create_dir_all(&dir).unwrap();
        let listener = UnixListener::bind(dir.join(".socket.sock")).unwrap();
        let animated = Arc::new(AtomicBool::new(animated));
        let now = animated.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                let mut question = [0u8; 128];
                let _ = stream.read(&mut question);
                let reply = format!(
                    "{{\"option\": \"animations:enabled\", \"bool\": {}, \"set\": true }}",
                    now.load(Ordering::SeqCst)
                );
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        animated
    }

    fn animate(&self, on: bool) {
        self.animated.store(on, std::sync::atomic::Ordering::SeqCst);
    }
}

/// ADR 0048: the compositor's animation setting is the user's request for reduced motion, it
/// outranks the key in the user's `shell.toml` while it answers, and the key counts when it does
/// not.
#[test]
fn the_compositors_animations_are_the_users_request_for_reduced_motion() {
    let c = Current::new();
    c.switch("nord", &fixture("nord"));
    let user = c.current.join("user-shell.toml");
    let compositor = Compositor::new(true);
    let mut m = ThemeManager::new(c.source())
        .with_user_shell(user.clone())
        .with_compositor(compositor.hyprland.clone());
    let first = changed(m.reload());
    assert!(!first.reduced_motion, "animations on: no request");
    assert_eq!(m.diagnostics().compositor_animations, Some(true));

    // Animations turned off: a change, and the request, with the theme as it was.
    compositor.animate(false);
    let off = changed(m.reload());
    assert!(off.reduced_motion);
    assert_eq!(off.semantic, first.semantic);
    assert_eq!(m.diagnostics().compositor_animations, Some(false));
    assert!(
        matches!(m.reload(), Reload::Unchanged),
        "same answer, no change"
    );

    // The user's key does not turn it off while the compositor says animations are on, and
    // does not turn it on when it says they are off.
    fs::write(&user, "[motion]\nreduced = true\n").unwrap();
    compositor.animate(true);
    assert!(
        !changed(m.reload()).reduced_motion,
        "the compositor outranks the key"
    );
    fs::write(&user, "[motion]\nreduced = false\n").unwrap();
    compositor.animate(false);
    assert!(changed(m.reload()).reduced_motion);

    // A manager with no compositor, or one that does not answer, keeps what the key says.
    let mut alone = ThemeManager::new(c.source()).with_user_shell(user.clone());
    assert!(!changed(alone.reload()).reduced_motion);
    fs::write(&user, "[motion]\nreduced = true\n").unwrap();
    assert!(changed(alone.reload()).reduced_motion);
    let nowhere = tempfile::tempdir().unwrap();
    let mut silent = ThemeManager::new(c.source())
        .with_user_shell(user)
        .with_compositor(Hyprland::at(nowhere.path(), None));
    assert!(
        changed(silent.reload()).reduced_motion,
        "unknown falls back to the key"
    );
    assert_eq!(silent.diagnostics().compositor_animations, None);
}

/// ADR 0048: a compositor that is restarted is silent for a moment and then answers from a new
/// instance. The silence changes nothing the user sees; the new answer is taken.
#[test]
fn a_restarting_compositor_keeps_its_last_answer_and_its_next_one_is_taken() {
    let c = Current::new();
    c.switch("nord", &fixture("nord"));
    let compositor = Compositor::new(false);
    let runtime = compositor._runtime.path().to_path_buf();
    // The environment still names the instance that was there when the application started.
    let mut m =
        ThemeManager::new(c.source()).with_compositor(Hyprland::at(&runtime, Some("test".into())));
    assert!(changed(m.reload()).reduced_motion, "animations off");

    // Gone: its directory and everything in it, as when Hyprland exits.
    fs::remove_dir_all(runtime.join("hypr")).unwrap();
    assert!(
        matches!(m.reload(), Reload::Unchanged),
        "silence changes nothing"
    );
    assert_eq!(m.diagnostics().compositor_animations, Some(false));

    // Back as another instance, with animations on this time.
    let again = Compositor::serve_in(&runtime, "after", true);
    assert!(!changed(m.reload()).reduced_motion);
    assert_eq!(m.diagnostics().compositor_animations, Some(true));
    drop(again);
}
