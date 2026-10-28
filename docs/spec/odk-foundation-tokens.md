# ODK foundation tokens, version 1

Normative for the foundation layer of ADR 0040, accepted on 2026-10-22. It must not change a decision recorded in an ADR; where the two disagree, the ADR wins.

The layer is built and tested. A component draws it from the change that moves it onto component tokens (`docs/plan.md`, "Design language migration", step 7 on); `Button`, the form controls, `Text`, `Heading` and the navigation and feedback components have moved. The others (the overlays and `Kbd`) still draw the sizes and durations of `docs/spec/odk-theme-tokens.md`, which ADR 0035 fixed before ADR 0040 amended it, and take from this layer only the monospace family (see Configuration).

## The four layers

ADR 0040 splits styling into four layers, each computed only from the one before:

1. **Source tokens:** the current Omarchy theme's colours, from `colors.toml` as a Palette (`docs/spec/odk-source-palette.md`), and from `shell.toml` where Omarchy wrote one (`docs/spec/odk-theme-tokens.md`).
2. **Foundation tokens:** ODK's own geometry, density, spacing, type and motion. This spec. No colour, nothing from the theme; the one thing that reaches it is the user's text size, which scales it (ADR 0043, "Text size" below).
3. **Semantic tokens:** colours by role (accent, neutral, danger, success, warning, info) and emphasis (solid, muted, subtle), made by an adaptive resolver (`docs/spec/odk-semantic-tokens.md`). Drawn through the component tokens.
4. **Component tokens:** the final style each component draws with, its colours named from the semantic layer and its geometry derived from this one (`docs/spec/odk-component-tokens.md`). Drawn by `Button`; the other components move onto them one at a time.

Omarchy supplies the colours; ODK defines their meaning, emphasis, geometry and behaviour; GPUI draws the result. The foundation layer lives in `odk_theme::foundation`, which has no GPUI dependency. `odk-ui` turns it into GPUI values at the rendering boundary.

## Tokens

Lengths are logical pixels, as `f32`. Every token is a field or method of `FoundationTokens`, which `odk::ui::foundation()` returns. A component reads it inside `render`, as `let f = odk::ui::foundation();`. Values marked *provisional* are ODK's own choice and still need the user's confirmation.

### Geometry (`f.geometry`)

| Token | Value | Purpose |
|---|---|---|
| `radius.none` | 0 | Always square. |
| `radius.default` | 2, or 0 with `CornerRadius::Square` | Controls: buttons, inputs, selects. |
| `radius.small` | 0 | Small marks, such as a key (*provisional* assignment). A check box is a control and takes `radius.default` (the form controls' brief, 2026-10-23); so does a badge (the brief for navigation and feedback, 2026-10-23, asked for the configured radius). |
| `radius.overlay` | 2, or 0 with `CornerRadius::Square` | Surfaces that float: menus, popovers, tooltips, dialogs. |
| `border_width` | 1 | Every border. |
| `focus_width` | 2 | The keyboard focus indicator. |
| `focus_offset` | 2 (*provisional*) | The gap of the surface between a control and its focus indicator, which is drawn outside it, so a solid fill never meets the indicator (`docs/spec/odk-semantic-tokens.md`, "Rules for component tokens"). |
| `thumb_gap` | 2 (*provisional*) | Between a control's inner edge and a part nested inside it: a switch's thumb in its track, a radio's dot in its circle, a button inside a field (a field action). |
| `target_min` | 24 | The smallest pointer target (WCAG 2.5.8). It does not scale with the text size. |
| `corner_radius` | `Subtle` | The application's choice that `radius.default` and `radius.overlay` follow. |

`CornerRadius::Square` is 0 px and `CornerRadius::Subtle` is 2 px, exactly (ADR 0042, which replaced ADR 0040's 0.8 px because a 0.8 px corner does not show at scale 1). No token is larger than 2 px, and no component may introduce a larger radius without the user's approval. The radius is set for the whole application; a component cannot choose its own.

The user gave that approval for one kind of part on 2026-10-22, in the brief for the component layer: a part whose roundness says what it is, a switch's track and thumb and a spinner, is a pill or a circle. Later that day the user added the radio, once v1 has one, and on 2026-10-23 the brief for navigation and feedback the status indicator's dot ("intrinsically circular indicators may retain circular geometry"). It is modelled as a shape (`odk_theme::component::Shape`), not as a radius, so no rectangle gains a larger one (`docs/spec/odk-component-tokens.md`, "Geometry").

GPUI 0.2.2 draws a radius as given: it scales radii by the display's scale factor without rounding and clamps them only to half the quad's size (`docs/checks.md`). At scale 1, a 2 px corner takes about 40% of the fill out of its corner pixel, enough to read as a corner while the control still reads as square-cut; 0.8 px took about 2% and could not be seen (`docs/checks.md`).

### Density (`f.controls`)

| `Density` | Height | Horizontal padding | Icon size | Gap |
|---|---:|---:|---:|---:|
| `Compact` | 28 | 10 | 14 | 6 (*provisional*) |
| `Default` | 34 | 12 | 16 | 8 (*provisional*) |
| `Comfortable` | 40 | 16 | 18 | 8 (*provisional*) |

`ControlMetrics { height, padding_x, icon_size, gap }` holds one row; `gap` is the space between an icon and a label inside a control, following shadcn/ui's buttons. `f.controls.metrics()` gives the application's density, `f.controls.resolve(own)` gives a control's own density when it has one (`Option<Density>`) and the application's otherwise, and `f.controls.for_density(d)` gives any row. The default density is `Default`.

Density changes control dimensions only: never a colour, a radius, the spacing scale or a type role.

A control's `Size` is a step from the density it would otherwise take (*provisional*, `docs/spec/odk-component-tokens.md`): `Md` is that density, `Sm` one step tighter (`Density::smaller`) and `Lg` one step roomier (`Density::larger`), stopping at compact and comfortable, so no control takes a height this table does not have. A density of the control's own wins over its size.

### Spacing (`f.spacing`)

A 4 px grid. Field `sN` is the spec's `space.N`.

| Token | Value |
|---|---:|
| `s0` | 0 |
| `s1` | 4 |
| `s2` | 8 |
| `s3` | 12 |
| `s4` | 16 |
| `s5` | 20 |
| `s6` | 24 |
| `s8` | 32 |
| `s12` | 48 |

### Layout (`f.layout`)

Spacing by purpose. A component asks for a purpose, not a step, when one fits.

| Token | Value | From |
|---|---:|---|
| `panel_padding` | 16 | `s4` |
| `dialog_padding` | 24 | `s6` |
| `section_gap` | 24 | `s6` |
| `form_gap` | 16 | `s4` |
| `field_gap` | 6 | its own value: an optical exception to the grid, between a field's label and its control, and between the other parts of a form field (description, message) |
| `menu_item_gap` | 8 | `s2` |
| `sidebar_width` | 220 | its own value: the width of an application's sidebar (`sidebar_frame`) |
| `content_width` | 1000 | its own value: the widest a page's column grows (`content_column`); a wider window centres it |

### Text size (`f.text_scale`)

The sizes in this spec are those at Omarchy's default text size, 12 px. `FoundationTokens::from_config` builds them so; `odk::ui::foundation()` is built with `FoundationTokens::scaled` at the user's own, a `TextScale` that is `[font] base-size` over 12 (ADR 0043). It multiplies, and rounds each result to a whole pixel on its own:

- every type size and line height (`f.typography.style`);
- every control metric: height, horizontal padding, icon size and gap, at every density (`f.controls`);
- every spacing step and the layout tokens built from them (`f.spacing`, `f.layout`).

It never changes a border width, the focus ring and its offset, the thumb gap, the 24 px target minimum, a corner radius or a duration, nor a weight or a family. The scale is `base-size / 12`, with `base-size` from the user's `~/.config/omarchy/shell.toml` over the theme's `shell.toml` (`docs/spec/odk-source-palette.md`, "Text size"), kept to 9 to 20 px and 12 when neither says. 20 px is 167% of the 12 px default; that ceiling is unchanged (`docs/accessibility.md`, "Text size"). Component tokens are derived from the foundation, so they follow it; `odk::ui::foundation()` is built again, from the application's configuration, when the scale changes and not otherwise.

### Typography (`f.typography`)

`f.typography.style(role)` returns a `TypeStyle { family, size, line_height, weight, letter_spacing }`.

| `TypeRole` | Size | Line height | Weight | Family |
|---|---:|---:|---|---|
| `Caption` | 11 | 16 | Regular | UI |
| `Small` | 12 | 18 | Regular | UI |
| `Body` | 13 | 20 | Regular | UI |
| `BodyStrong` | 13 | 20 | Medium | UI |
| `Button` | 13 | 18 | Medium | UI |
| `Subtitle` | 15 | 22 | Medium | UI |
| `Heading` | 18 | 26 | Semibold | UI |
| `Title` | 22 | 30 | Semibold | UI |
| `Mono` | 12 | 18 | Regular | Mono |

Regular, Medium and Semibold are weights 400, 500 and 600. Letter spacing is 0 for every role (*provisional*); GPUI 0.2.2's text style has no letter spacing, so a non-zero value could not be drawn (`docs/checks.md`). Type roles do not change with density.

The family of a role is `FontFamily::Ui` or `FontFamily::Mono`; `f.typography.family(family)` is the application's choice for it, or `None`. With `None`, both families are Omarchy's face (fontconfig's `monospace` alias, which `omarchy font set` chooses): Omarchy has no UI font setting, and its shell, terminals and launchers are all set in that one (ADR 0043). An application that wants GPUI's own face names `.SystemUIFont`, which GPUI 0.2.2 maps to IBM Plex Sans on Linux. The alias is resolved at first use and again whenever the user's `fonts.conf` changes, by `fc-match` run afresh (a fontconfig loaded into a process keeps the configuration it started with, `docs/checks.md`) and the font file it names opened for its family name; every window is drawn again when the family differs. A font installed after the application started is not known to GPUI's font lookup, and naming it would set all text in GPUI's fallback face, so the family is adopted only if GPUI can resolve it; otherwise the old font stays until the next start, and an `info` log line says so (`omarchy install font` followed by a running application is that case).

### Motion (`f.motion`)

| Token | `MotionDuration` | Value |
|---|---|---:|
| `instant` | `Instant` | 0 ms |
| `fast` | `Fast` | 140 ms |
| `normal` | `Normal` | 180 ms |
| `slow` | `Slow` | 240 ms |
| `enter` | `Enter` | 240 ms |
| `exit` | `Exit` | 160 ms |
| `cycle` | (a loop, not a transition) | 800 ms (*provisional*) |
| `hover_open`, `hover_close`, `toast` | (delays, not transitions) | 500 ms, 300 ms, 5 s (*provisional*) |

`easing` is `MotionEasing::Standard`, `cubic-bezier(0.22, 1, 0.36, 1)`; `MotionEasing::Linear` is the other choice. GPUI has no CSS transitions and no Bézier curves, so `MotionEasing::apply(t)` evaluates the curve itself (Newton's method, then bisection), and the same `t` always gives the same result. It is a plain `fn(f32) -> f32`, the form GPUI's `Animation::with_easing` takes.

`f.motion.duration(d)` is the token's value. `f.motion.transition(d)` is the length of a transition: the token's value, or zero when `reduced` is set. A `Button`'s spinner fades in and out over `normal` on the standard curve, a tab's indicator between tabs and a switch's thumb over `normal`, a menu's list in over `normal` and out over `exit`, and a progress bar's value over `normal`. GPUI 0.2.2 has no style transitions, so hover and press change colour at once (`docs/spec/odk-component-tokens.md`, "In GPUI").

`f.motion.cycle` is one turn of something that loops while work goes on: a `Spinner` turns once per cycle. It is not a `MotionDuration`, since a loop is not a transition, and nothing loops when `reduced` is set. The user moved it here from the legacy `Tokens.motion.cycle` on 2026-10-23, keeping its 800 ms, so the spinner could move onto component tokens (*provisional* value). The legacy `Tokens.motion.cycle` went with the legacy tokens (ADR 0047).

`f.motion.hover_open` (500 ms), `f.motion.hover_close` (300 ms) and `f.motion.toast` (5 s) are delays rather than transitions: how long the pointer rests on a tooltip's trigger before it opens, how long after it leaves before the tooltip or hover card closes, and how long a toast stays unless it says otherwise. They are not `MotionDuration`s, and reduced motion does not shorten them (waiting is not motion). They moved here from the legacy `Tokens.motion` with their values on 2026-10-23, so that `Tooltip`, `HoverCard` and `Toast` could move onto component tokens (the overlays' brief); like `cycle`, the values are *provisional* and the move needs the user's confirmation.

**Reduced motion.** When `f.motion.reduced` is set:

- Every non-essential transition is instant: a component shows its end state at once.
- Every state change still happens; only its animation goes.
- An indicator of slow work, such as a spinner, stays visible and stands still, so the work still reads as busy.
- Nothing waits for an animation before it takes a key.

## Configuration

An application changes its settings with `odk::ui::configure`. Whatever the closure leaves alone keeps its value, and every window redraws on the next frame:

```rust
use odk::theme::foundation::{CornerRadius, Density};

odk::ui::configure(cx, |c| {
    c.density = Density::Compact;
    c.corner_radius = CornerRadius::Square;
    c.fonts.mono = Some("JetBrains Mono".into());
});
```

| `FoundationConfig` field | Default | Effect |
|---|---|---|
| `density` | `Density::Default` | The density every control without its own uses. |
| `corner_radius` | `CornerRadius::Subtle` | `radius.default` and `radius.overlay`, for the whole application. |
| `reduced_motion` | `false` | The application's own choice. `odk::ui::set_reduced_motion(cx, on)` sets only this. |
| `fonts.ui` | `None` | The UI family, or `None` for Omarchy's font, the same as monospace (ADR 0043). `Some(".SystemUIFont")` keeps GPUI's own face. |
| `fonts.mono` | `None` | The monospace family, or `None` for Omarchy's. |

`odk::ui::foundation_config(cx)` returns the application's settings, which stay separate from the tokens built from them.

- **Reduced motion** is on when the application asks for it or the user does (ADR 0048). The user asks when the compositor's animations are off (Hyprland's `animations:enabled`, asked over its socket and followed on `configreloaded`). When the compositor does not answer, motion is not reduced. The `[motion] reduced` key in the user's own `~/.config/omarchy/shell.toml` is ignored. A sandboxed application cannot ask Hyprland; `odkd` asks, and tells it as it tells any application (ADR 0062). Either the application or the user reduces motion; an application's `false` never overrides the user's `true`, and the user's absence never turns the application's off. Precedence is therefore "either", with no ordering to get wrong. `odk::ui::theme_snapshot().reduced_motion` says whether the user asked, and the theme diagnostics say what the compositor said.
- **Density** can be overridden by one control: a component with its own `Option<Density>` reads `f.controls.resolve(own)`. Corner radius cannot be overridden locally.
- **Fonts.** Today the monospace choice reaches every existing component that sets text in monospace: `Kbd`, `Spinner`, the mark of a `Checkbox`, the marks in `Alert`, `Badge`, `Toast` and menus, and the icons of buttons, tabs, segments and sidebar rows. The UI choice, or Omarchy's font when there is none, reaches text styled with `text_role`; existing components follow it as they migrate.

## In GPUI

- `odk::ui::foundation()` returns an `Arc<FoundationTokens>` snapshot. It is built once per change of configuration, not per frame, and kept in the store beside the theme snapshot. Every change goes through one path, so a theme reload keeps the application's choice of reduced motion.
- Lengths go into GPUI with `px(...)`; durations are `std::time::Duration`, which GPUI takes as they are.
- `StyledExt::text_role(role)` sets a role's size, line height and weight, and its family when the role or the application names one.

```rust
let f = odk::ui::foundation();
let m = f.controls.resolve(self.density);
div()
    .h(px(m.height))
    .px(px(m.padding_x))
    .rounded(px(f.geometry.radius.default))
    .border(px(f.geometry.border_width))
    .text_role(TypeRole::Button)
```

## Rules for components

These bind a component from the change that moves it onto foundation tokens; `docs/spec/component-checklist.md` holds them as item 34, which applies only to components that have moved, so those that have not are not all known gaps at once.

- Do not hardcode geometry when a foundation token exists for it.
- Do not define a component's own spacing scale; use `f.spacing` and `f.layout`.
- Take control dimensions from the central density system, through `f.controls`, never from per-component numbers.
- Respect the configured corner radius: `radius.default` or `radius.overlay`, or `radius.none` or `radius.small` where a square is meant. Never a larger radius; a round part is a `Shape`.
- Keep foundation tokens free of colour, and never make a colour depend on density or corner radius.
- Use only the motion durations above and `transition()`, which honours reduced motion. No component has a duration of its own.
- Ask for a type role with `text_role`, not for a font size.
