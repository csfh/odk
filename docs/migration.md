# Migrating an application to the token design language

For applications written against ODK before ADR 0040 to 0047 (the flat `Tokens`, the shared `Variant`, a radius of 0). ODK is at version 0.0.0 and has no release yet, so these are breaking changes made on purpose, once, and listed here (ADR 0047). The code under "After" is compiled by `crates/odk/tests/migration.rs`; a section whose code stops compiling fails that test before this page is wrong. "Before" code does not compile any more, and is shown as text.

## What happens without a code change

Everything an application did not set itself. A theme that the user switches to is followed live; every component draws the new component layer; corners are 2 px (or 0 on request); controls take the density's height; text and headings take the type roles (at the default text size, body text is 13 px with a 20 px line, headings 22, 18 and 15 px); motion uses the shared durations and curve and stops under reduced motion; focus rings are 2 px outside the control. If an application used only `Button`, `Text`, `Heading`, the form controls and the overlays through their builders, it compiles as it was, apart from the table below.

## What needs a change

| Before | After |
|---|---|
| `odk::ui::tokens()`, `odk::theme::Tokens` | `odk::ui::theme_snapshot()` for colours, `odk::ui::foundation()` for sizes and durations, `odk::ui::components()` for a component's own style |
| `Variant::Primary`, `Secondary`, `Ghost` on a button | `ButtonVariant::Primary`, `Secondary`, `Ghost` (and `Outline`, `Destructive`, `Success`, `Warning`, `Link`) |
| `Badge::variant(Variant::X)` | `Badge::role(Role::Accent).emphasis(Emphasis::Solid)`, `Badge::tone(Tone::Danger)`, or `Badge::appearance(BadgeAppearance::new(..))` |
| Reading `tokens().accent`, `.surface`, `.radius`, `.space` | A component reads its own style from `components()`; an application's own drawing reads `theme_snapshot().semantic` (`surface`, `text`, `border`, `roles`) and `foundation()` |
| A `[controls]` colour in a theme's `shell.toml` | Nothing: ODK reads only `[font] base-size` of a theme's `shell.toml` and no colour from it (ADR 0047, accepted) |
| Control `Size` as a font and padding step | `Size::Sm`, `Md`, `Lg` is one density step tighter, the density, one roomier (*provisional*, `docs/spec/odk-component-tokens.md`); or name a density with `.density(..)` |

`docs/spec/odk-theme-tokens.md` has the full table of removed names.

## Starting

`odk::ui::run` loads the theme before the first window, so a component may read the tokens in `render` from the first frame. Calling `theme_snapshot()` or `components()` before `run` is a programming error and panics with a message that says so; `foundation()` returns the defaults.

```rust,ignore
odk::ui::run(|cx| {
    odk::ui::open_window(cx, WindowOptions::default(), |_, cx| cx.new(|_| MyApp))
        .unwrap();
});
```

Open windows with `odk::ui::open_window`, not GPUI's: it draws the `Shell` that hosts overlays, toasts and the text size.

## Density, corners and motion

One setting each, for the whole application, changed at any time; every window redraws on the next frame and keeps its values, focus and open overlays.

```rust,ignore
odk::ui::configure(cx, |c| {
    c.density = Density::Compact;           // 28 px controls at the default text size
    c.corner_radius = CornerRadius::Square; // 0 px; Subtle (the default) is 2 px
});
odk::ui::set_reduced_motion(cx, true);      // adds to the user's own request, never removes it
```

`CornerRadius` is `odk::theme::foundation::CornerRadius`. A control may take another density than the application's with `.density(..)`.

## Following and previewing themes

By default the application follows Omarchy's current theme. To show another theme without choosing it (a theme picker, a design tool), preview it; Omarchy's own files are not touched, and a change the system makes meanwhile is kept for when the preview ends.

```rust,ignore
let mut manager = ThemeManager::with_users_files(ThemeSource::named("nord")?);
manager.reload();
odk::ui::preview_theme(cx, manager.snapshot());
// ...
odk::ui::end_theme_preview(cx);
```

Tune how colours are resolved (strategy, preferred source colours, stricter contrast) with `odk::ui::set_colour_options(cx, ResolveOptions { .. })`. It never lets contrast go below 4.5:1 for text or 3:1 for indicators. `odk::ui::theme_diagnostics(cx)` says what the theme manager knows, and the gallery's Colour resolver page says why each colour was chosen.

## Buttons

Eight variants, any role at any emphasis, an icon-only button with a required name, a loading state that keeps the size.

```rust,ignore
// Before
Button::new("save").label("Save").variant(Variant::Primary)
// After
Button::new("save").label("Save").variant(ButtonVariant::Primary)
Button::new("delete").label("Delete").variant(ButtonVariant::Destructive).emphasis(Emphasis::Muted)
Button::icon("settings", "⚙", "Settings").variant(ButtonVariant::Ghost)
```

A disabled or loading button ignores a second activation; `on_click` is called once per completed press, from the pointer or from Enter and Space.

## Form controls

`Input`, `Textarea`, `Select`, `Checkbox`, `Switch` keep their builders. New, and worth moving to: `SearchInput`, `PasswordInput`, `NumberInput`, `RadioGroup`, and `FormField` (or `form()`), which gives a control its label, description and error. A control that is invalid keeps its edge and a `!` mark when focused, so it never relies on colour alone. `NumberInput::value` and `default_value` take a number that is not finite (NaN, infinity) as no number, so the field is empty instead of showing `NaN`; `min` and `max` ignore a NaN.

## Navigation

`Tabs`, `ToggleGroup` (the segmented control) and `NavList` take `.density(..)`; `Breadcrumb` follows the application's; a selected item reads apart from a hovered one and from a focused one. There is no `Pagination` yet (`docs/spec/odk-ui-components.md`, "Left out of v1").

## Badges and status

A badge is a role at an emphasis, or a `Tone` (an application's name for a status). `.marked()` adds the status mark so it reads without colour.

```rust,ignore
Badge::new("New").role(Role::Accent).emphasis(Emphasis::Solid)
Badge::new("Failed").tone(Tone::Danger).marked()
StatusIndicator::new(Status::Active, "Online")
```

`Role` here is `odk::theme::semantic::Role` (six colour roles), not `odk::a11y::Role` (what a component is). `Alert::variant` and `Toast::variant` take a `Tone`, as they did; the name is older than `ButtonVariant` and is kept so existing calls compile (a known inconsistency, `docs/release-readiness.md`).

## Overlays

`DropdownMenu`, `ContextMenu`, `Popover`, `Tooltip`, `HoverCard`, `Dialog`, `Sheet` and `Toast` keep their builders. A popover or modal opened inside a modal is lifted to a host the `Shell` draws (ADR 0039). Toasts are a manager: `toast(window, cx, Toast::new(..))` returns after queuing, `Toast::persistent()` stays until dismissed, `Toast::timeout(..)` sets how long (a duration too long to count is a toast that stays), and `dismiss_toast` and `set_toast_limit` manage the stack. Placement is `Placement` and `Align`.

## Your own components

Follow `docs/contributing-components.md`: read `components()` and `foundation()` in `render`, never the palette, never compute a colour while drawing, take durations from `foundation().motion`.

```rust,ignore
let foundation = odk::ui::foundation();
let height = foundation.controls.metrics().height;
let ink = odk::ui::theme_snapshot().semantic.text.primary;
```

## Accessibility

Name every icon-only control (`Button::icon(id, glyph, name)` or `.accessible_name(..)`), give a dialog an action and a title, and do not build a control from a bare `div` with `on_click`. ODK reports roles and states through `odk::a11y::Describe`, but GPUI 0.2.2 has no accessibility tree, so a screen reader cannot read an ODK window yet. `docs/accessibility.md` says what works.
