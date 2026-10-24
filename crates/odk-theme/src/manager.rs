//! The one place a theme is loaded, kept and replaced (`docs/spec/odk-source-palette.md`,
//! "Reload"), with the fallbacks of ADR 0041, and where its semantic colours are resolved
//! (`docs/spec/odk-semantic-tokens.md`, "Publishing").

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::component;
use crate::compositor::Hyprland;
use crate::load::{Loaded, check_shell, load};
use crate::semantic::{
    OptionsError, Report, Resolution, ResolveOptions, SemanticColours, Strategy, resolve,
};
use crate::{Error, Mode, Palette, PaletteKey, TextScale, ThemeSource};

/// The emergency palette of ADR 0041: dark, and greys only, with every status colour the grey of
/// the text, so it adds no hue and claims no meaning. Parsed like any theme.
const EMERGENCY_COLORS: &str = r##"
mode = "dark"
accent = "#d0d0d0"
muted = "#8a8a8a"
background = "#1c1c1c"
dark_background = "#141414"
lighter_background = "#2a2a2a"
foreground = "#d0d0d0"
red = "#d0d0d0"
yellow = "#d0d0d0"
green = "#d0d0d0"
blue = "#d0d0d0"
"##;

/// What an application draws with at one generation: a palette, where it came from, the user's
/// text size and the semantic colours resolved from the palette. Immutable once made; share it behind an `Arc`.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeSnapshot {
    /// The theme's name when it is known. `None` for the emergency palette.
    pub name: Option<String>,
    /// The `colors.toml` the palette was read from. `None` when it was not read from a file.
    pub colors: Option<PathBuf>,
    pub palette: Arc<Palette>,
    /// The user's text size as a scale (ADR 0043), read from `shell.toml` with the user's own
    /// over the theme's. The foundation tokens are scaled by it.
    pub text: TextScale,
    /// Whether the user asks for reduced motion: the compositor's animations are off, or, when it
    /// does not answer, the user's own `shell.toml` says `[motion] reduced` (ADR 0046, ADR 0048). The application's own choice is separate, and either one reduces motion.
    pub reduced_motion: bool,
    /// The semantic colours of ADR 0040 (`docs/spec/odk-semantic-tokens.md`), resolved from
    /// `palette` with `resolution.options`, with every component's colours checked against them
    /// (`docs/spec/odk-component-tokens.md`). Components draw them through the component layer.
    pub semantic: Arc<SemanticColours>,
    /// How each semantic colour was chosen, kept so asking never runs the resolver again.
    pub resolution: Arc<Report>,
    /// 0 for the emergency palette a manager starts on and for a snapshot made from text; then
    /// one more for each change a [`ThemeManager`] publishes. A cache built from a snapshot is
    /// still good while the generation is the same.
    pub generation: u64,
    /// Whether this is the emergency palette (ADR 0041).
    pub emergency: bool,
}

impl ThemeSnapshot {
    /// The emergency palette, for when no theme has loaded (ADR 0041), resolved with the
    /// default options.
    pub fn emergency() -> Self {
        Self {
            emergency: true,
            ..Self::from_toml(EMERGENCY_COLORS, None).expect("the emergency palette resolves")
        }
    }

    /// A snapshot of the text of a `colors.toml` and, if there is one, a `shell.toml`, read from
    /// no file, with its semantic colours resolved with the default options. For tests and
    /// tools.
    pub fn from_toml(colors: &str, shell: Option<&str>) -> Result<Self, Error> {
        let palette = Palette::parse(colors)?;
        check_shell(shell)?;
        let resolution = resolve_checked(&palette, &ResolveOptions::default())?;
        Ok(Self {
            name: None,
            colors: None,
            palette: Arc::new(palette),
            text: TextScale::from_shell(shell, None),
            reduced_motion: false,
            semantic: Arc::new(resolution.colours),
            resolution: Arc::new(resolution.report),
            generation: 0,
            emergency: false,
        })
    }

    /// Whether `other` draws the same and comes from the same place, whatever its generation.
    /// The text size and the request for reduced motion are compared as well as the palette,
    /// because a change to `shell.toml` alone changes what is drawn (the rest of a theme's
    /// `shell.toml` is not read, ADR 0047); and the semantic colours and their options, because
    /// a change of options alone changes them.
    fn same_as(&self, other: &Self) -> bool {
        self.name == other.name
            && self.colors == other.colors
            && self.emergency == other.emergency
            && self.palette == other.palette
            && self.text == other.text
            && self.reduced_motion == other.reduced_motion
            && self.semantic == other.semantic
            && self.resolution.options == other.resolution.options
    }

    /// The semantic colours of `palette` with `options`: this snapshot's own when it already
    /// resolved the same palette with the same options, so an unchanged theme is never resolved
    /// twice.
    fn resolve(
        &self,
        palette: &Palette,
        options: &ResolveOptions,
    ) -> Result<(Arc<SemanticColours>, Arc<Report>), Error> {
        if *self.palette == *palette && self.resolution.options == *options {
            return Ok((self.semantic.clone(), self.resolution.clone()));
        }
        let resolution = resolve_checked(palette, options)?;
        Ok((Arc::new(resolution.colours), Arc::new(resolution.report)))
    }
}

/// The semantic colours of `palette` with `options`, once every component's colours have passed
/// their checks too (`docs/spec/odk-component-tokens.md`). A theme that fails either is not
/// published.
fn resolve_checked(palette: &Palette, options: &ResolveOptions) -> Result<Resolution, Error> {
    let resolution = resolve(palette, options)?;
    component::validate(&resolution.colours, options)?;
    Ok(resolution)
}

/// What a [`ThemeManager::reload`] did.
#[derive(Debug, Clone)]
pub enum Reload {
    /// A different theme, or the same theme with different colours, loaded and is now the
    /// snapshot. Redraw.
    Changed(Arc<ThemeSnapshot>),
    /// The theme loaded and is what the snapshot already holds. Nothing to redraw.
    Unchanged,
    /// The theme could not be found or loaded. The snapshot is what it was.
    Failed(Arc<Error>),
}

/// A reload that failed, kept until a later one succeeds.
#[derive(Debug, Clone)]
pub struct Failure {
    /// The `colors.toml` it tried to read, when it got that far.
    pub colors: Option<PathBuf>,
    pub error: Arc<Error>,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.colors {
            Some(path) => write!(f, "{}: {}", path.display(), self.error),
            None => write!(f, "{}", self.error),
        }
    }
}

/// Which fallback of ADR 0041 is being drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fallback {
    /// The last theme that loaded, because the latest reload failed.
    LastGood,
    /// The emergency palette, because no theme has loaded.
    Emergency,
}

/// Loads the theme an application follows, keeps the last one that loaded, and says whether a
/// reload changed anything. It reads files only in [`ThemeManager::reload`], which a caller runs
/// off the UI thread; reading the snapshot never touches the disk.
#[derive(Debug)]
pub struct ThemeManager {
    source: Option<ThemeSource>,
    snapshot: Arc<ThemeSnapshot>,
    failure: Option<Failure>,
    /// The options every resolution uses from now on.
    options: ResolveOptions,
    /// The user's own `shell.toml`, when this manager reads it (ADR 0043).
    user_shell: Option<PathBuf>,
    /// The user's `fonts.conf`, which this manager does not read but whose changes it reports
    /// (see [`Self::user_files`]).
    user_fonts: Option<PathBuf>,
    /// The compositor whose animation setting is the user's request for reduced motion, when
    /// this manager asks it (ADR 0048).
    compositor: Option<Hyprland>,
    /// What the compositor last said: `None` when it was not asked or has never answered. A
    /// compositor that stops answering keeps this until it answers again.
    compositor_animations: Option<bool>,
}

impl ThemeManager {
    /// A manager for `source`, on the emergency palette until its first reload. It reads no
    /// `shell.toml` but the theme's, so what it loads never depends on the machine's own
    /// settings; [`Self::with_user_shell`] adds the user's.
    pub fn new(source: ThemeSource) -> Self {
        Self {
            source: Some(source),
            snapshot: Arc::new(ThemeSnapshot::emergency()),
            failure: None,
            options: ResolveOptions::default(),
            user_shell: None,
            user_fonts: None,
            compositor: None,
            compositor_animations: None,
        }
    }

    /// Also asks `compositor` whether it animates windows at every reload, and takes the answer
    /// as the user's request for reduced motion (ADR 0048): when animations are off the user
    /// asks, when they are on the user does not, and the `[motion] reduced` of the user's
    /// `shell.toml` counts only while the compositor says nothing. Only [`Self::current`] asks
    /// one; a manager that must not depend on the machine never does.
    pub fn with_compositor(mut self, compositor: Hyprland) -> Self {
        self.compositor = Some(compositor);
        self
    }

    /// The compositor this manager asks, if any, for [`Hyprland::watch_reloads`].
    pub fn compositor(&self) -> Option<&Hyprland> {
        self.compositor.as_ref()
    }

    /// Also reads `path`, the user's own `shell.toml`, for the text size (ADR 0043). A file that
    /// is missing, unreadable or does not parse says nothing.
    pub fn with_user_shell(mut self, path: PathBuf) -> Self {
        self.user_shell = Some(path);
        self
    }

    /// Also reports changes to `path`, the user's `fonts.conf`, where `omarchy font set` names the
    /// font (ADR 0043). The manager reads nothing from it: the application resolves the font.
    pub fn with_user_fonts(mut self, path: PathBuf) -> Self {
        self.user_fonts = Some(path);
        self
    }

    /// The user's own `shell.toml` this manager reads, if any.
    pub fn user_shell(&self) -> Option<&Path> {
        self.user_shell.as_deref()
    }

    /// The user's own `fonts.conf` whose changes are reported, if any.
    pub fn user_fonts(&self) -> Option<&Path> {
        self.user_fonts.as_deref()
    }

    /// The user's files a change to which is worth a reload: `shell.toml`, and `fonts.conf`
    /// (ADR 0043), for [`crate::ThemeWatcher::follow_with`].
    pub fn user_files(&self) -> Vec<PathBuf> {
        [&self.user_shell, &self.user_fonts]
            .into_iter()
            .flatten()
            .cloned()
            .collect()
    }

    /// A manager for Omarchy's current theme ([`ThemeSource::current`]), with the user's own
    /// `~/.config/omarchy/shell.toml`. When the environment gives no state directory it has
    /// nothing to load: it stays on the emergency palette and its diagnostics say why.
    pub fn current() -> Self {
        match ThemeSource::current() {
            Ok(source) => {
                let manager = Self::with_users_files(source);
                match Hyprland::from_environment() {
                    Some(compositor) => manager.with_compositor(compositor),
                    None => manager,
                }
            }
            Err(error) => Self {
                source: None,
                snapshot: Arc::new(ThemeSnapshot::emergency()),
                failure: Some(Failure {
                    colors: None,
                    error: Arc::new(error),
                }),
                options: ResolveOptions::default(),
                user_shell: None,
                user_fonts: None,
                compositor: None,
                compositor_animations: None,
            },
        }
    }

    /// A manager for `source` that reads the user's own files as [`Self::current`] does, so a
    /// theme loaded to be previewed in a running application is built at the user's text size
    /// and with the user's request for reduced motion, and publishing it changes nothing but
    /// colours. Use [`Self::new`] for a manager that must not depend on the machine.
    pub fn with_users_files(source: ThemeSource) -> Self {
        let manager = Self::new(source);
        match user_config_dir() {
            Some(dir) => manager
                .with_user_shell(dir.join("omarchy/shell.toml"))
                .with_user_fonts(dir.join("fontconfig/fonts.conf")),
            None => manager,
        }
    }

    /// The options semantic colours are resolved with.
    pub fn options(&self) -> &ResolveOptions {
        &self.options
    }

    /// Resolves the semantic colours again with `options`, from the palette already loaded:
    /// no file is read. A result that differs is published like a reload, one generation on;
    /// one that cannot be resolved keeps the snapshot as it is and is kept as the failure,
    /// and later reloads keep trying with these options. Invalid options change nothing.
    pub fn set_options(&mut self, options: ResolveOptions) -> Result<Reload, OptionsError> {
        options.validate()?;
        self.options = options;
        let current = self.snapshot.clone();
        let (semantic, resolution) = match current.resolve(&current.palette, &self.options) {
            Ok(resolved) => resolved,
            Err(error) => return Ok(self.fail(None, error)),
        };
        self.failure = None;
        let next = ThemeSnapshot {
            semantic,
            resolution,
            generation: current.generation + 1,
            ..(*current).clone()
        };
        if current.same_as(&next) {
            return Ok(Reload::Unchanged);
        }
        log::info!(
            "semantic colours resolved again with the {} strategy, generation {}",
            self.options.strategy.name(),
            next.generation
        );
        self.snapshot = Arc::new(next);
        Ok(Reload::Changed(self.snapshot.clone()))
    }

    pub fn source(&self) -> Option<&ThemeSource> {
        self.source.as_ref()
    }

    /// What to draw with now: the last theme that loaded, or the emergency palette.
    pub fn snapshot(&self) -> Arc<ThemeSnapshot> {
        self.snapshot.clone()
    }

    /// Finds and reads the theme again. A theme that loads and differs from the snapshot
    /// becomes the snapshot, one generation on; one that loads and does not leaves everything
    /// as it is; one that fails to load is kept as the failure and changes nothing else.
    pub fn reload(&mut self) -> Reload {
        let Some(source) = &self.source else {
            let error = self.failure.as_ref().map(|f| f.error.clone());
            return Reload::Failed(error.unwrap_or_else(|| Arc::new(Error::NoStateDir)));
        };
        let files = match source.resolve() {
            Ok(files) => files,
            Err(error) => return self.fail(None, error),
        };
        let Loaded {
            palette,
            text,
            reduced_motion,
        } = match load(&files, self.user_shell.as_deref()) {
            Ok(loaded) => loaded,
            Err(error) => return self.fail(Some(files.colors), error),
        };
        // The compositor's word is the user's when it gives one (ADR 0048); the key of the
        // user's `shell.toml`, which `oma` writes for an application that cannot reach the
        // compositor, only when it does not. A compositor that has answered and then says nothing,
        // as one does while it restarts, keeps its last answer until it speaks again, so a
        // restart never changes what the user sees.
        self.compositor_animations = self
            .compositor
            .as_ref()
            .and_then(Hyprland::animations_enabled)
            .or(self.compositor_animations);
        let reduced_motion = self
            .compositor_animations
            .map_or(reduced_motion, |animated| !animated);
        // Nothing partial is published: a theme whose semantic colours cannot be resolved is
        // a failed reload like any other.
        let (semantic, resolution) = match self.snapshot.resolve(&palette, &self.options) {
            Ok(resolved) => resolved,
            Err(error) => return self.fail(Some(files.colors), error),
        };
        self.failure = None;
        let next = ThemeSnapshot {
            name: files.name,
            colors: Some(files.colors),
            palette: Arc::new(palette),
            text,
            reduced_motion,
            semantic,
            resolution,
            generation: self.snapshot.generation + 1,
            emergency: false,
        };
        if self.snapshot.same_as(&next) {
            return Reload::Unchanged;
        }
        log::info!(
            "theme {} ({}) loaded from {}, generation {}",
            next.name.as_deref().unwrap_or("without a name"),
            next.palette.mode.name(),
            files_path(&next.colors),
            next.generation
        );
        self.snapshot = Arc::new(next);
        Reload::Changed(self.snapshot.clone())
    }

    fn fail(&mut self, colors: Option<PathBuf>, error: Error) -> Reload {
        let failure = Failure {
            colors,
            error: Arc::new(error),
        };
        // A reload during a theme switch can land between Omarchy's delete and move and fail
        // for a moment, so a failure behind a theme that did load is not worth a warning.
        if self.snapshot.emergency {
            log::warn!("no theme loaded, drawing the emergency palette: {failure}");
        } else {
            log::debug!("theme reload failed, keeping the last theme: {failure}");
        }
        let error = failure.error.clone();
        self.failure = Some(failure);
        Reload::Failed(error)
    }

    /// What the manager is drawing and why, for a developer looking at a wrong theme.
    pub fn diagnostics(&self) -> ThemeDiagnostics {
        let s = &self.snapshot;
        ThemeDiagnostics {
            source: self.source.clone(),
            name: s.name.clone(),
            colors: s.colors.clone(),
            mode: s.palette.mode,
            colours: s.palette.colours().count(),
            missing: s.palette.missing().collect(),
            fallback: match (s.emergency, &self.failure) {
                (true, _) => Some(Fallback::Emergency),
                (false, Some(_)) => Some(Fallback::LastGood),
                (false, None) => None,
            },
            failure: self.failure.clone(),
            generation: s.generation,
            text: s.text,
            reduced_motion: s.reduced_motion,
            compositor_animations: self.compositor_animations,
            strategy: s.resolution.options.strategy,
            monochrome: s.resolution.monochrome,
            adjusted: s.resolution.adjusted().count(),
        }
    }
}

/// `~/.config`, where Omarchy writes the user's text size (`omarchy/shell.toml`) and font
/// (`fontconfig/fonts.conf`). Omarchy finds them from `$HOME`; inside oma's sandbox `$HOME` is
/// the same path (ADR 0043).
fn user_config_dir() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    home.is_absolute().then(|| home.join(".config"))
}

fn files_path(path: &Option<PathBuf>) -> String {
    path.as_ref()
        .map_or_else(|| "no file".to_owned(), |p| p.display().to_string())
}

/// A summary of the theme being drawn, without its colours. `Display` writes it as a few lines.
#[derive(Debug, Clone)]
pub struct ThemeDiagnostics {
    /// What the manager follows; `None` when the environment gave it nothing to follow.
    pub source: Option<ThemeSource>,
    pub name: Option<String>,
    /// The `colors.toml` being drawn. Its directory is the theme's.
    pub colors: Option<PathBuf>,
    pub mode: Mode,
    /// How many palette colours the theme defines, out of [`PaletteKey::ALL`].
    pub colours: usize,
    /// The optional keys the theme leaves out.
    pub missing: Vec<PaletteKey>,
    pub fallback: Option<Fallback>,
    /// The latest reload or change of options, when it failed.
    pub failure: Option<Failure>,
    pub generation: u64,
    /// The text size being drawn (ADR 0043).
    pub text: TextScale,
    /// Whether the user asks for reduced motion (ADR 0046, ADR 0048).
    pub reduced_motion: bool,
    /// What the compositor last said about animating windows: `None` when it was not asked or
    /// has never answered.
    pub compositor_animations: Option<bool>,
    /// The strategy the semantic colours being drawn were resolved with.
    pub strategy: Strategy,
    /// Whether the palette is monochrome, so its semantic colours are greys.
    pub monochrome: bool,
    /// How many semantic colours were changed from the palette colour they came from, for
    /// contrast. The snapshot's `resolution` says which and why.
    pub adjusted: usize,
}

impl fmt::Display for ThemeDiagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let source = match &self.source {
            Some(ThemeSource::Current { current }) => {
                format!("the current theme in {}", current.display())
            }
            Some(ThemeSource::Named { name, .. }) => format!("the theme {name:?}"),
            Some(ThemeSource::Dir(dir)) => format!("the directory {}", dir.display()),
            None => "nothing".to_owned(),
        };
        writeln!(f, "following {source}")?;
        match self.fallback {
            Some(Fallback::Emergency) => writeln!(f, "drawing the emergency palette")?,
            _ => writeln!(
                f,
                "drawing {} ({}), generation {}, from {}",
                self.name.as_deref().unwrap_or("a theme without a name"),
                self.mode.name(),
                self.generation,
                files_path(&self.colors)
            )?,
        }
        writeln!(f, "text size {} px", self.text.base_size())?;
        match (self.reduced_motion, self.compositor_animations) {
            (true, Some(false)) => {
                writeln!(f, "reduced motion: the compositor's animations are off")?
            }
            (true, _) => writeln!(f, "reduced motion asked for in shell.toml")?,
            (false, Some(true)) => writeln!(f, "the compositor's animations are on")?,
            (false, _) => {}
        }
        let missing: Vec<_> = self.missing.iter().map(|k| k.name()).collect();
        write!(f, "{} of {} colours", self.colours, PaletteKey::ALL.len())?;
        if !missing.is_empty() {
            write!(f, "; missing {}", missing.join(", "))?;
        }
        write!(
            f,
            "\nsemantic colours: {} strategy, {} adjusted for contrast{}",
            self.strategy.name(),
            self.adjusted,
            if self.monochrome { ", monochrome" } else { "" }
        )?;
        if let Some(failure) = &self.failure {
            let kept = match self.fallback {
                Some(Fallback::LastGood) => ", so the last theme that loaded is kept",
                _ => "",
            };
            write!(f, "\nthe latest reload failed{kept}: {failure}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_manager_draws_the_emergency_palette_and_says_so() {
        let m = ThemeManager::new(ThemeSource::Dir(PathBuf::from("/nonexistent")));
        let s = m.snapshot();
        assert!(s.emergency);
        assert_eq!(s.generation, 0);
        assert_eq!(s.palette.mode, Mode::Dark);
        let d = m.diagnostics();
        assert_eq!(d.fallback, Some(Fallback::Emergency));
        assert!(d.to_string().contains("emergency palette"), "{d}");
    }

    #[test]
    fn the_emergency_palette_defines_only_the_required_keys() {
        let s = ThemeSnapshot::emergency();
        assert!(s.palette.colours().all(|(k, _)| k.is_required()));
        assert!(s.palette.missing().all(|k| !k.is_required()));
    }
}
