//! The one place a theme is loaded, kept and replaced (`docs/spec/odk-source-palette.md`,
//! "Reload"), with the fallbacks of ADR 0041, and where its semantic colours are resolved
//! (`docs/spec/odk-semantic-tokens.md`, "Publishing").

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use crate::component;
use crate::host::{Host, HostState};
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
    /// Whether the user asks for reduced motion: the compositor's animations are off, as `odkd`
    /// reports it (ADR 0048, ADR 0062). When it says nothing, or no `odkd` answers, the user has
    /// not asked. The user's own file is not a source. The application's own choice is separate,
    /// and either one reduces motion.
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

/// What one reading of a theme gave.
struct Read {
    name: Option<String>,
    colors: Option<PathBuf>,
    palette: Palette,
    text: TextScale,
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
    /// What `odkd` says (ADR 0062): the palette, when there is no source, and always the user's
    /// text size, request for reduced motion and font.
    host: Option<Host>,
    /// What the host last said about the user's request for reduced motion: `None` when it
    /// has not said. The user's own file is not a source.
    compositor_animations: Option<bool>,
    /// The monospace family the host last named.
    font_family: Option<String>,
}

impl ThemeManager {
    /// A manager for `source`, on the emergency palette until its first reload. It follows no
    /// host, so what it loads never depends on the machine's own settings; [`Self::with_host`]
    /// adds the user's.
    pub fn new(source: ThemeSource) -> Self {
        Self {
            source: Some(source),
            snapshot: Arc::new(ThemeSnapshot::emergency()),
            failure: None,
            options: ResolveOptions::default(),
            host: None,
            compositor_animations: None,
            font_family: None,
        }
    }

    /// Also follows `host` for the user's text size (ADR 0043), request for reduced motion
    /// (ADR 0048) and font. The colours stay `source`'s.
    pub fn with_host(mut self, host: Option<Host>) -> Self {
        self.host = host;
        self
    }

    /// The host this manager follows, if any, for [`Host::on_change`].
    pub fn host(&self) -> Option<&Host> {
        self.host.as_ref()
    }

    /// The monospace family `odkd` last named, if it has (ADR 0062).
    pub fn font_family(&self) -> Option<&str> {
        self.font_family.as_deref()
    }

    /// A manager that follows `odkd` for everything: the palette, the user's text size, request
    /// for reduced motion and font (ADR 0062). It waits a moment for the first line, so the
    /// first frame has the user's theme. When no `odkd` answers it stays on the emergency
    /// palette, and its diagnostics say why.
    pub fn current() -> Self {
        Self::from_host(Host::from_environment())
    }

    /// [`Self::current`], with `host` in place of the environment's.
    pub fn from_host(host: Option<Host>) -> Self {
        Self {
            source: None,
            snapshot: Arc::new(ThemeSnapshot::emergency()),
            failure: host.is_none().then(|| Failure {
                colors: None,
                error: Arc::new(Error::NoHost),
            }),
            options: ResolveOptions::default(),
            host,
            compositor_animations: None,
            font_family: None,
        }
    }

    /// A manager for `source` that takes the user's text size from `odkd` as [`Self::current`]
    /// does, so a theme loaded to be previewed in a running application is built at the user's
    /// text size. Use [`Self::new`] for a manager that must not depend on the machine.
    pub fn following_host(source: ThemeSource) -> Self {
        Self::new(source).with_host(Host::from_environment())
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
        let said = self.host.as_ref().and_then(Host::state);
        let Read {
            name,
            colors,
            palette,
            mut text,
        } = match self.read(said.as_ref()) {
            Ok(read) => read,
            Err(failed) => return failed,
        };
        // The host's word is the user's when it gives one (ADR 0048, ADR 0043). What it does
        // not say is not asked of anyone else, and the user's own file is ignored.
        if let Some(said) = said {
            if let Some(size) = said.base_size {
                text = TextScale::from_base_size(size);
            }
            self.compositor_animations = said.reduced_motion.map(|reduced| !reduced);
            self.font_family = said.font_family;
        }
        let reduced_motion = self.compositor_animations == Some(false);
        // Nothing partial is published: a theme whose semantic colours cannot be resolved is
        // a failed reload like any other.
        let (semantic, resolution) = match self.snapshot.resolve(&palette, &self.options) {
            Ok(resolved) => resolved,
            Err(error) => return self.fail(colors, error),
        };
        self.failure = None;
        let next = ThemeSnapshot {
            name,
            colors: colors.clone(),
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

    /// The theme and the text size it comes with: from the source's files when there is a
    /// source, else from what the host said. A failure is already recorded.
    fn read(&mut self, said: Option<&HostState>) -> Result<Read, Reload> {
        if let Some(source) = &self.source {
            let files = match source.resolve() {
                Ok(files) => files,
                Err(error) => return Err(self.fail(None, error)),
            };
            return match load(&files) {
                Ok(Loaded { palette, text }) => Ok(Read {
                    name: files.name,
                    colors: Some(files.colors),
                    palette,
                    text,
                }),
                Err(error) => Err(self.fail(Some(files.colors), error)),
            };
        }
        let Some(said) = said else {
            return Err(self.fail(None, Error::NoHost));
        };
        let Some(colors) = said.colors_toml() else {
            return Err(self.fail(None, Error::HostWithoutColors));
        };
        match Palette::parse(&colors) {
            Ok(palette) => Ok(Read {
                name: None,
                colors: None,
                palette,
                text: TextScale::default(),
            }),
            Err(error) => Err(self.fail(None, error)),
        }
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
    /// Whether the user asks for reduced motion (ADR 0048).
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
            (true, _) => writeln!(f, "reduced motion: the compositor's animations are off")?,
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
