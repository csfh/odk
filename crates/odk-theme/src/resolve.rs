//! The adaptive colour resolver of ADR 0040 (`docs/spec/odk-semantic-tokens.md`): from a
//! [`Palette`] to [`SemanticColours`], checking every colour against what it is drawn on and
//! changing, in OKLCH, only what fails.
//!
//! The pipeline, in the order [`resolve`] runs it:
//!
//! 1. **Surfaces** come from the theme's backgrounds; hover and pressed are foreground tints.
//! 2. **Fills**: for each role, every candidate is judged as a solid fill with each of the
//!    theme's neutrals as its label. The strategy picks one; if none passes, the picked one's
//!    lightness is moved until its best label does.
//! 3. **Tints**: muted and subtle fills are the chosen source colour made translucent until it
//!    is a fixed perceptual step from the canvas.
//! 4. **Text and inks**: every text colour, and each role's text colour, is judged against
//!    every surface and every tint it may sit on, and chosen the same way.
//! 5. **States**: hover and pressed move a solid fill's lightness away from its label, or make
//!    a tint a step stronger; disabled is neutral.
//! 6. **Validation** checks the whole result once more, independently of how it was made.
//!
//! Every choice is recorded in the [`Report`], so a developer can ask why a colour is what it
//! is without running the resolver again.

use std::fmt;

use crate::color::drawn_contrast;
use crate::oklch::{JND, Oklch, hue_distance};
use crate::semantic::{
    BorderColours, Emphasis, EmphasisColours, OptionsError, ResolveOptions, Role, RoleColours,
    Roles, SemanticColours, State, Strategy, SurfaceColours, TextColours, Treatment,
};
use crate::{Color, Mode, NON_TEXT_CONTRAST, Palette, PaletteKey};

/// A colour whose chroma is within one JND of the grey of its lightness reads as grey, so its
/// hue means nothing. A palette whose every colour is that close to grey is monochrome.
const ACHROMATIC: f32 = JND;
/// Two hues of one family: within half the usual distance in OKLCH between neighbouring named
/// hues, which sit about 60° apart (red 29°, yellow 110°, green 142°, cyan 195°, blue 264°).
const FAMILY_HUE: f32 = 30.0;
/// How far a hovered fill moves from its default state, in OKLab: two JNDs, subtle but seen.
const HOVER_STEP: f32 = 2.0 * JND;
/// How far a pressed fill moves: four JNDs, twice hover.
const PRESSED_STEP: f32 = 4.0 * JND;
/// How far a muted fill is from the canvas.
const MUTED_FILL: f32 = 3.0 * JND;
/// How far a muted treatment's border is from the canvas.
const MUTED_BORDER: f32 = 6.0 * JND;
/// How far a separator is from the canvas.
const BORDER_SUBTLE: f32 = 3.0 * JND;
/// How far the border of a card or an overlay is from the canvas.
const BORDER_DEFAULT: f32 = 6.0 * JND;
/// How far a selection the theme does not define is from the canvas, made of the accent.
const SELECTION_FILL: f32 = 6.0 * JND;
/// Disabled text stays below WCAG's non-text minimum against the canvas, so it never passes
/// for active text or an active control.
const DISABLED_MAX: f32 = NON_TEXT_CONTRAST;
/// And at least this, so its words can still be read.
const DISABLED_MIN: f32 = 2.0;
/// The lightness search moves one JND at a time, then halves the last step this many times,
/// to well under one 8-bit step.
const SEARCH_HALVINGS: u32 = 12;
/// The opacity of the scrim behind a dialog or a sheet: the legacy scrim's
/// (`docs/spec/odk-theme-tokens.md`), so the two dim alike.
const SCRIM_OPACITY: f32 = 0.6;

/// The semantic colours of one palette, and how each was chosen.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolution {
    pub colours: SemanticColours,
    pub report: Report,
}

/// Resolves the semantic colours of `palette`. Pure: the same palette and options always give
/// the same result, and nothing is read or drawn. Fails only when the options are invalid or
/// no colour can reach what they ask for; then nothing is returned, so nothing partial can be
/// published.
pub fn resolve(palette: &Palette, options: &ResolveOptions) -> Result<Resolution, ResolveError> {
    options.validate()?;
    let mut resolver = Resolver::new(palette, options);
    let colours = resolver.run()?;
    let report = Report {
        options: options.clone(),
        mode: palette.mode,
        monochrome: resolver.monochrome,
        max_chroma: resolver.max_chroma,
        decisions: resolver.decisions,
        notes: resolver.notes,
    };
    validate(&colours, &report)?;
    Ok(Resolution { colours, report })
}

/// A palette colour that may play a role, with every key that holds the same colour.
#[derive(Debug, Clone)]
struct Candidate {
    keys: Vec<PaletteKey>,
    colour: Color,
    lch: Oklch,
}

/// What one candidate gives for one use.
#[derive(Debug, Clone)]
struct Pick {
    colour: Color,
    contrast: f32,
    /// OKLab distance from the candidate as the theme wrote it.
    change: f32,
    label: Option<(Vec<PaletteKey>, Color)>,
}

/// One candidate judged for one use.
#[derive(Debug, Clone)]
struct Judged {
    /// Its contrast as the theme wrote it, with its best label for a fill.
    as_written: f32,
    passes: bool,
    in_family: bool,
    /// The colour to use: the candidate when it passes, else its nearest lightness that does;
    /// `None` when no lightness does.
    pick: Option<Pick>,
}

struct Resolver<'a> {
    palette: &'a Palette,
    options: &'a ResolveOptions,
    canvas: Color,
    monochrome: bool,
    max_chroma: f32,
    decisions: Vec<Decision>,
    notes: Vec<Note>,
}

/// What every surface but the canvas must carry: text at text contrast, drawn on the surface
/// and in the strongest fill of the foreground over it, a pressed muted treatment. Text loses
/// contrast as the foreground's own fill gets stronger, so that fill stands for hover,
/// pressed and every weaker one.
#[derive(Clone, Copy)]
struct Carry {
    foreground: Color,
    /// Black or white, whichever way the foreground lies from the canvas: the most any text
    /// of the theme can do.
    extreme: Color,
    strongest: Color,
    min: f32,
}

impl Carry {
    fn worst(&self, text: Color, surface: Color) -> f32 {
        worst(text, &[surface, self.strongest.over(surface)])
    }

    /// Whether any text could be read on `surface`.
    fn possible(&self, surface: Color) -> bool {
        self.worst(self.extreme, surface) >= self.min
    }

    /// Whether the theme's own foreground can be read on `surface`.
    fn carries(&self, surface: Color) -> bool {
        self.worst(self.foreground, surface) >= self.min
    }

    /// `surface` as the theme wrote it when any text could be read on it, since text adjusts
    /// more cheaply than a surface; else its nearest lightness that carries the foreground
    /// (or, where the foreground fails even on the canvas, any text).
    fn surface(&self, written: Color, canvas: Color) -> Color {
        if self.possible(written) {
            return written;
        }
        let target = |s: Color| {
            if self.carries(canvas) {
                self.carries(s)
            } else {
                self.possible(s)
            }
        };
        nearest(written, target).unwrap_or(written)
    }
}

/// What a role's fill and tints are made of, before its text colours are chosen.
struct RoleBase {
    role: Role,
    candidates: Vec<Candidate>,
    fill: Color,
    label: Color,
    /// The palette colour the fill was chosen from, as written: tints keep its hue exactly.
    source: Color,
    muted: [Color; 3],
    muted_border: Color,
    subtle: [Color; 2],
}

impl<'a> Resolver<'a> {
    fn new(palette: &'a Palette, options: &'a ResolveOptions) -> Self {
        let max_chroma = palette
            .colours()
            .map(|(_, c)| c.to_oklch().c)
            .fold(0.0, f32::max);
        Self {
            palette,
            options,
            canvas: palette.background.with_alpha(1.0),
            monochrome: max_chroma <= ACHROMATIC,
            max_chroma,
            decisions: Vec::new(),
            notes: Vec::new(),
        }
    }

    fn run(&mut self) -> Result<SemanticColours, ResolveError> {
        let (t, ui) = (self.options.text_contrast, self.options.ui_contrast);
        let p = self.palette;
        let canvas = self.canvas;

        // 1. Surfaces. The canvas and the foreground are the theme's anchors: every other
        // surface must carry the theme's text, also in the foreground's own fills ([`Carry`]),
        // so a surface colour that cannot is moved in lightness until it can. Some themes
        // write `darker_background` as a mid grey no text reads on.
        use {PaletteKey as K, SurfaceToken as S};
        self.keep(Subject::Surface(S::Canvas), K::Background, canvas);
        let foreground = self.opaque(p.foreground);
        let hover = self.tint(Subject::Surface(S::Hover), foreground, HOVER_STEP);
        let pressed = self.tint(Subject::Surface(S::Pressed), foreground, PRESSED_STEP);
        let strongest = self.translucent(
            Subject::Surface(S::Pressed),
            foreground,
            MUTED_FILL + PRESSED_STEP,
        );
        let extreme = if foreground.lightness() < canvas.lightness() {
            Color::BLACK
        } else {
            Color::from_rgba8([255; 4])
        };
        let carry = Carry {
            foreground,
            extreme,
            strongest,
            min: t,
        };
        let recessed = self.surface(S::Recessed, K::DarkBackground, p.dark_background, carry);
        let raised = self.surface(S::Raised, K::LighterBackground, p.lighter_background, carry);
        let overlay = self.surface(
            S::Overlay,
            K::LighterBackground,
            p.lighter_background,
            carry,
        );
        let sunken = match p.darker_background {
            Some(c) => self.surface(S::Sunken, K::DarkerBackground, c, carry),
            None => {
                let (from, to) = (canvas.to_oklch(), recessed.to_oklch());
                let sunken = with_lightness(to, (2.0 * to.l - from.l).clamp(0.0, 1.0));
                let sunken = carry.surface(sunken, canvas);
                self.rule(
                    Subject::Surface(S::Sunken),
                    recessed,
                    sunken,
                    "`darker_background` is missing: `dark_background` one more canvas-to-recessed step in lightness",
                );
                sunken
            }
        };
        let grounds = [canvas, recessed, sunken, raised, overlay];
        let scrim = sunken.with_alpha(SCRIM_OPACITY).quantized();
        self.rule(
            Subject::Surface(S::Scrim),
            sunken,
            scrim,
            "`surface.sunken` at 60% opacity",
        );

        // 2 and 3. Each role's fill and tints.
        let mut bases = Vec::new();
        for role in Role::ALL {
            bases.push(self.role_base(role)?);
        }
        let base = |role: Role| &bases[role as usize];

        let selection = match (p.selection_background, p.selection) {
            (Some(c), _) => self.keep(Subject::Surface(S::Selection), K::SelectionBackground, c),
            (None, Some(c)) => self.keep(Subject::Surface(S::Selection), K::Selection, c),
            // Translucent, like every tint, so it keeps the accent's hue on every surface.
            (None, None) => {
                let accent = base(Role::Accent).source;
                self.tint(Subject::Surface(S::Selection), accent, SELECTION_FILL)
            }
        };

        // 4. Text, on every surface and under hover and pressed. Text inside a role's muted
        // fill is that treatment's foreground, chosen with the role.
        let mut text_grounds = grounds.to_vec();
        for g in grounds {
            text_grounds.extend([hover.over(g), pressed.over(g)]);
        }
        let on_surfaces = "every surface, and under hover and pressed";
        let neutrals = self.collect(Role::Neutral.candidates().iter().copied());
        let primary = self.ink(
            Subject::Text(TextToken::Primary),
            None,
            &neutrals,
            false,
            &text_grounds,
            t,
            on_surfaces,
        )?;
        let muted = self.collect([PaletteKey::Muted]);
        let tertiary = self.ink(
            Subject::Text(TextToken::Tertiary),
            None,
            &muted,
            false,
            &text_grounds,
            t,
            on_surfaces,
        )?;
        let secondary = self.secondary(primary, tertiary, &text_grounds)?;
        let disabled = self.disabled_text();
        let accent_candidates = base(Role::Accent).candidates.clone();
        let link = self.ink(
            Subject::Text(TextToken::Link),
            Some(Role::Accent),
            &accent_candidates,
            true,
            &text_grounds,
            t,
            on_surfaces,
        )?;
        if link.contrast(primary) < NON_TEXT_CONTRAST {
            self.notes.push(Note::LinkNearText {
                contrast: link.contrast(primary),
            });
        }
        let mut on_selection_keys = vec![PaletteKey::SelectionForeground];
        on_selection_keys.extend(Role::Neutral.candidates());
        on_selection_keys.extend([PaletteKey::Background, PaletteKey::DarkBackground]);
        let on_selection_candidates = self.collect(on_selection_keys);
        let selected = grounds.map(|g| selection.over(g));
        let on_selection = self.ink(
            Subject::Text(TextToken::OnSelection),
            None,
            &on_selection_candidates,
            false,
            &selected,
            t,
            "the selection over every surface",
        )?;
        let disabled_fill = base(Role::Neutral).muted[0];

        // 4 and 5. Each role's text and focus colours, and its states.
        let mut roles = Vec::new();
        for b in &bases {
            roles.push(self.role_colours(b, &grounds, disabled, disabled_fill)?);
        }
        let role = |r: Role| &roles[r as usize];

        // Borders.
        let border_subtle = self.tint(Subject::Border(BorderToken::Subtle), primary, BORDER_SUBTLE);
        let border_default = self.tint(
            Subject::Border(BorderToken::Default),
            primary,
            BORDER_DEFAULT,
        );
        let strong = self.edge(primary, &grounds, ui)?;
        let border_hover = self.edge_hover(primary, strong);
        let border = BorderColours {
            subtle: border_subtle,
            default: border_default,
            strong,
            hover: border_hover,
            focus: role(Role::Accent).focus,
            disabled: border_subtle,
            danger: role(Role::Danger).focus,
        };
        let on_accent = role(Role::Accent).solid.default.foreground;
        let same =
            |me: &mut Self, subject, colour: Color, rule| me.rule(subject, colour, colour, rule);
        use {BorderToken as B, TextToken as T};
        same(
            self,
            Subject::Border(B::Focus),
            border.focus,
            "the accent's focus colour",
        );
        same(
            self,
            Subject::Border(B::Disabled),
            border.disabled,
            "`border.subtle`",
        );
        same(
            self,
            Subject::Border(B::Danger),
            border.danger,
            "the danger role's focus colour",
        );
        same(self, Subject::Text(T::Inverse), canvas, "the canvas");
        same(
            self,
            Subject::Text(T::Placeholder),
            tertiary,
            "`text.tertiary`",
        );
        same(
            self,
            Subject::Text(T::OnAccent),
            on_accent,
            "the label of the accent's solid fill",
        );
        let text = TextColours {
            primary,
            secondary,
            tertiary,
            disabled,
            inverse: canvas,
            link,
            placeholder: tertiary,
            on_accent,
            on_selection,
        };
        let mut roles = roles.into_iter();
        let mut next = || roles.next().expect("one per role");
        let roles = Roles {
            accent: next(),
            neutral: next(),
            danger: next(),
            success: next(),
            warning: next(),
            info: next(),
        };
        Ok(SemanticColours {
            mode: p.mode,
            surface: SurfaceColours {
                canvas,
                recessed,
                sunken,
                raised,
                overlay,
                selection,
                hover,
                pressed,
                scrim,
            },
            text,
            border,
            roles,
        })
    }

    /// A palette colour as drawn: one with alpha is drawn over the canvas.
    fn opaque(&self, colour: Color) -> Color {
        if colour.a < 1.0 {
            colour.over(self.canvas)
        } else {
            colour
        }
    }

    /// The palette keys, in priority order, as candidates: keys the theme leaves out are
    /// skipped, and keys holding the same colour are one candidate.
    fn collect(&self, keys: impl IntoIterator<Item = PaletteKey>) -> Vec<Candidate> {
        let mut out: Vec<Candidate> = Vec::new();
        for key in keys {
            let Some(colour) = self.palette.get(key).map(|c| self.opaque(c)) else {
                continue;
            };
            match out.iter_mut().find(|c| c.colour.same_rgba8(colour)) {
                Some(same) if same.keys.contains(&key) => {}
                Some(same) => same.keys.push(key),
                None => out.push(Candidate {
                    keys: vec![key],
                    colour,
                    lch: colour.to_oklch(),
                }),
            }
        }
        out
    }

    /// A role's candidates: its override's keys first, then its defaults, or under the
    /// manual strategy the override's keys alone. The accent's defaults are `accent` and the
    /// palette's other colours of its hue family, nearest hue first.
    fn role_candidates(&mut self, role: Role) -> Vec<Candidate> {
        let mut defaults: Vec<PaletteKey> = role.candidates().to_vec();
        if role == Role::Accent {
            let accent = self.opaque(self.palette.accent).to_oklch();
            if accent.c > ACHROMATIC {
                let mut related: Vec<(f32, PaletteKey)> = CHROMATIC_KEYS
                    .into_iter()
                    .filter_map(|key| {
                        let lch = self.opaque(self.palette.get(key)?).to_oklch();
                        same_family(lch, accent).then(|| (hue_distance(lch.h, accent.h), key))
                    })
                    .collect();
                related.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
                defaults.extend(related.into_iter().map(|(_, key)| key));
            }
        }
        let Some(choice) = self.options.overrides.get(role) else {
            return self.collect(defaults);
        };
        let mut present = Vec::new();
        for key in choice.keys() {
            if self.palette.get(key).is_some() {
                present.push(key);
            } else {
                self.notes.push(Note::MissingOverrideKey { role, key });
            }
        }
        if self.options.strategy == Strategy::Manual && !present.is_empty() {
            return self.collect(present);
        }
        self.collect(present.into_iter().chain(defaults))
    }

    /// The solid fill of `role` and the tints made from it.
    fn role_base(&mut self, role: Role) -> Result<RoleBase, ResolveError> {
        let candidates = self.role_candidates(role);
        let labels = self.labels(role);
        let t = self.options.text_contrast;
        let judge = |c: &Candidate| fill_judge(c, &labels, t);
        let (decision, pick, chosen) = self.select(
            Subject::Fill(role),
            Some(role),
            &candidates,
            role != Role::Neutral,
            Requirement {
                min: t,
                against: "its label",
            },
            judge,
        )?;
        self.decisions.push(decision);
        let source = candidates[chosen].colour;
        // `fill_judge` gives every pick a label; a pick without one is an error, not a panic.
        let (_, label) = pick.label.ok_or(ResolveError::Unreachable {
            subject: Subject::Fill(role),
            required: t,
            against: "its label",
            best: pick.contrast,
        })?;
        let tint = |me: &mut Self, emphasis, state, step| {
            me.tint(Subject::Treatment(role, emphasis, state), source, step)
        };
        let muted = [
            tint(self, Emphasis::Muted, State::Default, MUTED_FILL),
            tint(self, Emphasis::Muted, State::Hover, MUTED_FILL + HOVER_STEP),
            tint(
                self,
                Emphasis::Muted,
                State::Pressed,
                MUTED_FILL + PRESSED_STEP,
            ),
        ];
        let subtle = [
            tint(self, Emphasis::Subtle, State::Hover, HOVER_STEP),
            tint(self, Emphasis::Subtle, State::Pressed, PRESSED_STEP),
        ];
        let muted_border = self.tint(Subject::MutedBorder(role), source, MUTED_BORDER);
        if pick.colour.contrast(self.canvas) < self.options.ui_contrast {
            self.notes.push(Note::FillNearCanvas {
                role,
                contrast: pick.colour.contrast(self.canvas),
            });
        }
        Ok(RoleBase {
            role,
            candidates,
            fill: pick.colour,
            label,
            source,
            muted,
            muted_border,
            subtle,
        })
    }

    /// The colours a solid fill's label may be: the theme's `selection_foreground` for the
    /// accent, `background` and `foreground`, then every other neutral the theme has. Every
    /// one is tried as written before the fill is changed ([`fill_judge`]).
    fn labels(&self, role: Role) -> Vec<Candidate> {
        use PaletteKey as K;
        let mut keys = Vec::new();
        if role == Role::Accent {
            keys.push(K::SelectionForeground);
        }
        keys.extend([
            K::Background,
            K::Foreground,
            K::BrightForeground,
            K::LightForeground,
            K::DarkForeground,
            K::DarkBackground,
            K::DarkerBackground,
            K::LighterBackground,
        ]);
        self.collect(keys)
    }

    /// A role's text, focus and state colours, given its fill and tints.
    fn role_colours(
        &mut self,
        b: &RoleBase,
        grounds: &[Color; 5],
        disabled_text: Color,
        disabled_fill: Color,
    ) -> Result<RoleColours, ResolveError> {
        let (t, ui) = (self.options.text_contrast, self.options.ui_contrast);
        let role = b.role;
        // Every place the role's text and focus indicator may be drawn: each surface, and
        // each muted and subtle fill over it.
        let mut role_grounds = grounds.to_vec();
        for g in grounds {
            role_grounds.extend(b.muted.iter().chain(&b.subtle).map(|tint| tint.over(*g)));
        }
        let family = role != Role::Neutral;
        let against = "every surface, and every muted and subtle fill of the role over it";
        let ink = self.ink(
            Subject::Ink(role),
            Some(role),
            &b.candidates,
            family,
            &role_grounds,
            t,
            against,
        )?;
        let focus = self.ink(
            Subject::Focus(role),
            Some(role),
            &b.candidates,
            family,
            &role_grounds,
            ui,
            against,
        )?;
        if focus.contrast(b.fill) < ui {
            self.notes.push(Note::FocusNearFill {
                role,
                contrast: focus.contrast(b.fill),
            });
        }

        let fill_hover = self.fill_state(role, State::Hover, b.fill, b.label, HOVER_STEP);
        let fill_pressed = self.fill_state(role, State::Pressed, b.fill, b.label, PRESSED_STEP);
        let solid_state = |fill: Color| Treatment {
            background: fill,
            foreground: b.label,
            border: fill,
        };
        let transparent = b.source.with_alpha(0.0);
        let disabled = |background: Color, border: Color| Treatment {
            background,
            foreground: disabled_text,
            border,
        };
        let muted_state = |background: Color| Treatment {
            background,
            foreground: ink,
            border: b.muted_border,
        };
        let subtle_state = |background: Color| Treatment {
            background,
            foreground: ink,
            border: transparent,
        };
        let neutral_border = disabled_fill;
        Ok(RoleColours {
            solid: EmphasisColours {
                default: solid_state(b.fill),
                hover: solid_state(fill_hover),
                pressed: solid_state(fill_pressed),
                disabled: disabled(disabled_fill, disabled_fill),
            },
            muted: EmphasisColours {
                default: muted_state(b.muted[0]),
                hover: muted_state(b.muted[1]),
                pressed: muted_state(b.muted[2]),
                disabled: disabled(disabled_fill, neutral_border),
            },
            subtle: EmphasisColours {
                default: subtle_state(transparent),
                hover: subtle_state(b.subtle[0]),
                pressed: subtle_state(b.subtle[1]),
                disabled: disabled(transparent, transparent),
            },
            focus,
        })
    }

    /// A solid fill under hover or press: its lightness moved `step` away from its label, so
    /// contrast only grows; toward the label only when there is no room, and only as far as
    /// contrast allows.
    fn fill_state(
        &mut self,
        role: Role,
        state: State,
        fill: Color,
        label: Color,
        step: f32,
    ) -> Color {
        let lch = fill.to_oklch();
        let away = if label.to_oklch().l <= lch.l {
            1.0
        } else {
            -1.0
        };
        let t = self.options.text_contrast;
        // The nearest lightness that looks `step` away, so a step near black is as visible as
        // one elsewhere.
        let moved = [away, -away].into_iter().find_map(|dir| {
            let c = search(lch, dir, &|c: Color| c.apparent_distance(fill) >= step)?;
            (c.contrast(label) >= t).then_some(c)
        });
        let colour = moved.unwrap_or_else(|| {
            self.notes.push(Note::StateUnchanged { role, state });
            fill
        });
        let subject = Subject::Treatment(role, Emphasis::Solid, state);
        self.rule(
            subject,
            fill,
            colour,
            "the solid fill's lightness moved away from its label",
        );
        colour
    }

    /// Chooses one of `candidates` for text (or, with `min` at the non-text level, for a focus
    /// indicator) against every colour in `grounds`.
    #[allow(clippy::too_many_arguments)]
    fn ink(
        &mut self,
        subject: Subject,
        role: Option<Role>,
        candidates: &[Candidate],
        family: bool,
        grounds: &[Color],
        min: f32,
        against: &'static str,
    ) -> Result<Color, ResolveError> {
        let judge = |c: &Candidate| ink_judge(c, grounds, min);
        let (decision, pick, _) = self.select(
            subject,
            role,
            candidates,
            family,
            Requirement { min, against },
            judge,
        )?;
        self.decisions.push(decision);
        Ok(pick.colour)
    }

    /// Judges every candidate, picks one by the strategy, and records why.
    fn select(
        &mut self,
        subject: Subject,
        role: Option<Role>,
        candidates: &[Candidate],
        family: bool,
        requirement: Requirement,
        judge: impl Fn(&Candidate) -> (f32, Option<Pick>),
    ) -> Result<(Decision, Pick, usize), ResolveError> {
        let anchor = candidates.first().ok_or(ResolveError::Unreachable {
            subject,
            required: requirement.min,
            against: requirement.against,
            best: 0.0,
        })?;
        let judged: Vec<Judged> = candidates
            .iter()
            .map(|c| {
                let (as_written, pick) = judge(c);
                Judged {
                    as_written,
                    passes: as_written >= requirement.min,
                    in_family: !family || self.monochrome || same_family(c.lch, anchor.lch),
                    pick,
                }
            })
            .collect();
        let strategy = self.options.strategy;
        let feasible = judged
            .iter()
            .enumerate()
            .filter_map(|(i, j)| Some((i, j, j.pick.as_ref()?)));
        let chosen = match strategy {
            Strategy::Adaptive => feasible.min_by(|(i, a, pa), (k, b, pb)| {
                (!a.in_family)
                    .cmp(&!b.in_family)
                    .then(pa.change.total_cmp(&pb.change))
                    .then(i.cmp(k))
            }),
            Strategy::Strict | Strategy::Manual => {
                feasible.min_by(|(i, a, _), (k, b, _)| (!a.passes).cmp(&!b.passes).then(i.cmp(k)))
            }
        };
        let Some((index, chosen, pick)) = chosen else {
            return Err(ResolveError::Unreachable {
                subject,
                required: requirement.min,
                against: requirement.against,
                best: judged.iter().map(|j| j.as_written).fold(0.0, f32::max),
            });
        };
        let pick = pick.clone();
        let reason = match (strategy, chosen.passes, chosen.in_family) {
            (Strategy::Adaptive, _, false) => Reason::OutsideFamily,
            (_, true, _) => Reason::Passes,
            (Strategy::Adaptive, false, true) => Reason::LeastChange,
            (_, false, _) => Reason::FirstAdjusted,
        };
        if strategy == Strategy::Manual
            && let Some(role) = role
            && let Some(choice) = self.options.overrides.get(role)
        {
            let preferred = &candidates[0];
            if preferred.keys.contains(&choice.preferred) && !(index == 0 && chosen.passes) {
                self.notes.push(Note::PreferredFails {
                    subject,
                    preferred: choice.preferred,
                    contrast: judged[0].as_written,
                    required: requirement.min,
                    used: candidates[index].keys.clone(),
                    adjusted: !chosen.passes,
                });
            }
        }
        let considered = candidates
            .iter()
            .zip(&judged)
            .map(|(c, j)| Considered {
                keys: c.keys.clone(),
                colour: c.colour,
                contrast: j.as_written,
                passes: j.passes,
                in_family: j.in_family,
                adjusted: j
                    .pick
                    .as_ref()
                    .filter(|_| !j.passes)
                    .map(|p| (p.colour, p.change)),
            })
            .collect();
        let decision = Decision {
            subject,
            requirement: Some(requirement),
            source: Source::Palette(candidates[index].keys.clone()),
            original: candidates[index].colour,
            resolved: pick.colour,
            contrast_before: Some(chosen.as_written),
            contrast_after: Some(pick.contrast),
            label: pick.label.clone(),
            reason,
            considered,
        };
        Ok((decision, pick, index))
    }

    /// `colour` made translucent: the least opacity, in 8-bit steps, at which it is drawn
    /// `step` from the canvas in OKLab. Opaque when even that is not far enough, with a note.
    fn tint(&mut self, subject: Subject, colour: Color, step: f32) -> Color {
        let tint = self.translucent(subject, colour, step);
        self.rule(
            subject,
            colour,
            tint,
            "translucent, a fixed perceptual step from the canvas",
        );
        tint
    }

    /// The tint of [`Resolver::tint`], without recording a decision.
    fn translucent(&mut self, subject: Subject, colour: Color, step: f32) -> Color {
        let canvas = self.canvas;
        let reached = (1..=u8::MAX)
            .map(|k| colour.with_alpha(f32::from(k) / 255.0))
            .find(|c| c.over(canvas).apparent_distance(canvas) >= step);
        reached.unwrap_or_else(|| {
            self.notes.push(Note::TintTooClose {
                subject,
                reached: colour.with_alpha(1.0).apparent_distance(canvas),
                step,
            });
            colour.with_alpha(1.0)
        })
    }

    /// A surface: the theme's colour, unless no text could be read on it ([`Carry::surface`]).
    fn surface(
        &mut self,
        token: SurfaceToken,
        key: PaletteKey,
        colour: Color,
        carry: Carry,
    ) -> Color {
        let written = self.opaque(colour);
        let resolved = carry.surface(written, self.canvas);
        self.decisions.push(Decision {
            subject: Subject::Surface(token),
            requirement: Some(Requirement {
                min: carry.min,
                against: "`foreground` drawn on it, also in a pressed muted fill",
            }),
            source: Source::Palette(vec![key]),
            original: colour,
            resolved,
            contrast_before: Some(carry.worst(carry.foreground, written)),
            contrast_after: Some(carry.worst(carry.foreground, resolved)),
            label: None,
            reason: Reason::Rule,
            considered: Vec::new(),
        });
        resolved
    }

    /// A palette colour used as the theme wrote it (drawn over the canvas if translucent).
    fn keep(&mut self, subject: Subject, key: PaletteKey, colour: Color) -> Color {
        let resolved = self.opaque(colour);
        self.decisions.push(Decision {
            subject,
            requirement: None,
            source: Source::Palette(vec![key]),
            original: colour,
            resolved,
            contrast_before: None,
            contrast_after: None,
            label: None,
            reason: Reason::Rule,
            considered: Vec::new(),
        });
        resolved
    }

    /// `text.secondary`: `text.primary` with its lightness halfway to `text.tertiary`'s, so the
    /// three are evenly spaced and secondary keeps primary's hue; moved further only if the
    /// midpoint were to fall short.
    fn secondary(
        &mut self,
        primary: Color,
        tertiary: Color,
        grounds: &[Color],
    ) -> Result<Color, ResolveError> {
        let (a, b) = (primary.to_oklch(), tertiary.to_oklch());
        let mid = with_lightness(a, (a.l + b.l) / 2.0);
        let t = self.options.text_contrast;
        let before = worst(mid, grounds);
        let resolved = if before >= t {
            mid
        } else {
            nearest(mid, |c| worst(c, grounds) >= t).ok_or(ResolveError::Unreachable {
                subject: Subject::Text(TextToken::Secondary),
                required: t,
                against: "every surface",
                best: before,
            })?
        };
        self.decisions.push(Decision {
            subject: Subject::Text(TextToken::Secondary),
            requirement: Some(Requirement {
                min: t,
                against: "every surface, and under hover and pressed",
            }),
            source: Source::Rule("`text.primary` with its lightness halfway to `text.tertiary`'s"),
            original: mid,
            resolved,
            contrast_before: Some(before),
            contrast_after: Some(worst(resolved, grounds)),
            label: None,
            reason: Reason::Rule,
            considered: Vec::new(),
        });
        Ok(resolved)
    }

    /// `text.disabled`: the theme's `muted`, moved in lightness only as far as it takes to lie
    /// between [`DISABLED_MIN`] and [`DISABLED_MAX`] against the canvas.
    fn disabled_text(&mut self) -> Color {
        let canvas = self.canvas;
        let muted = self.opaque(self.palette.muted);
        let fits = |c: Color| (DISABLED_MIN..=DISABLED_MAX).contains(&c.contrast(canvas));
        let resolved = if fits(muted) {
            muted
        } else {
            // Contrast against one colour runs continuously from 1 to at least 4.5 toward
            // black or white, so a lightness in the band always exists.
            nearest(muted, fits).unwrap_or(muted)
        };
        self.decisions.push(Decision {
            subject: Subject::Text(TextToken::Disabled),
            requirement: None,
            source: Source::Palette(vec![PaletteKey::Muted]),
            original: muted,
            resolved,
            contrast_before: Some(muted.contrast(canvas)),
            contrast_after: Some(resolved.contrast(canvas)),
            label: None,
            reason: Reason::Rule,
            considered: Vec::new(),
        });
        resolved
    }

    /// `border.strong`: `colour` at the least opacity at which it reaches `min` against every
    /// surface it is drawn over.
    fn edge(&mut self, colour: Color, grounds: &[Color], min: f32) -> Result<Color, ResolveError> {
        let subject = Subject::Border(BorderToken::Strong);
        let edge = (1..=u8::MAX)
            .map(|k| colour.with_alpha(f32::from(k) / 255.0))
            .find(|c| grounds.iter().all(|g| c.over(*g).contrast(*g) >= min))
            .ok_or(ResolveError::Unreachable {
                subject,
                required: min,
                against: "every surface",
                best: worst(colour, grounds),
            })?;
        self.decisions.push(Decision {
            subject,
            requirement: Some(Requirement {
                min,
                against: "every surface",
            }),
            source: Source::Rule("`text.primary` at the least opacity that reaches the minimum"),
            original: colour,
            resolved: edge,
            contrast_before: Some(worst(colour, grounds)),
            contrast_after: Some(
                grounds
                    .iter()
                    .map(|g| edge.over(*g).contrast(*g))
                    .fold(f32::INFINITY, f32::min),
            ),
            label: None,
            reason: Reason::Rule,
            considered: Vec::new(),
        });
        Ok(edge)
    }

    /// `border.hover`: `strong` made more opaque until it has moved a hover step.
    fn edge_hover(&mut self, colour: Color, strong: Color) -> Color {
        let from = strong.over(self.canvas);
        let hover = (strong.to_rgba8()[3]..=u8::MAX)
            .map(|k| colour.with_alpha(f32::from(k) / 255.0))
            .find(|c| c.over(self.canvas).apparent_distance(from) >= HOVER_STEP)
            .unwrap_or(colour.with_alpha(1.0));
        self.rule(
            Subject::Border(BorderToken::Hover),
            strong,
            hover,
            "`border.strong` made more opaque by a hover step",
        );
        hover
    }

    fn rule(&mut self, subject: Subject, original: Color, resolved: Color, rule: &'static str) {
        self.decisions.push(Decision {
            subject,
            requirement: None,
            source: Source::Rule(rule),
            original,
            resolved,
            contrast_before: None,
            contrast_after: None,
            label: None,
            reason: Reason::Rule,
            considered: Vec::new(),
        });
    }
}

/// The palette's colour keys, the ones the accent may borrow a relative from.
const CHROMATIC_KEYS: [PaletteKey; 14] = {
    use PaletteKey as K;
    [
        K::Red,
        K::Orange,
        K::Yellow,
        K::Green,
        K::Cyan,
        K::Blue,
        K::Magenta,
        K::Brown,
        K::BrightRed,
        K::BrightYellow,
        K::BrightGreen,
        K::BrightCyan,
        K::BrightBlue,
        K::BrightMagenta,
    ]
};

/// Judges a candidate as a solid fill: as written with the first label that reaches `min`,
/// else moved in lightness until one does, the least change winning.
fn fill_judge(c: &Candidate, labels: &[Candidate], min: f32) -> (f32, Option<Pick>) {
    // The theme's own choice for text on the accent comes first. Then whichever of
    // `background` and `foreground` contrasts more, then the other, then the other neutrals in
    // their order.
    let mut ordered: Vec<&Candidate> = labels.iter().collect();
    let fixed = usize::from(
        labels
            .first()
            .is_some_and(|l| l.keys.contains(&PaletteKey::SelectionForeground)),
    );
    let ends = ordered[fixed..]
        .iter()
        .take_while(|l| {
            l.keys.contains(&PaletteKey::Background) || l.keys.contains(&PaletteKey::Foreground)
        })
        .count();
    ordered[fixed..fixed + ends].sort_by(|a, b| {
        b.colour
            .contrast(c.colour)
            .total_cmp(&a.colour.contrast(c.colour))
    });
    if let Some(label) = ordered.iter().find(|l| l.colour.contrast(c.colour) >= min) {
        let contrast = label.colour.contrast(c.colour);
        let pick = Pick {
            colour: c.colour,
            contrast,
            change: 0.0,
            label: Some((label.keys.clone(), label.colour)),
        };
        return (contrast, Some(pick));
    }
    // Failing as written: the best any label reaches.
    let as_written = ordered
        .iter()
        .map(|l| l.colour.contrast(c.colour))
        .fold(0.0, f32::max);
    let pick = ordered
        .iter()
        .filter_map(|label| {
            let colour = nearest(c.colour, |x| x.contrast(label.colour) >= min)?;
            Some(Pick {
                colour,
                contrast: colour.contrast(label.colour),
                change: colour.apparent_distance(c.colour),
                label: Some((label.keys.clone(), label.colour)),
            })
        })
        .min_by(|a, b| a.change.total_cmp(&b.change));
    (as_written, pick)
}

/// Judges a candidate as text, or a mark, against every colour in `grounds`.
fn ink_judge(c: &Candidate, grounds: &[Color], min: f32) -> (f32, Option<Pick>) {
    let as_written = worst(c.colour, grounds);
    if as_written >= min {
        let pick = Pick {
            colour: c.colour,
            contrast: as_written,
            change: 0.0,
            label: None,
        };
        return (as_written, Some(pick));
    }
    let pick = nearest(c.colour, |x| worst(x, grounds) >= min).map(|colour| Pick {
        colour,
        contrast: worst(colour, grounds),
        change: colour.apparent_distance(c.colour),
        label: None,
    });
    (as_written, pick)
}

/// Whether two colours are of one hue family: both grey, or both coloured with hues within
/// [`FAMILY_HUE`].
fn same_family(a: Oklch, b: Oklch) -> bool {
    match (a.c <= ACHROMATIC, b.c <= ACHROMATIC) {
        (true, true) => true,
        (false, false) => hue_distance(a.h, b.h) <= FAMILY_HUE,
        _ => false,
    }
}

/// The least contrast of the opaque `colour` against any of `grounds`.
fn worst(colour: Color, grounds: &[Color]) -> f32 {
    grounds
        .iter()
        .map(|g| colour.contrast(*g))
        .fold(f32::INFINITY, f32::min)
}

/// `lch` at lightness `l`, brought into gamut by lowering chroma, and rounded to the bytes it
/// is drawn with.
fn with_lightness(lch: Oklch, l: f32) -> Color {
    Oklch { l, ..lch }.to_color_mapped().quantized()
}

/// The colour that looks nearest `source` and that `pass` accepts, found by changing only its
/// lightness: hue is kept, and chroma is lowered only where sRGB cannot show it. Both
/// directions are searched, one JND at a time and then by halving the last step; the nearer
/// result wins, the lighter on a tie. `None` when no lightness passes. Bounded: at most
/// `2 × (1 / JND + 1 + SEARCH_HALVINGS)` tries.
fn nearest(source: Color, pass: impl Fn(Color) -> bool) -> Option<Color> {
    let lch = source.to_oklch();
    let change = |c: &Color| c.apparent_distance(source);
    [1.0, -1.0]
        .into_iter()
        .filter_map(|direction| search(lch, direction, &pass))
        .min_by(|a, b| change(a).total_cmp(&change(b)))
}

fn search(lch: Oklch, direction: f32, pass: &impl Fn(Color) -> bool) -> Option<Color> {
    let end = if direction > 0.0 { 1.0 } else { 0.0 };
    let mut failing = lch.l;
    for step in 1..=(1.0 / JND).ceil() as u32 + 1 {
        let l = (lch.l + direction * JND * step as f32).clamp(0.0, 1.0);
        if pass(with_lightness(lch, l)) {
            let mut passing = l;
            for _ in 0..SEARCH_HALVINGS {
                let mid = (failing + passing) / 2.0;
                if pass(with_lightness(lch, mid)) {
                    passing = mid;
                } else {
                    failing = mid;
                }
            }
            return Some(with_lightness(lch, passing));
        }
        if l == end {
            return None;
        }
        failing = l;
    }
    None
}

/// Checks a finished resolution once more, independently of how it was made: every colour in
/// gamut, every text and treatment readable where it is drawn, every focus indicator and
/// essential border visible, and no new hue in a monochrome palette.
fn validate(c: &SemanticColours, report: &Report) -> Result<(), ResolveError> {
    let (t, ui) = (report.options.text_contrast, report.options.ui_contrast);
    let grounds = c.surface.grounds();
    let need = |subject: Subject, found: f32, required: f32, against: &'static str| {
        if found >= required {
            Ok(())
        } else {
            Err(ResolveError::Invalid {
                subject,
                against,
                found,
                required,
            })
        }
    };
    let worst_drawn = |fg: Color, bg: Color| {
        grounds
            .iter()
            .map(|g| drawn_contrast(fg, bg, *g))
            .fold(f32::INFINITY, f32::min)
    };
    let surfaces = "every surface";
    for (token, colour) in [
        (TextToken::Primary, c.text.primary),
        (TextToken::Secondary, c.text.secondary),
        (TextToken::Tertiary, c.text.tertiary),
        (TextToken::Placeholder, c.text.placeholder),
        (TextToken::Link, c.text.link),
    ] {
        for under in [
            c.surface.hover,
            c.surface.pressed,
            c.surface.canvas.with_alpha(0.0),
        ] {
            need(
                Subject::Text(token),
                worst_drawn(colour, under),
                t,
                "every surface, under hover and pressed",
            )?;
        }
    }
    let accent = &c.roles.accent.solid;
    for state in [&accent.default, &accent.hover, &accent.pressed] {
        need(
            Subject::Text(TextToken::OnAccent),
            c.text.on_accent.contrast(state.background),
            t,
            "the accent's solid fill",
        )?;
    }
    need(
        Subject::Text(TextToken::OnSelection),
        worst_drawn(c.text.on_selection, c.surface.selection),
        t,
        "the selection over every surface",
    )?;
    need(
        Subject::Text(TextToken::Inverse),
        c.text.inverse.contrast(c.text.primary),
        t,
        "a fill of `text.primary`",
    )?;
    let disabled = c.text.disabled.contrast(c.surface.canvas);
    if !(DISABLED_MIN - 1e-3..=DISABLED_MAX).contains(&disabled) {
        return Err(ResolveError::Invalid {
            subject: Subject::Text(TextToken::Disabled),
            against: "the canvas, at most 3:1",
            found: disabled,
            required: DISABLED_MIN,
        });
    }
    for role in Role::ALL {
        let colours = c.roles.get(role);
        let mut backgrounds = Vec::new();
        for emphasis in Emphasis::ALL {
            for state in [State::Default, State::Hover, State::Pressed] {
                let tr = colours.emphasis(emphasis).state(state);
                need(
                    Subject::Treatment(role, emphasis, state),
                    worst_drawn(tr.foreground, tr.background),
                    t,
                    surfaces,
                )?;
                if emphasis != Emphasis::Solid {
                    backgrounds.push(tr.background);
                }
            }
        }
        for bg in backgrounds {
            need(
                Subject::Focus(role),
                worst_drawn(colours.focus, bg),
                ui,
                "every surface, and every muted and subtle fill of the role over it",
            )?;
        }
    }
    for (token, colour) in [
        (BorderToken::Strong, c.border.strong),
        (BorderToken::Hover, c.border.hover),
        (BorderToken::Focus, c.border.focus),
        (BorderToken::Danger, c.border.danger),
    ] {
        let found = grounds
            .iter()
            .map(|g| colour.over(*g).contrast(*g))
            .fold(f32::INFINITY, f32::min);
        need(Subject::Border(token), found, ui, surfaces)?;
    }
    for (subject, colour) in every_colour(c) {
        if !colour.is_in_gamut() {
            return Err(ResolveError::OutOfGamut(subject));
        }
        if report.monochrome && colour.to_oklch().c > ACHROMATIC {
            return Err(ResolveError::NewHue(subject));
        }
    }
    Ok(())
}

/// Every colour of a resolution, with the subject it belongs to.
fn every_colour(c: &SemanticColours) -> Vec<(Subject, Color)> {
    let s = &c.surface;
    let mut out = vec![
        (Subject::Surface(SurfaceToken::Canvas), s.canvas),
        (Subject::Surface(SurfaceToken::Recessed), s.recessed),
        (Subject::Surface(SurfaceToken::Sunken), s.sunken),
        (Subject::Surface(SurfaceToken::Raised), s.raised),
        (Subject::Surface(SurfaceToken::Overlay), s.overlay),
        (Subject::Surface(SurfaceToken::Selection), s.selection),
        (Subject::Surface(SurfaceToken::Hover), s.hover),
        (Subject::Surface(SurfaceToken::Pressed), s.pressed),
        (Subject::Surface(SurfaceToken::Scrim), s.scrim),
    ];
    let t = &c.text;
    out.extend(
        [
            (TextToken::Primary, t.primary),
            (TextToken::Secondary, t.secondary),
            (TextToken::Tertiary, t.tertiary),
            (TextToken::Disabled, t.disabled),
            (TextToken::Inverse, t.inverse),
            (TextToken::Link, t.link),
            (TextToken::Placeholder, t.placeholder),
            (TextToken::OnAccent, t.on_accent),
            (TextToken::OnSelection, t.on_selection),
        ]
        .map(|(token, colour)| (Subject::Text(token), colour)),
    );
    let b = &c.border;
    out.extend(
        [
            (BorderToken::Subtle, b.subtle),
            (BorderToken::Default, b.default),
            (BorderToken::Strong, b.strong),
            (BorderToken::Hover, b.hover),
            (BorderToken::Focus, b.focus),
            (BorderToken::Disabled, b.disabled),
            (BorderToken::Danger, b.danger),
        ]
        .map(|(token, colour)| (Subject::Border(token), colour)),
    );
    for role in Role::ALL {
        let colours = c.roles.get(role);
        out.push((Subject::Focus(role), colours.focus));
        for emphasis in Emphasis::ALL {
            for state in State::ALL {
                let tr = colours.emphasis(emphasis).state(state);
                let subject = Subject::Treatment(role, emphasis, state);
                out.extend([tr.background, tr.foreground, tr.border].map(|c| (subject, c)));
            }
        }
    }
    out
}

/// A surface colour, for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceToken {
    Canvas,
    Recessed,
    Sunken,
    Raised,
    Overlay,
    Selection,
    Hover,
    Pressed,
    Scrim,
}

impl SurfaceToken {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Canvas => "canvas",
            Self::Recessed => "recessed",
            Self::Sunken => "sunken",
            Self::Raised => "raised",
            Self::Overlay => "overlay",
            Self::Selection => "selection",
            Self::Hover => "hover",
            Self::Pressed => "pressed",
            Self::Scrim => "scrim",
        }
    }
}

/// A text colour, for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextToken {
    Primary,
    Secondary,
    Tertiary,
    Disabled,
    Inverse,
    Link,
    Placeholder,
    OnAccent,
    OnSelection,
}

impl TextToken {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Tertiary => "tertiary",
            Self::Disabled => "disabled",
            Self::Inverse => "inverse",
            Self::Link => "link",
            Self::Placeholder => "placeholder",
            Self::OnAccent => "on_accent",
            Self::OnSelection => "on_selection",
        }
    }
}

/// A border colour, for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BorderToken {
    Subtle,
    Default,
    Strong,
    Hover,
    Focus,
    Disabled,
    Danger,
}

impl BorderToken {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Subtle => "subtle",
            Self::Default => "default",
            Self::Strong => "strong",
            Self::Hover => "hover",
            Self::Focus => "focus",
            Self::Disabled => "disabled",
            Self::Danger => "danger",
        }
    }
}

/// What a decision is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Subject {
    /// A role's solid fill (`tone.{role}.solid.bg`) and, with it, its label.
    Fill(Role),
    /// A role's text colour in its muted and subtle treatments.
    Ink(Role),
    /// A role's focus indicator.
    Focus(Role),
    /// One state of one treatment, where it is made by a rule: a tint or a moved fill.
    Treatment(Role, Emphasis, State),
    /// The border of a role's muted treatment.
    MutedBorder(Role),
    Surface(SurfaceToken),
    Text(TextToken),
    Border(BorderToken),
}

impl fmt::Display for Subject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fill(role) => write!(f, "{role} solid fill"),
            Self::Ink(role) => write!(f, "{role} text"),
            Self::Focus(role) => write!(f, "{role} focus"),
            Self::Treatment(role, emphasis, state) => {
                write!(f, "{role} {} {}", emphasis.name(), state.name())
            }
            Self::MutedBorder(role) => write!(f, "{role} muted border"),
            Self::Surface(token) => write!(f, "surface.{}", token.name()),
            Self::Text(token) => write!(f, "text.{}", token.name()),
            Self::Border(token) => write!(f, "border.{}", token.name()),
        }
    }
}

/// The least contrast a colour had to reach, and against what.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Requirement {
    pub min: f32,
    pub against: &'static str,
}

/// Where a resolved colour came from.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// A palette colour, under every key that holds it, possibly adjusted.
    Palette(Vec<PaletteKey>),
    /// A fixed rule applied to colours already chosen.
    Rule(&'static str),
}

/// Why a candidate was chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// The first candidate, in priority order, that passes as the theme wrote it (under the
    /// adaptive strategy: the first of the role's hue family).
    Passes,
    /// Adaptive: no candidate of the role's hue family passes as written; this one needs the
    /// least change.
    LeastChange,
    /// Strict and manual: no candidate passes as written, so the first is adjusted.
    FirstAdjusted,
    /// Adaptive: nothing of the role's hue family can pass, so the nearest candidate outside
    /// it is used.
    OutsideFamily,
    /// Made by a fixed rule, not chosen among candidates.
    Rule,
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Passes => "the first candidate that passes as the theme wrote it",
            Self::LeastChange => {
                "no candidate of the role's hue family passes as written; this one needs the least change"
            }
            Self::FirstAdjusted => {
                "no candidate passes as written, so the first in priority order is adjusted"
            }
            Self::OutsideFamily => {
                "nothing of the role's hue family can pass, so the nearest candidate outside it is used"
            }
            Self::Rule => "made by a fixed rule",
        })
    }
}

/// One candidate as the resolver judged it.
#[derive(Debug, Clone, PartialEq)]
pub struct Considered {
    pub keys: Vec<PaletteKey>,
    /// As the theme wrote it.
    pub colour: Color,
    /// Its least contrast as written (for a fill, with its best label).
    pub contrast: f32,
    pub passes: bool,
    /// Whether it is of the hue family of the role's first candidate.
    pub in_family: bool,
    /// When it does not pass: its nearest lightness that does, and how far that is in OKLab.
    pub adjusted: Option<(Color, f32)>,
}

/// One colour the resolver chose or made, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    pub subject: Subject,
    /// The least contrast it had to reach, when it had to reach one.
    pub requirement: Option<Requirement>,
    pub source: Source,
    /// The colour before any change: the palette colour, or what the rule started from.
    pub original: Color,
    pub resolved: Color,
    /// Its least contrast against what the requirement names, before and after.
    pub contrast_before: Option<f32>,
    pub contrast_after: Option<f32>,
    /// For a solid fill: the label drawn on it, under the palette keys that hold it.
    pub label: Option<(Vec<PaletteKey>, Color)>,
    pub reason: Reason,
    /// Every candidate, in priority order. Empty for a rule.
    pub considered: Vec<Considered>,
}

impl Decision {
    /// Whether the resolved colour differs from the palette colour it was chosen from.
    pub fn adjusted(&self) -> bool {
        matches!(self.source, Source::Palette(_)) && !self.original.same_rgba8(self.resolved)
    }

    pub fn original_lch(&self) -> Oklch {
        self.original.to_oklch()
    }

    pub fn resolved_lch(&self) -> Oklch {
        self.resolved.to_oklch()
    }
}

fn keys(keys: &[PaletteKey]) -> String {
    keys.iter()
        .map(|k| format!("`{}`", k.name()))
        .collect::<Vec<_>>()
        .join(" = ")
}

fn lch(c: Color) -> String {
    let o = c.to_oklch();
    format!("L {:.2} C {:.3} h {:.0}°", o.l, o.c, o.h)
}

impl fmt::Display for Decision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let from = match &self.source {
            Source::Palette(k) => keys(k),
            Source::Rule(rule) => (*rule).to_owned(),
        };
        write!(f, "{}: {from}", self.subject)?;
        if self.original.same_rgba8(self.resolved) {
            write!(f, " {} ({})", self.resolved.to_hex(), lch(self.resolved))?;
        } else {
            write!(
                f,
                " {} ({}) became {} ({})",
                self.original.to_hex(),
                lch(self.original),
                self.resolved.to_hex(),
                lch(self.resolved)
            )?;
        }
        if let (Some(r), Some(before), Some(after)) =
            (self.requirement, self.contrast_before, self.contrast_after)
        {
            write!(
                f,
                "; needs {}:1 against {}: {before:.2}:1 as written, {after:.2}:1 as resolved",
                r.min, r.against
            )?;
        }
        if let Some((k, c)) = &self.label {
            write!(f, "; label {} {}", keys(k), c.to_hex())?;
        }
        if self.reason != Reason::Rule {
            write!(f, "; chosen as {}", self.reason)?;
        }
        for c in self
            .considered
            .iter()
            .filter(|c| !matches!(&self.source, Source::Palette(k) if *k == c.keys))
        {
            write!(
                f,
                "\n  also {} {}: {:.2}:1",
                keys(&c.keys),
                c.colour.to_hex(),
                c.contrast
            )?;
            if c.passes {
                write!(f, ", passes as written")?;
            } else if let Some((colour, change)) = c.adjusted {
                write!(f, ", would become {} (ΔE {change:.3})", colour.to_hex())?;
            } else {
                write!(f, ", cannot pass")?;
            }
            if !c.in_family {
                write!(f, ", another hue family")?;
            }
        }
        Ok(())
    }
}

/// Something about a resolution worth knowing, though nothing failed.
#[derive(Debug, Clone, PartialEq)]
pub enum Note {
    /// An override names a palette key this theme leaves out; the role's other candidates are
    /// used.
    MissingOverrideKey { role: Role, key: PaletteKey },
    /// Manual strategy: the preferred key does not pass as written, so another key, or the
    /// preferred one adjusted, is used.
    PreferredFails {
        subject: Subject,
        preferred: PaletteKey,
        contrast: f32,
        required: f32,
        used: Vec<PaletteKey>,
        adjusted: bool,
    },
    /// A tint could not move its full step from the canvas even opaque: the source colour is
    /// too close to the canvas.
    TintTooClose {
        subject: Subject,
        reached: f32,
        step: f32,
    },
    /// A role's solid fill is below the non-text minimum against the canvas: its label, not
    /// its shape, marks the control.
    FillNearCanvas { role: Role, contrast: f32 },
    /// A role's focus colour is below the non-text minimum against its own solid fill: draw
    /// the indicator with a gap of the surface between them.
    FocusNearFill { role: Role, contrast: f32 },
    /// The link colour is below 3:1 against primary text, so the underline sets it apart.
    LinkNearText { contrast: f32 },
    /// A solid fill's hover or pressed state could not move without losing its label's
    /// contrast, so it equals the default.
    StateUnchanged { role: Role, state: State },
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingOverrideKey { role, key } => write!(
                f,
                "the override for {role} names `{}`, which this theme does not define; its other candidates are used",
                key.name()
            ),
            Self::PreferredFails {
                subject,
                preferred,
                contrast,
                required,
                used,
                adjusted,
            } => {
                write!(
                    f,
                    "{subject}: the preferred `{}` reaches {contrast:.2}:1, below {required}:1; ",
                    preferred.name()
                )?;
                if *adjusted {
                    write!(f, "{} is used with its lightness adjusted", keys(used))
                } else {
                    write!(f, "{} is used instead", keys(used))
                }
            }
            Self::TintTooClose {
                subject,
                reached,
                step,
            } => write!(
                f,
                "{subject}: its colour is only {reached:.3} from the canvas, less than the step of {step:.3}, so it is opaque"
            ),
            Self::FillNearCanvas { role, contrast } => write!(
                f,
                "the {role} solid fill is {contrast:.2}:1 against the canvas: its label, not its shape, marks the control"
            ),
            Self::FocusNearFill { role, contrast } => write!(
                f,
                "the {role} focus colour is {contrast:.2}:1 against its solid fill: draw it with a gap of the surface between them"
            ),
            Self::LinkNearText { contrast } => write!(
                f,
                "links are {contrast:.2}:1 against primary text: draw them underlined"
            ),
            Self::StateUnchanged { role, state } => write!(
                f,
                "the {role} solid {} state equals its default: no lightness keeps its label readable",
                state.name()
            ),
        }
    }
}

/// How a resolution was made: the options, what the palette is like, every decision and every
/// note. Kept in the [`crate::ThemeSnapshot`], so reading it never runs the resolver.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub options: ResolveOptions,
    pub mode: Mode,
    /// Whether every palette colour is within one JND of grey (`docs/spec/odk-semantic-tokens.md`,
    /// "Monochrome themes").
    pub monochrome: bool,
    /// The highest OKLCH chroma of any palette colour.
    pub max_chroma: f32,
    pub decisions: Vec<Decision>,
    pub notes: Vec<Note>,
}

impl Report {
    /// The decision about `subject`.
    pub fn decision(&self, subject: Subject) -> Option<&Decision> {
        self.decisions.iter().find(|d| d.subject == subject)
    }

    /// The decisions whose colour differs from the palette colour it came from.
    pub fn adjusted(&self) -> impl Iterator<Item = &Decision> {
        self.decisions.iter().filter(|d| d.adjusted())
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{} palette, {} strategy, text {}:1, controls {}:1{}",
            self.mode.name(),
            self.options.strategy.name(),
            self.options.text_contrast,
            self.options.ui_contrast,
            if self.monochrome { ", monochrome" } else { "" }
        )?;
        for decision in &self.decisions {
            writeln!(f, "{decision}")?;
        }
        for note in &self.notes {
            writeln!(f, "note: {note}")?;
        }
        Ok(())
    }
}

/// Why no semantic colours could be resolved. Nothing is published then.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ResolveError {
    #[error(transparent)]
    Options(#[from] OptionsError),
    #[error(
        "no colour for {subject} can reach {required}:1 against {against}; the best as the theme wrote it is {best:.2}:1"
    )]
    Unreachable {
        subject: Subject,
        required: f32,
        against: &'static str,
        best: f32,
    },
    #[error("{subject} is {found:.2}:1 against {against}, below the {required}:1 it needs")]
    Invalid {
        subject: Subject,
        against: &'static str,
        found: f32,
        required: f32,
    },
    #[error("{0} is not a colour sRGB can show")]
    OutOfGamut(Subject),
    #[error("{0} has a hue the monochrome palette does not")]
    NewHue(Subject),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Color {
        Color::parse_hex(s).unwrap()
    }

    #[test]
    fn nearest_moves_lightness_no_further_than_needed() {
        let black = Color::BLACK;
        let found = nearest(hex("#404040"), |c| c.contrast(black) >= 4.5).unwrap();
        let contrast = found.contrast(black);
        assert!((4.5..4.65).contains(&contrast), "{contrast}");
        assert!(found.to_oklch().c < 1e-3, "a grey stays grey");

        // A coloured source keeps its hue.
        let red = hex("#8a1c2c");
        let lifted = nearest(red, |c| c.contrast(hex("#1a1b26")) >= 4.5).unwrap();
        assert!(hue_distance(lifted.to_oklch().h, red.to_oklch().h) < 2.0);
        assert!(lifted.to_oklch().l > red.to_oklch().l);
    }

    #[test]
    fn nearest_takes_the_nearer_direction_and_gives_up_when_nothing_passes() {
        let grey = hex("#777777");
        // Both lighter and darker reach 1.5:1; darker needs less change from here.
        let found = nearest(grey, |c| c.contrast(grey) >= 1.5).unwrap();
        let (lighter, darker) = (
            search(grey.to_oklch(), 1.0, &|c: Color| c.contrast(grey) >= 1.5).unwrap(),
            search(grey.to_oklch(), -1.0, &|c: Color| c.contrast(grey) >= 1.5).unwrap(),
        );
        let nearer = if lighter.apparent_distance(grey) <= darker.apparent_distance(grey) {
            lighter
        } else {
            darker
        };
        assert_eq!(found, nearer);
        // Nothing is 6:1 against mid grey: neither white nor black.
        assert_eq!(nearest(grey, |c| c.contrast(grey) >= 6.0), None);
    }

    #[test]
    fn a_tint_is_the_least_opacity_that_reaches_its_step() {
        let palette = Palette::parse(
            "mode = \"dark\"\naccent = \"#7aa2f7\"\nmuted = \"#414868\"\nbackground = \"#1a1b26\"\ndark_background = \"#13141c\"\nlighter_background = \"#24283b\"\nforeground = \"#c0caf5\"\nred = \"#f7768e\"\nyellow = \"#e0af68\"\ngreen = \"#9ece6a\"\nblue = \"#7aa2f7\"\n",
        )
        .unwrap();
        let options = ResolveOptions::default();
        let mut r = Resolver::new(&palette, &options);
        let red = palette.red;
        let tint = r.translucent(Subject::Surface(SurfaceToken::Hover), red, MUTED_FILL);
        let canvas = palette.background;
        assert!(tint.over(canvas).apparent_distance(canvas) >= MUTED_FILL);
        let one_less = red.with_alpha(tint.a - 1.0 / 255.0);
        assert!(one_less.over(canvas).apparent_distance(canvas) < MUTED_FILL);
        assert!(r.notes.is_empty());
        // A colour that is the canvas cannot be a step from it.
        let same = r.translucent(Subject::Surface(SurfaceToken::Hover), canvas, MUTED_FILL);
        assert_eq!(same.a, 1.0);
        assert!(matches!(r.notes[0], Note::TintTooClose { .. }));
    }

    #[test]
    fn hue_families_are_judged_from_the_colours_not_their_names() {
        let lch = |s: &str| hex(s).to_oklch();
        // Tokyo Night's yellow and its orange bright_yellow are one family.
        assert!(same_family(lch("#e0af68"), lch("#ff9e64")));
        // Its cyan and blue are not.
        assert!(!same_family(lch("#449dab"), lch("#7aa2f7")));
        // Greys are one family, and never the family of a colour.
        assert!(same_family(lch("#a4a4a4"), lch("#cecece")));
        assert!(!same_family(lch("#a4a4a4"), lch("#f7768e")));
        // Hue wraps around: a red at 10° and a pink at 355° are one family.
        let (a, b) = (lch("#f7768e"), lch("#f070a0"));
        assert!(hue_distance(a.h, b.h) < FAMILY_HUE && same_family(a, b));
    }

    #[test]
    fn the_search_is_bounded() {
        let calls = std::cell::Cell::new(0u32);
        let never = |_: Color| {
            calls.set(calls.get() + 1);
            false
        };
        assert_eq!(nearest(hex("#808080"), never), None);
        let bound = 2 * ((1.0 / JND).ceil() as u32 + 1 + SEARCH_HALVINGS);
        assert!(calls.get() <= bound, "{} tries", calls.get());
    }
}
