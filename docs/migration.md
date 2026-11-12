# Migrating an application from GPUI to ODK on Rune

For applications written against `odk::ui` while it drew with GPUI. ADR 0094 made one breaking switch: `odk::ui` is `odk-view`, on Rune, and `odk::gpui`, `odk-ui`, `patches/` and the `gpui` pin are gone. ODK is at version 0.x (ADR 0064), so this is a change made on purpose, once. The code in this guide is compiled by `crates/odk/tests/migration.rs`, one function for each section, so a change to an API the guide shows fails there first.

## What does not change

The theme and everything under it: `odk::theme` (the palette, the semantic and component tokens, `ThemeManager`), `odk::a11y`, `odk::db`, `odk::plugin`. The component names are the same (`Button`, `Input`, `DataTable`, ...), and so are their builders where a builder is about what a component is: `variant`, `density`, `disabled`, `open`, `on_close`.

## What changes

| Before (GPUI) | After (Rune) |
|---|---|
| A view is a `Render` type with `&mut Window` and `&mut Context<Self>` | A view is a function `fn(&Hosted<T>, &mut Cx<Hosted<T>>) -> El` over the application's own state `T` |
| A handler takes `&mut Window` and `&mut App`, and calls `cx.notify()` | A handler takes `&mut T`; the window draws again by itself |
| `Button::new("save").label("Save").on_click(cx.listener(..))` | `Button::new("save", "Save", \|app: &mut T\| ..).show(hosted)` |
| A component is a GPUI element, `.child(button)` | A component's `show(hosted)` returns an `El`; a group is `Layout::of(hosted).stack(Space::Sm)` or `.row(..)` |
| `odk::gpui`, the raw escape hatch | Gone. `odk::ui::prelude` brings `Cx` and `El`, which are Rune's |
| `Stack::new()`, `Row::new()`, `Grid` | `Layout::stack`, `Layout::row`, `Layout::row_stretch`, `Layout::sidebar`, and Rune's `col()` and `row()` for your own drawing |
| An overlay (`Dialog`, `Popover`, `DropdownMenu`, `Select`) is a child of the page | It draws into a `layers` list that you give `frame`; what a modal's content opens goes after the modal (see "Overlays") |
| `odk::ui::theme_snapshot()`, `foundation()`, `components()` anywhere | `hosted.theme()`, `hosted.foundation()`, `hosted.components()`, from the `Hosted` your view is given |
| `Window::refresh`, `cx.notify()`, `Entity` | Nothing: an `Input`'s text, a `Select`'s open list and a caret live in the window's `Store`, and the rest is your `T` |

## Starting

`odk::ui::run` loads the theme before the window opens, so a component may read the tokens in the view from its first frame. It opens one window and returns when it closes. The font is the family `odkd` reports, found among the fonts the sandbox shows (ADR 0095); with none it is Rune's bundled one.

```rust,ignore
let window = WindowConfig::new((960.0, 680.0)).title("My app");
odk::ui::run(App::default(), window, view)
```

The view puts the page and its layers in `frame`:

```rust,ignore
fn view(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    frame(hosted, cx, buttons(hosted), [])
}
```

## Density, corners and motion

One setting each, for the whole application, changed at any time: the next frame draws with it. At start, in the `WindowConfig`; later, from a handler, through the process's poster, which returns `NoWindow` when the application did not start through `run`.

```rust,ignore
odk::ui::configure::<App>(|c| {
    c.density = Density::Compact;            // 28 px controls at the default text size
    c.corner_radius = CornerRadius::Square;  // 0 px; Subtle (the default) is 2 px
})?;
odk::ui::set_reduced_motion::<App>(true)?;   // adds to the user's own request, never removes it
```

`CornerRadius`, `Density` and `FoundationConfig` are in `odk::theme::foundation`. A control may take another density than the application's with `.density(..)`.

## Following and previewing themes

By default the application follows Omarchy's current theme: the palette, the user's text size and `odkd`'s request for reduced motion change from the next frame. To show another theme without choosing it (a theme picker, a design tool), preview it; Omarchy's own files are not touched, and a change the system makes meanwhile is kept and drawn when the preview ends. The user's request for reduced motion is not a theme's, and follows the user under a preview.

```rust,ignore
let mut manager = ThemeManager::following_host(ThemeSource::named("nord")?);
manager.reload();
odk::ui::preview_theme((*manager.snapshot()).clone())?;
// ...and back to the theme the system follows.
odk::ui::end_theme_preview()?;
```

Tune how colours are resolved (strategy, preferred source colours, stricter contrast) with `odk::ui::set_colour_options(ResolveOptions { .. })`; it never lets contrast go below 4.5:1 for text or 3:1 for indicators, and refuses options that would. `odk::ui::reload_theme()` reads the theme again.

## Buttons

Eight variants, an icon-only button with a required name, a busy state that keeps the size. A handler takes the application's state and is called once per completed press, from the pointer or from Enter and Space; a disabled or busy button ignores a second activation.

```rust,ignore
Button::new("save", "Save", |app: &mut App| app.saved += 1)
    .variant(ButtonVariant::Primary)
    .show(hosted)
```

`ButtonVariant` is `odk::theme::component::ButtonVariant`.

## Form controls

`Input`, `Textarea`, `Select`, `Checkbox`, `Switch`, `SearchInput`, `PasswordInput`, `NumberInput`, `RadioGroup` and `FormField` take the application's state in their handlers (`on_change(|app: &mut App, value| ..)`). A control is controlled when you give it its value and uncontrolled when you give it a default: an uncontrolled one keeps its state in the window's `Store`, so the application gains no field for it.

## Navigation

`Tabs`, `ToggleGroup` (the segmented control) and `NavList` take `.density(..)`; `Breadcrumb` follows the application's. A selected item reads apart from a hovered one and from a focused one.

## Badges and status

A badge is a role at an emphasis, or a `Tone` (an application's name for a status). `.marked()` adds the status mark so it reads without colour.

```rust,ignore
Badge::new("New").role(Role::Accent).emphasis(Emphasis::Solid).show(hosted)
Badge::new("Failed").tone(Tone::Danger).marked().show(hosted)
StatusIndicator::new(Status::Active, "Online").show(hosted)
```

`Role` here is `odk::theme::semantic::Role` (six colour roles), not `odk::a11y::Role` (what a component is). `Alert` and `Toast` take a `Tone` with `.tone(..)`; it was `.variant(..)` on GPUI's, a name `ButtonVariant` owns.

## Overlays

`DropdownMenu`, `ContextMenu`, `Popover`, `Select`, `Tooltip`, `HoverCard`, `Dialog`, `Sheet`, `Command` and `Toast` draw into a `Vec<El>` of layers that you pass to `frame`, and a later layer is drawn above an earlier one. A modal is modal to the page and to the layers before it, so a menu, a select or a popover in a `Dialog`'s or a `Sheet`'s content must be put after the modal in the list: build the content with its own `inner` list, `show` the modal into `layers`, then `layers.extend(inner)`. Escape closes the top one first (`crates/odk-view/tests/nested.rs`). Under reduced motion every overlay appears and goes at once.

## Layout

A group is `Layout::of(hosted)` (made once per view; it reads the theme) and a `Space` for the gap:

| Before | After |
|---|---|
| `Stack::new().gap(Space::Sm)` | `layout.stack(Space::Sm)` |
| `Row::new().gap(Space::Sm)` | `layout.row(Space::Sm)` |
| `Stack::page()` | `layout.content_column(..)` |
| `sidebar_frame()` | `layout.sidebar(sidebar, content)` |

For drawing of your own, `rune::col()` and `rune::row()` are the primitives, and `odk::ui::ink(color)` turns a token's colour into Rune's.

## Your own components

Follow `docs/contributing-components.md`: read `hosted.components()` and `hosted.foundation()` in the view, never the palette, never compute a colour while drawing, take durations from `foundation().motion`, and draw a focus ring with the component tokens' (`style::focus_ring`).

```rust,ignore
let foundation = hosted.foundation();
let height = foundation.controls.metrics().height;
let ink = hosted.theme().semantic.text.primary;
let tokens = hosted.components();
```

## Several windows

`odk::ui::Windows` is `run` for an application of more than one window: the first is the application (closing it ends it), the others open and close while it runs, each with a state of its own.

```rust,ignore
let mut windows = Windows::new(App::default(), WindowConfig::new((960.0, 680.0)), view);
let settings = windows.window("settings", Settings::default(),
    WindowConfig::new((480.0, 360.0)).title("Settings"), settings_view);
// A handler of the main window keeps `settings` (a `Window`, cheap to clone) and calls
// `settings.show()`; `hide()` closes it keeping its state; `focus()` gives it the keyboard.
windows.run()
```

Every window draws the same theme, follows it together, and takes a preview of it together. What reaches a window from outside (`configure::<T>`, `set_reduced_motion::<T>`, `on_host::<T>`, a `FileChooser` answer) names a type of state and reaches the first window whose state is that type, so give each window its own type. A window's own foundation settings (density, corner radius) are its own.

## Accessibility

Name every icon-only control (`Button::icon(id, glyph, name)` or `.accessible_name(..)`), give a dialog an action and a title. ODK reports roles and states through Rune's accessibility tree (`odk-a11y`'s `Describe`, applied by `describe::apply`); a sandboxed application reaches a screen reader through `odkd`'s bus (ADR 0096).
