# The Omarchy design language in ODK

The entry point to how ODK looks and why. It says what exists and where the rule is written; it does not restate the numbers, since a number written twice drifts (the radius did, once: ADR 0042). When this page and a spec or an ADR disagree, the ADR wins, then the spec. Last checked against the code on 2026-10-24 (Phase 12).

**Precise. Quiet. Responsive.** Precise: sizes sit on a 4 px grid, controls at one density share one height, a corner is 0 or 2 px and nothing is rounder, and no state moves anything. Quiet: colour is the theme's own, resolved to what can be read and no louder, motion is short and stops for the user who asks. Responsive: every control answers hover, press and focus at once, and a change of theme, density, corners or text size shows on the next frame without losing what the user was doing.

## Architecture

ADR 0040 (accepted 2026-10-22; its 0.8 px radius is 2 px, ADR 0042) gives four layers, each made from the one above and specified on its own.

```text
Omarchy colors.toml  ->  Source (Palette)  ->  Adaptive colour resolver  ->  Semantic tokens
                                                                                  |
                      Foundation tokens (geometry, density, type, spacing, motion) |
                                                  \                               /
                                                   Component tokens  ->  odk-ui components (GPUI)
```

| Layer | Crate and module | Spec | Reached through |
|---|---|---|---|
| Source | `odk_theme::Palette` | `docs/spec/odk-source-palette.md` | `odk::ui::theme_snapshot().palette`, only to show the palette |
| Semantic | `odk_theme::semantic`, `resolve` | `docs/spec/odk-semantic-tokens.md` | `theme_snapshot().semantic` |
| Foundation | `odk_theme::foundation` | `docs/spec/odk-foundation-tokens.md` | `odk::ui::foundation()` |
| Component | `odk_theme::component` | `docs/spec/odk-component-tokens.md` | `odk::ui::components()` |

`odk-theme` has no GPUI. `odk-ui` holds the current snapshot, its component tokens and the foundation in one process store (`crates/odk-ui/src/theme.rs`); a component reads `components()` and `foundation()` inside `render` and converts a colour with `colour()`. That store is the only theme state in `odk-ui`, and `checklist/source.rs` fails the build if another appears (checklist items 40 to 42).

What comes from Omarchy is narrow (`docs/spec/odk-theme-tokens.md`): the theme's `colors.toml`, the user's text size, Omarchy's font and the user's request for reduced motion, which is Hyprland's own animations setting (ADR 0048). Sizes, radius, borders and durations are ODK's own (ADR 0035, ADR 0043).

## Foundation

Specified in `docs/spec/odk-foundation-tokens.md`; the numbers live in `crates/odk-theme/src/foundation.rs` and are asserted by its tests.

- **Geometry**: corner radius `Square` (0) or `Subtle` (2 px), one setting for the whole application, so no rectangle is rounder than 2 px; parts that are round by nature (a switch, a radio, a spinner, a status dot) are shapes, not a larger radius. A border is 1 px; the focus ring is 2 px, drawn outside the control with a gap, so it takes no space.
- **Density**: `Compact`, `Default` and `Comfortable` give a control 28, 34 and 40 px, horizontal padding 10, 12 and 16 px and icons 14, 16 and 18 px, at Omarchy's default text size of 12 px (the type roles below are at that size too: body is 13 px on a 20 px line). The user's text size scales these (ADR 0043), so on a machine with larger text a "34 px" control is taller; the tests assert the numbers at the default size and the gallery's status bar shows what is in force.
- **Typography**: nine roles (Caption, Small, Body, Body strong, Button, Subtitle, Heading, Title, Mono) with a size, line height and weight each, in Omarchy's font.
- **Spacing**: a 4 px grid read through `Space`.
- **Motion**: `Instant` 0, `Fast` 140, `Normal` 180, `Slow` 240, `Enter` 240 and `Exit` 160 ms, one easing curve (`cubic-bezier(0.22, 1, 0.36, 1)`), and reduced motion that removes transitions and loops. A component never invents a duration.

## Colour

Specified in `docs/spec/odk-source-palette.md` and `docs/spec/odk-semantic-tokens.md`.

- **Source**: the palette exactly as the theme wrote it, with an error that names the key when one is malformed. Missing optional keys stay missing.
- **Adaptive resolver**: for each of six roles (accent, neutral, danger, success, warning, info) it collects candidates from the regular and bright colours, judges each on what it is (contrast, hue family, how far it would have to move) and never on its name, and picks deterministically. Where nothing reaches the contrast it moves a colour in lightness only. A monochrome palette stays monochrome: the resolver adds no hue. Maths is in OKLab and OKLCH, with CIE L* for perceptual steps near black, and gamut mapping by chroma.
- **Emphasis**: `Solid`, `Muted` and `Subtle` are a component's choice and independent of a colour's source intensity (regular or bright).
- **Contrast**: text 4.5:1, large text and essential non-text 3:1, focus rings 3:1 against every surface they sit on. Translucent colours are measured as composited over the surface under them, as GPUI draws them.
- **Overrides**: `odk::ui::set_colour_options` takes a `ResolveOptions` (strategy, preferred keys, stricter contrast). It can never switch contrast off.
- **Failure**: a theme that cannot be loaded or resolved never stops an application; the last good snapshot stays, and with none the emergency palette is used (ADR 0041).

## Components

`docs/spec/odk-ui-components.md` is the list, with what each one takes. All of them draw component tokens only (checklist item 34).

| Family | Components |
|---|---|
| Actions | `Button` (eight `ButtonVariant`s, any role at any emphasis, icon-only, loading), link as `ButtonVariant::Link` |
| Forms | `Input`, `SearchInput`, `PasswordInput`, `NumberInput`, `Textarea`, `InputGroup`, `Select`, `Command` (a palette: a field over a filtered list), `Checkbox`, `RadioGroup`, `Switch`, `FormField` |
| Navigation | `Tabs`, `ToggleGroup` (segmented control), `NavList`, `Breadcrumb` |
| Feedback | `Badge`, `StatusIndicator`, `Alert`, `Progress`, `Spinner`, `Skeleton`, `Separator`, `Text`, `Heading`, `Kbd` |
| Overlays | `DropdownMenu`, `ContextMenu`, `Popover`, `Tooltip`, `HoverCard`, `Dialog`, `Sheet`, `Toast` |

Not built, on purpose: a stand-alone `Combobox` and `Pagination` (`odk-ui-components.md`, "Left out of v1"), and everything that needs drag or virtualisation (sliders, resizable panels, tables), which waits for its own ADR.

## Interaction

Specified in `docs/spec/odk-interaction.md` (the contract, state precedence, the motion table), `docs/spec/odk-overlays.md` and, for what an application must do, `docs/accessibility.md`.

- **States** compose in one order, taken from `States` in the component layer: disabled suppresses everything, loading keeps the resting look and ignores a second activation, pressed beats hover, focus is independent of all of them, and invalid, selected and checked stay readable under focus and hover.
- **Keyboard**: every component works without a pointer; a group is one tab stop with arrows inside it; Enter and Space activate; Escape closes an overlay and gives focus back. Behaviour is tested on GPUI's test platform, not assumed from the web.
- **Accessibility**: ODK reports roles, names and states through `odk_a11y::Describe`. GPUI 0.2.2 has no accessibility tree, so **no screen reader can read an ODK window today**; what works and is tested is the keyboard, focus, contrast, motion and not relying on colour alone. `docs/accessibility.md` says the same.

## Theming

- **The active theme**: `odk::ui::run` loads Omarchy's current theme before the first window and follows it with a watcher; a change is resolved off the UI thread and published whole, so no reader sees half of one.
- **Custom themes and previews**: `odk::ui::preview_theme` shows any theme without touching Omarchy's; `end_theme_preview` returns to the followed one. The gallery's toolbar uses both.
- **Live updates**: density, corners, reduced motion, theme and text size change on the next frame and keep values, selection, focus, open overlays and toast deadlines (`checklist/forms.rs`, `overlays.rs`, `toast.rs`, `odk-theme/tests/switching.rs`).

## Developing

- Adding a component, token or test: `docs/contributing-components.md`.
- Looking at it: the gallery (`docs/gallery.md`), headlessly through `tools/headless-shot`; reference screenshots with `tools/visual-scenarios`.
- Moving an application from the earlier design system: `docs/migration.md`.
- Where it stands for release: `docs/release-readiness.md`.
