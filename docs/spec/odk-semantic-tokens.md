# ODK semantic colour tokens, version 1

Normative for the semantic layer of ADR 0040, accepted on 2026-10-22. It must not change a decision recorded in an ADR; where the two disagree, the ADR wins.

Built in `odk-theme` (`oklch.rs`, `color.rs`, `semantic.rs`, `resolve.rs`, `manager.rs`) and published through `odk-ui`'s store, on 2026-10-22. Components draw them through the component tokens (`docs/spec/odk-component-tokens.md`) from the change that moves each one (step 7 of `docs/plan.md`, "Design language migration"): `Button` since 2026-10-22, the form controls since 2026-10-23 (`docs/spec/odk-ui-components.md`, "Form controls"). Every component has moved since, and the legacy tokens are gone (ADR 0047). Values marked *provisional* are ODK's own choice and still need the user's confirmation.

## The resolver

The resolver turns a Palette (`docs/spec/odk-source-palette.md`) into colours by Role and Emphasis. It is one pure function, `odk_theme::semantic::resolve(&Palette, &ResolveOptions) -> Result<Resolution, ResolveError>`: no window, no file, no theme name, and the same inputs always give the same `Resolution { colours, report }`. It runs when a theme loads and when its options change, never while drawing.

It runs these stages in order:

1. **Surfaces.** Each surface is the theme's colour, unless no text could be read on it (see Surfaces).
2. **Fills.** For each Role, every candidate is judged as a solid fill under each of the theme's neutrals as its label. The strategy picks one. If none passes as written, the picked one's lightness moves until its best label passes.
3. **Tints.** Muted and subtle fills are the chosen palette colour made translucent, a fixed perceptual step from the canvas.
4. **Text.** Each text colour, and each Role's text and focus colour, is judged against every surface and every fill it may be drawn on, and chosen the same way.
5. **States.** Hover and pressed move a solid fill's lightness away from its label or make a tint a step stronger; disabled is neutral.
6. **Validation.** The whole result is checked once more, independently of how it was made. A result that fails is an error, never a colour.

Every choice is recorded in the `Report` (see Diagnostics).

## Roles

| `Role` | Meaning | Candidates, most preferred first |
|---|---|---|
| `Accent` | The theme's own colour: primary actions, selection, links | `accent`, then its relatives |
| `Neutral` | A control with no further meaning: secondary actions, chips | `foreground`, `bright_foreground`, `light_foreground` |
| `Danger` | Destructive actions, errors | `red`, `bright_red` |
| `Success` | Completed actions, valid input | `green`, `bright_green` |
| `Warning` | Something to look at before going on | `yellow`, `bright_yellow`, `orange` |
| `Info` | Information that wants attention | `cyan`, `bright_cyan`, `blue`, `bright_blue` |

- **The accent's relatives** are the palette's chromatic colours (`red` to `bright_magenta`) of the accent's hue family, nearest hue first. A grey accent has none.
- **Keys the theme leaves out** are skipped. Keys holding the same 8-bit colour are one candidate under all their names, so `red = bright_red` is judged once.
- **A Role's colour comes only from its candidates** (or, where it fails, a lightness of one of them). No colour is taken from outside the palette, and the palette is never changed.
- **Names are not promises.** Several stock themes remap their named slots on purpose: Lumon's `red` is blue, Lupine's `yellow` is blue, Matte Black's `yellow` is red. The resolver judges colours, not names, and keeps the theme's choice.

## Emphasis and treatments

The same source colour gives all three emphases; choosing the colour and building the emphasis are separate stages.

| `Emphasis` | Background | Foreground | Border |
|---|---|---|---|
| `Solid` | the fill, opaque | its label, a theme neutral | the fill |
| `Muted` | the source colour, translucent, 3 JND from the canvas | the Role's text colour | the source colour, translucent, 6 JND from the canvas |
| `Subtle` | none (fully transparent) | the Role's text colour | none |

Each emphasis has a `Treatment { background, foreground, border }` for each `State`:

| `State` | Solid | Muted | Subtle |
|---|---|---|---|
| `Default` | as above | as above | as above |
| `Hover` | fill lightness moved 2 JND away from the label | fill 3 + 2 JND from the canvas | fill 2 JND from the canvas |
| `Pressed` | fill lightness moved 4 JND away from the label | fill 3 + 4 JND from the canvas | fill 4 JND from the canvas |
| `Disabled` | the neutral muted fill, `text.disabled` | the same, `border.disabled` | no fill, `text.disabled` |

- **Hover** is a perceptible step, **pressed** twice that, and neither changes the hue: a fill moves in lightness only and a tint keeps its source's colour. Moving a solid fill away from its label can only raise contrast; it moves toward the label only where there is no room, and only if contrast holds. Otherwise the state equals the default and a note says so.
- **Disabled** carries no Role's colour, so it never looks selected or active, and its label stays readable (see Text).
- **Focus is not a state.** `RoleColours::focus` is drawn over any state, together with hover or press.
- **Loading** is a behaviour, not a colour: a loading control keeps its treatment.

`SemanticColours::treatment(role, emphasis, state)` and `roles.<role>.<emphasis>.<state>` give a treatment; components never need to know which palette key it came from.

## Tokens

`SemanticColours { mode, surface, text, border, roles }`.

| Group | Token | Value |
|---|---|---|
| `surface` | `canvas` | `background` |
| | `recessed` | `dark_background` |
| | `sunken` | `darker_background`, else `dark_background` moved one more canvas-to-recessed step in lightness |
| | `raised` | `lighter_background` |
| | `overlay` | `lighter_background`; overlays are set apart by their border |
| | `selection` | `selection_background`, else `selection`, else the accent, translucent, 6 JND from the canvas |
| | `hover` | the foreground, translucent, 2 JND from the canvas |
| | `pressed` | the foreground, translucent, 4 JND from the canvas |
| | `scrim` | `sunken` at 60% opacity, the legacy scrim's: dims the window behind a dialog or a sheet |
| `text` | `primary` | the first of the Neutral candidates that is readable everywhere text is drawn |
| | `secondary` | `primary` with its lightness halfway to `tertiary`'s |
| | `tertiary` | `muted`, its lightness moved until readable everywhere text is drawn |
| | `disabled` | `muted`, its lightness moved into 2:1 to 3:1 against the canvas (*provisional* floor of 2:1) |
| | `inverse` | the canvas: text on a fill of `text.primary` |
| | `link` | the Accent's candidates, readable everywhere text is drawn |
| | `placeholder` | `tertiary` |
| | `on_accent` | the label of the Accent's solid fill |
| | `on_selection` | `selection_foreground`, else a neutral, readable on the selection |
| `border` | `subtle` | `text.primary`, translucent, 3 JND from the canvas: separators, decoration |
| | `default` | `text.primary`, translucent, 6 JND from the canvas: cards, panels, overlays, decoration |
| | `strong` | `text.primary` at the least opacity that reaches the non-text minimum on every surface: the edge of a field or check box |
| | `hover` | `strong` made more opaque by 2 JND |
| | `focus` | the Accent's focus colour |
| | `disabled` | `subtle` |
| | `danger` | the Danger focus colour: the edge of a field whose value is wrong |
| `roles.<role>` | `solid`, `muted`, `subtle` | an `EmphasisColours` each: `default`, `hover`, `pressed`, `disabled` treatments |
| | `focus` | the Role's candidates, at the non-text minimum against every surface and every muted and subtle fill of the Role |

Surfaces are opaque except `hover`, `pressed`, `scrim` and a `selection` the theme does not define. Muted and subtle fills and most borders are translucent: one token serves every surface.

## Contrast

| Context | Minimum | Option |
|---|---:|---|
| Text, including every label whatever its size or density | 4.5:1 | `text_contrast` |
| Large text | 3:1 | `odk_theme::LARGE_TEXT_CONTRAST`; no token uses it, since ODK holds every label to 4.5:1 |
| Essential non-text: focus indicators, `border.strong`, `border.hover`, `border.danger` | 3:1 | `ui_contrast` |
| Disabled text | 2:1 to 3:1 against the canvas | fixed; below 3:1 so it never passes for active text or an active control |
| Decoration: `border.subtle`, `border.default`, muted borders | none | |

- **Ratios** are WCAG 2.2's, from relative luminance (`Color::luminance`, `Color::contrast`). Its cutoff of 0.03928 and the sRGB standard's 0.04045 agree on every 8-bit colour.
- **What is drawn is what is measured.** A translucent colour is composited over what lies beneath before its contrast is measured (`odk_theme::drawn_contrast(fg, bg, surface)`), the way GPUI blends: in gamma-encoded sRGB (`docs/checks.md`). Contrast is never measured against an abstract translucent colour.
- **Colours are rounded first.** Every adjusted colour is rounded to the 8-bit channels it is drawn with before it is checked.
- **Where text is drawn** is every surface (`canvas`, `recessed`, `sunken`, `raised`, `overlay`), and under `hover` and `pressed` on each. A Role's text and focus colour must also hold in every muted and subtle fill of that Role over every surface; text inside a Role's muted fill is that treatment's foreground. A solid label is checked against its fill in each state.
- **Final validation** checks every text colour, every default, hover and pressed treatment over every surface, every focus colour against every surface and fill it may sit on, every essential border, every colour in gamut, and, in a monochrome palette, every colour grey.

## Choosing a candidate

Each candidate is judged for one use (a fill, text or a focus indicator). It either passes as written, or it gets its nearest lightness that passes, with the change measured as an apparent distance (see Perceptual measures). Then the strategy ranks, lexicographically, so nothing can make up for failing contrast:

| `Strategy` | Ranking |
|---|---|
| `Adaptive` (default) | 1. can pass; 2. of the hue family of the Role's first candidate; 3. least change (as written is zero); 4. earlier candidate |
| `Strict` | 1. can pass; 2. passes as written; 3. earlier candidate. So: the first candidate that passes as written, else the first one adjusted |
| `Manual` | as `Strict`, but a Role with an override considers only the override's keys |

- **Hue family**: two colours whose OKLCH hues are within 30°, half the usual distance between neighbouring named hues (*provisional*), or two greys. A grey and a colour are never one family. Neutral has no family, and in a monochrome palette every candidate is in it.
- **What this means**: Adaptive keeps a Role in its own hue and adjusts it before borrowing a colour of another hue that passes as written; Strict prefers anything the theme wrote that passes. Neither picks a colour because it is called `bright`, because it is more saturated, or because it is darker.
- **A solid fill's label** is chosen from the theme's neutrals before the fill is changed: `selection_foreground` first for the Accent, then whichever of `background` and `foreground` contrasts more, then the other, then `bright_foreground`, `light_foreground`, `dark_foreground`, `dark_background`, `darker_background` and `lighter_background`. When none passes, the fill moves for whichever label needs the least change.
- **The reasons** recorded are `Passes`, `LeastChange`, `FirstAdjusted`, `OutsideFamily` and, for a colour a rule made, `Rule`.

## Perceptual measures

- **OKLCH** (Ottosson's OKLab, as CSS Color 4 defines it) separates lightness, chroma and hue, so the resolver can move a colour's lightness without changing its hue. Distances in it follow how different colours look far better than distances in RGB. `odk_theme::{Oklab, Oklch}`, `Color::to_oklch`, `Oklch::to_color`.
- **Apparent distance** (`Color::apparent_distance`) is OKLab's ΔE with lightness taken as CIE L*/100 (`Color::lightness`). OKLab's own lightness overstates differences near black, where a tint measured with it vanished on Vantablack's black canvas (`#010101` is 0.067 from black in OKLab and 0.003 in L*).
- **JND** is 0.02, roughly a just-noticeable difference (`odk_theme::JND`, CSS Color 4's gamut-mapping tolerance). Every perceptual step is a multiple of it: hover 2, pressed 4, muted fill 3, muted border and `border.default` 6, `border.subtle` 3, derived selection 6 (*provisional*).
- **Gamut mapping** keeps lightness and hue and lowers chroma, by bisection, to the most sRGB can show (`Oklch::to_color_mapped`). Channels are never clamped beyond rounding, and a lightness past white or black is white or black.
- **The lightness search** moves one JND at a time from the source, toward lighter and toward darker, then halves the last step twelve times. It takes the nearer result, the lighter on a tie. It is bounded at 2 × (51 + 12) tries and always returns a colour that passes, or nothing.
- **Tints** take the least 8-bit opacity at which the colour drawn over the canvas is the step away. When even opaque is not far enough, the tint is opaque and a note says so.

## Surfaces

The canvas and the foreground are the theme's anchors. Every other surface keeps the theme's colour, unless no text of the theme could be read on it: neither black nor white (whichever way the foreground lies) reaches text contrast on it and in a pressed muted fill of the foreground over it. Such a surface moves in lightness only as far as the theme's own foreground needs. Elsewhere, text adjusts and surfaces do not.

Four of the user's themes (generated by Aether) write `darker_background` as a mid grey near 4.3:1 against their dark foreground. Their `sunken` is lightened (`#7e786f` to `#989288` in `dazzle-dawn`); no stock theme's surface changes.

## Monochrome themes

A colour is grey when its OKLCH chroma is at most one JND (0.02), so a grey written a hair off neutral still counts. A palette is monochrome when every colour it defines is grey (`Report::monochrome`); Vantablack and White are. The resolver only ever uses palette colours and changes their lightness, so a monochrome palette's semantic colours are all grey, and validation refuses any that is not. Roles keep their identity in the tokens and the report even when Danger and Success are almost the same grey; components tell them apart with icons and labels. More generally, no theme gains a hue: every colour the resolver emits has the hue of a palette colour, or is grey, and is no more colourful than the palette's most colourful colour.

## Options

`ResolveOptions { text_contrast, ui_contrast, strategy, overrides }`, defaults 4.5, 3.0, `Adaptive` and none. `ResolveOptions::validate` refuses NaN, infinities, a `text_contrast` outside 4.5 to 21, a `ui_contrast` outside 3 to 21, and a `ui_contrast` above `text_contrast`. An option can make the resolver stricter than WCAG AA, never weaker.

**Overrides** (`Overrides`, `RoleOverride { preferred, fallback }`) are an application's own preferences: they put palette keys in front of a Role's candidates under every strategy, and under `Manual` they are the Role's only candidates. They never change what a colour must pass. A key the theme leaves out is a note, and the Role's other candidates are used. They can also be read from TOML text with `ResolveOptions::from_toml`, which reads no file:

```toml
[colors]
strategy = "manual"        # "adaptive", "strict" or "manual"
text_contrast = 7.0
ui_contrast = 3.0

[colors.warning]
preferred = "yellow"
fallback = "bright_yellow"   # or a list: ["bright_yellow", "orange"]
```

An unknown role, option or palette key, a missing `preferred`, or a value of the wrong type is an `OptionsError` naming it; other top-level tables are ignored. Where an application keeps such a file is not decided (`docs/plan.md`).

Under `Manual`, a preferred key that fails as written is never used as written. The fallback is used, or the preferred colour adjusted, and `Note::PreferredFails` says which, with the contrast it reached.

## Diagnostics

`Report { options, mode, monochrome, max_chroma, decisions, notes }` is kept in the snapshot, so asking never runs the resolver again.

- `Report::decision(Subject)` finds one decision; `Subject` names a fill, text or focus colour of a Role, a treatment state, a muted border, or a surface, text or border token.
- A `Decision` holds:
  - the requirement (minimum and against what), the source (palette keys or a rule), and the colour as written and as resolved, with OKLCH for each;
  - the contrast before and after, the label of a fill, and the reason;
  - every candidate considered, with its contrast as written, whether it passes, whether it is in the family, and the colour it would have become.
- `Display` writes a decision as a sentence, and a report as one line per decision and note. For example, why Tokyo Night's warning is its regular yellow:

  ```
  warning solid fill: `yellow` #e0af68 (L 0.78 C 0.106 h 75°); needs 4.5:1 against its label: 8.55:1 as written, 8.55:1 as resolved; label `background` #1a1b26; chosen as the first candidate that passes as the theme wrote it
    also `bright_yellow` #ff9e64: 8.40:1, passes as written
    also `orange` #eb927b: 7.27:1, passes as written, another hue family
  ```

- `Report::adjusted()` lists the colours changed for contrast.
- `Note` covers the rest:
  - an override key the theme lacks;
  - a preferred key that failed;
  - a tint that could not reach its step;
  - a solid fill below 3:1 against the canvas (its label marks the control);
  - a focus colour below 3:1 against its own solid fill (draw it with a gap);
  - links below 3:1 against text (underline them);
  - a state that could not move.
- `ThemeDiagnostics` adds the strategy, whether the palette is monochrome, and how many colours were adjusted. Its `Display` still writes no colour value, and nothing is logged per colour.

## Publishing

- **The snapshot.** `ThemeSnapshot` gains `semantic: Arc<SemanticColours>` and `resolution: Arc<Report>`, made from the same Palette in the same reload, so they are never published apart.
- **Component checks.** Every resolution is followed by the component layer's checks (`docs/spec/odk-component-tokens.md`, "Validation"), in the manager, `ThemeSnapshot::from_toml` and the emergency palette alike, so every snapshot's colours have passed both. A theme that fails them is a failed reload (`Error::Component`).
- **A theme that cannot be resolved.** A theme whose colours cannot be resolved with the options in use is a failed reload (`Error::Resolve`). The snapshot stays and the failure is kept for diagnostics, as ADR 0041 orders.
- **The emergency palette** always resolves with the default options.
- **Caching.** A reload that finds the same palette, with the same options, reuses the snapshot's resolution rather than resolving again; a change to `shell.toml` alone reloads without resolving.
- **Changing options.** `ThemeManager::set_options` resolves again from the Palette already read, without touching a file:
  - colours that differ publish one generation on;
  - options that cannot be met keep the snapshot and are kept as the failure;
  - invalid options are refused and change nothing.
- **In `odk-ui`.**
  - `odk::ui::set_colour_options(cx, options)` validates the options and hands them to the background task that owns the manager. It is kept apart from `configure`, so foundation tokens stay free of colour.
  - Requests that queue up are answered together. Only a snapshot that changed is published, and then every window redraws on the next frame.
  - `odk::ui::colour_options(cx)` returns what the application asked for.
  - `odk::ui::theme_snapshot().semantic` is what component tokens will read.
  - A change of density or reduced motion never resolves colours, and a change of colours never rebuilds the foundation.

## Rules for component tokens

The component layer (`docs/spec/odk-component-tokens.md`, built on 2026-10-22) follows these, and so does every component from the change that moves it onto component tokens (step 7 of `docs/plan.md`).

- Take colours from `SemanticColours`, by `Role`, `Emphasis` and `State`; never name a palette key, and never compute a colour, an OKLCH value or a contrast in `render`.
- Draw translucent tokens as they are: they were checked over every surface.
- A field's or check box's edge is `border.strong`; a wrong value's is `border.danger`. Cards, panels and overlays use `border.default`, separators `border.subtle`.
- Draw the focus indicator with `RoleColours::focus` (or `border.focus`) outside the control, with a gap of the surface between it and a solid fill. It is checked against surfaces and muted and subtle fills, not against the solid fill it surrounds.
- Underline links: in many themes colour alone does not set them apart from text.
- Text inside a muted fill is that treatment's foreground; body text stays on surfaces.
- Disabled controls use the `disabled` treatment, never a lowered opacity of another.

## Checked

On 2026-10-22, under the default options:

- **Stock themes.** All 22 stock themes in `/usr/share/omarchy/themes` (`omarchy` 4.0.6-1) resolve and pass the accessibility checks of `crates/odk-theme/tests/semantic.rs`.
- **Fixtures.** So do the fixtures (Tokyo Night, Nord, Catppuccin Mocha and Latte, Vantablack, the incomplete custom theme) and the emergency palette.
- **The user's themes.** All 23 of the user's own themes resolve, reported and not asserted.
- **No changes needed.** No stock surface changes, and the surfaces and text of every stock theme and fixture are byte-for-byte what they were before the resolver existed (the legacy tokens that were compared are gone, ADR 0047).
- **Stricter options.** Under 7:1 every fixture resolves. Tokyo Night, Catppuccin, Latte and Vantablack keep every surface; Nord's `raised` and `overlay` are darkened, since no text reaches 7:1 in a pressed fill on them.
- **Performance.** The whole of `tests/semantic.rs`, which resolves every stock theme and fixture several times over, runs in about 0.3 s in a debug build.

Not checked:

- how the steps and tints look on a screen beyond the buttons, which were seen headless at rest in three themes (hover and press, which the steps are for, cannot be seen without a pointer);
- other Omarchy releases.

## Open questions

1. Whether the focus indicator must also reach 3:1 against the solid fill it surrounds, which no single colour can do in most themes, or whether the gap above is the rule (plan finding 5).
2. Where an application's colour options live, if anywhere but its code.
3. The provisional values above: the step sizes, the hue family width and the 2:1 floor of disabled text.
