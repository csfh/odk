# ODK UI components, version 1

Normative. This spec lists the components `odk-ui` ships in v1 (ADR 0027). It must not change a decision recorded in an ADR. Components use only the tokens of `odk-theme-tokens.md`, or, once they have moved, the component tokens of `odk-component-tokens.md` and nothing else.

| Group | Components |
|---|---|
| Layout and text | `v_flex`, `h_flex`, `Heading`, `Text`, `Separator`, `Kbd`, `Breadcrumb`, `Resizable`, `Image` |
| Controls | `Button`, `Input`, `InputGroup`, `Textarea`, `SearchInput`, `PasswordInput`, `NumberInput`, `Checkbox`, `Switch`, `RadioGroup`, `Select`, `ToggleGroup`, `Tabs`, `NavList` |
| Forms | `FormField`, `form` |
| Feedback | `Badge`, `StatusIndicator`, `Tooltip`, `Spinner`, `Progress`, `Skeleton`, `Alert`, `Toast` |
| Overlays | `Dialog`, `Sheet`, `Popover`, `HoverCard`, `ContextMenu` (sharing one menu surface with a `DropdownMenu`), `Command` |
| Data | `DataTable`, `Chart` |

Names and behaviour follow shadcn/ui; the look does not (ADR 0027, ADR 0035). Every component takes `variant` and `size` where they make sense, composes with `.child`, and follows the theme without application code (ADR 0019).

## Conventions

- Windows are opened with `odk::ui::open_window`, not GPUI's `open_window`, from a `WindowConfig`: `WindowConfig::new(app_id)` with `title`, `size(width, height)` and `min_size(width, height)` in plain numbers, so an application needs none of GPUI's types. The size it opens at is given at the 12 px text size and grows with the user's text size (ADR 0043), never past the display; the smallest size is in pixels as given and never past the display, and the opening size is at least it (`docs/checks.md`, "Window sizes"). GPUI's `WindowOptions` is still accepted for what a config does not cover. It wraps the application's view in a `Shell`: the theme's base surface, the window's toasts, and the host for overlays opened inside a modal (ADR 0039). A window opened the raw way has none of these, and asking it for a toast or such an overlay panics, naming `open_window`. The application's view starts with `root()`, which fills the window, paints the base surface and sets the text colour.
- `Space` (`Xs` to `Xl`) goes wherever GPUI takes a length, so `.gap(Space::Md)` and `.p(Space::Lg)` are GPUI's own methods. `p_xy(x, y)` is ODK's addition, and so is `text_role(role)`, which sets a foundation type role (`docs/spec/odk-foundation-tokens.md`).
- Components read the current tokens inside `render`, so a theme change takes effect on the next frame without application code.
- A theme that cannot be loaded never stops an application (ADR 0041): it starts on the emergency palette, or keeps the last theme that loaded. `odk::ui::theme_diagnostics` says which theme is drawn and why, `odk::ui::reload_theme` asks for the theme to be read again, and `odk::ui::theme_snapshot` gives the Palette to code that needs it; components draw tokens, never the Palette (`docs/spec/odk-source-palette.md`). The snapshot also carries the semantic colours of ADR 0040; `odk::ui::set_colour_options` changes how they are resolved (`docs/spec/odk-semantic-tokens.md`). `odk::ui::components()` gives each component's style by variant, density and state from them and the foundation (`docs/spec/odk-component-tokens.md`). Components move onto it from step 7 of `docs/plan.md`: `Button`, the form controls (see Form controls), `Text`, `Heading` and the navigation and feedback components (see Navigation and feedback) have, and draw nothing else; the overlays (`Dialog`, `Sheet`, `Popover`, `HoverCard`, `Tooltip`, the menus, `Command`), `Toast` and `Kbd` have too since 2026-10-23 (Phase 9 of the design language, `docs/spec/odk-overlays.md`); every component draws the component layer now. The legacy `Tokens` and `Variant` were removed on 2026-10-24 (plan step 9, ADR 0047); `root()`, `Space` and the `Shell` read the semantic and foundation layers, and `Tone` stays as the names of a status.
- Buttons are as wide as their content, not stretched across a column, unless they are told to fill it (`full_width`).
- Notices take a `Tone`: `Info`, `Success`, `Warning` or `Danger`. It picks a status colour and its tint. A `Badge` takes any of the six Roles (`odk::theme::semantic::Role`; a `Tone` converts), and a `StatusIndicator` a `Status`.
- Keys, marks and spinners use the monospace family an application chose with `odk::ui::configure`, else Omarchy's monospace font, the family `odkd` names, which is what fontconfig's `monospace` alias resolves to for the user, and again whenever it changes (ADR 0062, `docs/checks.md`). ODK has no icon set yet, so marks and a button's icons are characters, except a radio's dot, a switch's thumb and a number field's steppers, which are painted (ADR 0078).
- A part without a visible label of its own takes its accessible name in its constructor, so it cannot be left unnamed: `ToggleGroup::new(id, name)`, `Tabs::new(id, name)`, `NavList::new(id, name)`, `Progress::new(id, name)`, `HoverCard::new(id, name, trigger)`, `ContextMenu::new(id, name, trigger)`, `Input::new(id, name)`.
- Content that is shown later, such as a tab's panel or an overlay's body, comes from a builder (`.tab(value, label, |window, cx| …)`, `.content(|window, cx| …)`), which runs only while it is shown.
- Text that stays on one line ends in `…` through `truncate_end`, never GPUI's `truncate` (`docs/checks.md`, ADR 0058). Prose wraps. A caller changes that with a decoration (`Wrap`, `EndEllipsis`, `MiddleEllipsis`, `Clamp(n)`; ADR 0057). A truncated string keeps its full text in `Describe` and, where the control measures overflow, in a tooltip. Overlays are never wider than the window, less a margin. A dialog is a fixed width unless `Autosize`; a menu and a toast grow with their content and the text size (`Autosize` by default).
- An application can turn reduced motion on with `odk::ui::set_reduced_motion`. `odk::ui::configure` also sets density, corner radius and fonts, the foundation tokens of `docs/spec/odk-foundation-tokens.md`; every component draws them, and `Space`, `root()` and the `Shell` read the foundation's spacing and the semantic canvas and text, since the legacy `Tokens` were removed (ADR 0047).
- Stateful components such as `Tabs`, `ToggleGroup` and `NavList` are uncontrolled by default and keep their own value, starting from `default_value`. Passing `value` makes them controlled, and the application follows `on_change`.

## Focus and keyboard

- `run` binds Tab and Shift+Tab to move between tab stops in the focused window.
- Every interactive part is a tab stop with a focus handle of its own, but a field action whose job a key of the field does: a search field's clear button (Escape) and a number field's steppers (Up and Down) take the pointer only, and a press on any field action leaves focus in the field (`docs/spec/odk-text-input.md`, "Focus"). A focused part draws its border in `accent.base`, because Omarchy's `focus` border is faint by default; a component on component tokens draws its style's ring instead, 2 px wide and 2 px outside it (see Button). GPUI 0.2.2 cannot tell keyboard focus from pointer focus, so the ring also shows after a click.
- Enter and Space activate a focused button or link.
- A composite control (tabs, toggle group, a `NavList`, and later menus and lists) has one tab stop. The arrow keys move between its items and wrap at the ends; Home and End jump to the first and last item. Tabs select the item focus moves to, unless `manual_activation`, where Enter or Space selects. A toggle group only moves focus, and Space or Enter toggles. A `NavList` moves with Up and Down and opens a row only on Enter, Space or a click, since opening a page is more than moving.
- A path's links are separate tab stops, as links are: each link of a `Breadcrumb`, and its folded `…`.
- Its focused item's ring is painted by the composite control around the item, over the items after it (`docs/spec/odk-component-tokens.md`, "In GPUI").

## Overlays

The behaviour of every overlay is in `docs/spec/odk-overlays.md`; this is a summary. Overlays follow ADR 0037, amended by ADR 0039 and ADR 0045. Menus, popovers, hover cards, tooltips, side panels and sheets draw their overlay where they are declared, with `deferred`. One declared inside a modal's content is lifted instead, to a host that the window's `Shell` draws, because GPUI cannot nest `deferred` elements. The `Shell` also draws each window's toast stack. In layer order, popovers are lowest, then modals, then what opens inside a modal, then toasts.

- **Menus** (`ContextMenu`, `DropdownMenu`) share one surface. It opens with focus on the first enabled item. Up and Down move between enabled items and wrap, Home and End jump, and Enter or Space chooses. Escape, Tab, Shift+Tab or a press outside closes it, and that press presses nothing under it (ADR 0045). Typing a letter jumps to the next item that starts with it. Closing gives focus back to whatever had it before. A dropdown opens above its trigger when there is no room below. `ContextMenu` opens at the pointer on a right-click, or below its trigger with the Menu key or Shift+F10. `DropdownMenu` opens below its button on a press, Enter, Space or Down, and a second press closes it (it toggles on the press, not the click, because a frame is drawn between the two).
- **Popover** opens on a press on its trigger or Enter or Space, takes focus, does not trap Tab (Tab and Shift+Tab leave it beside the trigger) and leaves the window behind it live; Escape, and a press outside unless `close_on_outside_click(false)`, close it and focus goes back to the trigger.
- **Hover card** opens once the pointer has rested on the trigger for the foundation's `hover_open`. It closes `hover_close` after the pointer has left both the trigger and the card. Focusing the trigger opens it at once, and Escape closes it. It never takes focus.
- **Sheet** is controlled (`open`, `on_close`). It slides in from the left, right or bottom over a scrim and takes focus, and Tab stays inside it. Escape, the close button or a click on the scrim calls `on_close`. `modal(false)` makes it a side panel: no scrim, no focus taken or trapped, the window behind live, Escape closing it while focus is inside. A side sheet is 400 px wide and a bottom sheet 400 px tall unless `extent` says otherwise.
- **Toast**: `toast(window, cx, Toast)` adds one to the window's stack, in the bottom-right corner. Each toast dismisses itself after the foundation's `toast` (twice that with an action), unless it is `persistent()` or has a `timeout`. The clocks pause while the pointer is over the stack or focus is in it. Collapsed, identical on-screen toasts are one card with a count; hovering the stack or focusing it expands them, which is the same condition that pauses the clocks, and the column scrolls when it is still taller than the window (ADR 0058). At most three are shown (`set_toast_limit`) and later ones wait; a toast with an id that is already shown updates it in place. The limit counts toasts, not grouped cards. A toast never takes focus, though its buttons are tab stops, which a modal that traps focus keeps out of reach of the keyboard. A description joins the accessible name, as an alert's does.

## Motion

Components animate with the foundation's durations (`docs/spec/odk-foundation-tokens.md`); the overlays' are in `docs/spec/odk-overlays.md`: a dialog and a sheet come in over `enter` and go over `exit`, a tooltip fades in over `fast`, menus, popovers, hover cards and toasts come in over `normal`; a button's spinner fades in and out over the foundation's `normal`; a tab's indicator fades from the old tab to the new over `normal`; a progress bar moves to a new value over `normal`; a spinner turns once per `motion.cycle`. A skeleton stands still. When `motion.reduced` is set, everything shows its end state at once and the spinner stands still.

## Accessibility

Every component implements `odk_a11y::Describe` and reports a role, a name and its state (ADR 0036). Composite controls also report their items (`item_accessibles`). Nothing reads these yet.

## Checklist

Every component must pass the checklist in `component-checklist.md`, which also lists where built components still fail it.

## Built so far

| Component | Status | Notes |
|---|---|---|
| `v_flex`, `h_flex`, `root`, `Space` | built | |
| `sidebar_frame`, `content_column`, `scroll_y` | built (2026-10-24) | Layout helpers, not components: no checklist item applies to them as such, and `checklist/layout.rs` tests them. An application's view uses them in place of GPUI's `px`, `ScrollHandle` and bare `div()`, which the prelude does not give it. `sidebar_frame(sidebar, content)` fills its parent with the sidebar at `layout.sidebar_width` on the recessed surface, its content inset by `layout.sidebar_padding` on every side (so a row's fill and the current page's indicator stand clear of the edges, as in the gallery), a vertical `Separator` and the content taking the rest, laid out in a box that is given its size (absolutely filling the room the row leaves) because a scroll region under a flex-sized item measured its children at the width of their content. `content_column()` is a column as wide as its parent up to `layout.content_width`, centred, with `layout.panel_padding` and `layout.section_gap`. `scroll_y(id)` takes the room its parent leaves, as wide as the parent, and scrolls (without its own width a field under a long paragraph was measured at the width of its icon); it is a block, not a flex column, because the children of a flex column shrink to fit and never scroll, so put one `v_flex()` or `content_column()` inside it. Widths follow the user's text size (`docs/spec/odk-foundation-tokens.md`, Layout) |
| `Heading`, `Text` | built, on component tokens | `Text::new(content)`, `muted()` for secondary text, `mono()` for what is read as characters (a command line, a path; the `mono` type role), `role(TypeRole)` for another type role, `single_line()` for `decorate(EndEllipsis)` (give a row child `flex_1` and `min_w_0`); `Heading::new(content)`, `level(n)`. Body text is the `body` type role and wraps; `decorate` attaches `EndEllipsis`, `MiddleEllipsis`, `Wrap` or `Clamp` (ADR 0058). A heading's level picks its role: 1 `title`, 2 `heading`, 3 and higher `subtitle`, while the level it reports stays the one it was given (1 to 6). Since 2026-10-23, so the titles and descriptions of `Dialog` and `Sheet` draw them before those panels move |
| `WindowConfig` | built (2026-10-28) | The application id, title, opening size and smallest size of a window, for `open_window`; no GPUI type in its API. Unit tests in `window.rs` |
| `FileChooser` | built (2026-10-28) | Not a component: the desktop's file chooser, which `odkd` opens for the application (ADR 0068), so a sandboxed one can ask too. `FileChooser::open(title)`, `save(title)`, `folder(title)`; `multiple`, `filter(label, patterns)`, `accept_label`, `name` (a save dialog's suggested name), `start_in(StartFolder)` (`Home`, `Desktop`, `Documents`, `Downloads`, `Music`, `Pictures`, `Videos`, which `odkd` looks up outside the sandbox, or `Path`), then `choose(cx, \|chosen, cx\| …)`: `Ok(Some(paths))`, `Ok(None)` when the user cancelled, or `Err(FileChooserError)` when no dialog could be shown (`NoHost` without `odkd`, `Failed(why)` when it refused). A path is a name and not access; a Bundle hands it to its daemon. The dialog is the desktop's own window, not attached to the application's. Not in the gallery, which would open a real dialog on the desktop: tested against a fake `odkd` (`files.rs`) and, in `odkd`, against a fake portal on a private bus |
| `open_window`, `Shell` | built | Opens a window with the base surface, the toast host and the overlay host around the application's view, and sets the window's text in the UI face (Omarchy's font unless the application names one), which everything below inherits, overlays included. Every size follows the user's text size (ADR 0043). |
| `preview_theme`, `end_theme_preview`, `previewed_theme`, `followed_theme` | built | Draw a `ThemeSnapshot` in every window in place of the followed theme, and give the followed one back as it is then; for tools that show a theme without choosing it, such as the gallery. Load the snapshot with `ThemeManager::with_users_files` (`docs/spec/odk-source-palette.md`, "Previewing a theme") |
| `Button` | built, on component tokens | Eight `ButtonVariant`s and an `emphasis`; the application's density, its own `density`, or a `Size` step from it; `leading_icon`, `trailing_icon`, `Button::icon(id, glyph, name)`; `loading`, `disabled`, `accessible_name`, `full_width`, `target_size`; `listening` and `on_chord` take the next key press as a `Chord` (ADR 0076). See Button below |
| `Kbd` | built, on component tokens | `Kbd::new("Ctrl+K")` or `Kbd::keys([...])` |
| `FormField`, `form` | built, on component tokens | `FormField::new(id, label).child(control)`; `description`, `error`, `warning`, `success`, `help`, `required`, `optional`, `disabled`. `form()` is a column of fields 16 px apart. See Form controls |
| `SearchInput`, `PasswordInput`, `NumberInput` | built, on component tokens | Each an `Input` with what it adds (`docs/spec/odk-text-input.md`, "Search, password and number fields"). See Form controls |
| `RadioGroup` | built, on component tokens | `RadioGroup::new(id, name)`; `item`, `disable`, `value`, `default_value`, `on_change(&str)`, `disabled`, `invalid`, `required`, `horizontal`, `density`. See Form controls |
| `Spinner` | built, on component tokens | `size` a step from the density, `density`, `label`. Secondary text standing alone, the control's foreground inside a `Button` or a field; one implementation for both. See Navigation and feedback |
| `Alert` | built, on component tokens | `variant(Tone)`, `emphasis` (muted by default), `compact`, `description`, `action`, `on_dismiss`. See Navigation and feedback |
| `Breadcrumb` | built, on component tokens | Items take `Fn(&mut Window, &mut App)`. A path too long for its width truncates its labels, the middle first. `max_items` folds the middle into a `…` that opens a menu of the hidden items. Its links are `link.rs`'s, which the `…` shares |
| `ToggleGroup` | built, on component tokens | The segmented control. Single and multiple; `item`, `icon_item` and `icon(value, glyph)`; `disable(value)`, `disabled`, `density`. As wide as its options |
| `Tabs` | built, on component tokens | `disable(value)`, `disabled`, `icon(value, glyph)`, `counter(value, text)`, `manual_activation`, `density`. The selected tab has primary text and an accent indicator over the list's baseline, no fill. Tabs that do not fit scroll sideways, and the selected one is brought into view when the selection changes, the focused one when the arrows move. That scroll is the one truncation exception: tabs take no truncation decoration (ADR 0058) |
| `NavList` | built, on component tokens | A sidebar's navigation: `NavList::new(id, name)`; `group(heading)`, `item(value, label)`, `icon`, `counter`, `disable`, `disabled`, `density`, `value`, `default_value`, `on_change`. See Navigation and feedback |
| `StatusIndicator` | built, on component tokens | `StatusIndicator::new(Status, label)`: a dot and a label that is always shown. See Navigation and feedback |
| `Progress` | built, on component tokens | Determinate. `Progress::new(id, name)`; `value`, `max`, `tone`, `paused`, `disabled`, `show_label`, `show_value`, `value_text`. See Navigation and feedback |
| `Skeleton` | built, on component tokens | `Skeleton::line()`, `text(lines)`, `block(width, height)`, `label`. Still. See Navigation and feedback |
| `ContextMenu`, `DropdownMenu` | built; their list on component tokens | `Menu` with `item`, `separator`, `heading`; `MenuItem` with `hint`, `icon`, `checked`, `radio`, `disabled`, `danger`, `on_select`. Typing a letter jumps to the next item that starts with it. A menu taller than the window is capped to it and scrolls, keeping the focused item in view. The list is the one a `Select` opens (`menu_list.rs`), on `menu()` and `menu_item()` since 2026-10-23: items a control's height, hover, highlight and checked drawn apart, fading over the foundation's `fast`; a context menu's trigger draws the component layer's ring |
| `HoverCard` | built, on component tokens | `placement`, `align`; opens below its trigger, on the other side when there is no room. Its trigger's ring is the component layer's |
| `Popover` | built, on component tokens | `Popover::new(id, name, trigger)`; `title`, `content`, `placement`, `align`, `open` / `default_open` / `on_open_change`, `close_on_escape`, `close_on_outside_click`. Interactive content, takes focus, not modal. Added 2026-10-23 (Phase 9) |
| `Sheet` | built, on component tokens | `Side::Left`, `Right`, `Bottom`; `modal(false)` makes it a side panel; `close_on_escape`, `close_on_backdrop`. Content comes from a builder, `.content(\|window, cx\| …)`, which the sheet keeps, with its title, side and size, until its exit has run |
| `Toast` | built, on component tokens | `toast(window, cx, Toast::new(title).variant(..).description(..).action(label, handler))`; `neutral`, `persistent`, `timeout`, `with_id(ToastId)`, `dismiss_toast`, `set_toast_limit`, `decorate` (`Autosize` by default, or `FixedWidth`). Identical on-screen toasts collapse to one card with a count until the stack is hovered or focused. The action sits under the text, so the text keeps the toast's width in a narrow window; the dismiss `×` is an icon-only button |
| `InputGroup` | built, on component tokens | `InputGroup::new(id, name)`; every option of `Input`, plus `prefix(text)`, `suffix(text)` and `action(Button)`. One field's box around a bare `Input`, exactly as high as an `Input` of its density; its button is a field action, inside the box and lower than it, at the group's density and taking its `disabled`. A press on the text around the field focuses it |
| `Command` | built, on component tokens | `Command::new(id, name)`; `open`, `on_close`, `placeholder`, `empty`, `item(CommandItem)`; `CommandItem::new(label)` with `hint`, `keywords`, `disabled`, `on_select`. A modal palette: a field over a list narrowed by substring match on every word of the query, highlight moved with Down and Up (wrapping, skipping disabled), Enter runs it, Escape closes. Focus stays in the field. Keeps its commands while it fades out |
| `Textarea` | built, on component tokens | `Textarea::new(id, name)`; `rows` (three by default) or `min_rows`, `max_rows`, `value`, `default_value`, `placeholder`, `size`, `density`, `disabled`, `read_only`, `masked`, `invalid`, `required`, `on_change(&str)`. `Input`'s state and element with several lines: Enter ends a line, text wraps and breaks inside a word that does not fit, Up, Down, PageUp and PageDown move by row and keep their column, the wheel scrolls it, a triple-click selects the line. The box is `rows` lines high however much is typed, or grows with its text up to `max_rows` |
| `Resizable`, `ResizablePanel` | built, on component tokens (2026-10-28) | ADR 0069. `Resizable::horizontal(id, name)` or `vertical`; `panel(ResizablePanel::new().default_size(f).min_size(f).max_size(f).child(…))` with sizes as fractions of the group; `disabled`, `on_layout(\|sizes, window, cx\| …)`. Fills its parent; each panel grows by its fraction into what the handles leave. A handle is a line one border wide (`resize_handle`), the accent at the focus width while hovered or dragged, with a `target_min` pointer target over the panels' edges that takes no room. A drag holds the window until the button is released, Escape puts it back, and it is reported once at its end; each handle is a tab stop, the arrows along the group move it by a twentieth, Home and End take the panel before it to its limits, each key reported. Reports `Group`, and each handle `Separator` with the panel before it as its value. No motion: a resize follows the pointer or the key at once |
| `DataTable`, `Column`, `SortDirection`, `SortKey` | built, on component tokens (2026-10-28) | ADR 0070. `DataTable::new(id, name, rows)`, generic over the application's row; `column(Column::new(key, header, \|row\| text))`, `Column::cell(key, header, \|row, window, cx\| element)` or `Column::action(key, header, \|row\| label, \|row, window, cx\| …)` (a pointer-only button whose action belongs in the row's menu too), with `grow(weight)` or `chars(n)` (characters of the table's text, so it follows the text size), `align_end`, `sort_by(\|row\| key)`; `row_key`, `indent` (a tree's depth), `default_sort` or a controlled `sort` with `on_sort`, `selected` / `on_select`, `on_activate`, `activate_on_click`, `row_menu(\|row, menu\| …)`, `empty`, `loading`, `density`. Rows are a control's height and drawn only in view on GPUI's `uniform_list`; the header stays in place with `▲`/`▼` on the sorted column. The sortable headers are one tab stop (Left, Right; Enter or Space sorts, again reverses) and the rows the next (Up, Down, Home, End, Page Up, Page Down select and scroll; Enter activates; the Menu key or Shift+F10 opens the selected row's menu, a right-click the clicked row's). The selection is a row's key, so it survives a re-sort. A cell's text ends in `…` with the whole of it in a tooltip on hover. Reports `Table`, `ColumnHeader` with its sort, and `Row` with its cells' text |
| `Chart`, `ChartHeight`, `ChartPath`, `Scene`, `Ink`, `StrokeWidth`, `TextAnchor`, `TextBaseline` | built, on component tokens (2026-10-28) | ADR 0071, ADR 0075. A canvas, not chart kinds: `Chart::new(id, name)` fills its parent, or with `height(ChartHeight::Compact \| Regular \| Tall)` is that tall and as wide as its parent, and `paint(\|scene, t\| …)` draws in the box's coordinates with `fill`, `stroke`, `fill_rect`, `stroke_rect` and `text` (the `small` type role, anchored, with an optional halo), paths of `move_to`, `line_to`, `curve_to`, `arc` and `close`. Colours are `Ink`s (`Series(n)`, `SeriesFill(n)`, `Heat(0..=4)`, `Role(..)`, `RoleFill(..)`, `Text`, `TextMuted`, `Grid`, `Axis`, `Canvas`), widths `Thin`, `Regular`, `Thick` (`chart()` tokens). `transition(generation)` runs `t` from 0 to 1 over `normal` (at once with reduced motion), asking for frames only while it moves and is in view. `on_press`, `on_pan` (a drag, Shift and the wheel, the arrows) and `on_zoom` (the wheel, `+`, `-`) make it a tab stop; Enter or Space presses its middle. Reports `Image` with its `description` |
| `Image`, `ImageFit` | built, on component tokens (2026-10-28) | ADR 0072. `Image::new(id, name).source(bytes)` draws a PNG, JPEG, WebP, GIF or BMP from an `Arc<[u8]>` the application holds, never a path (a sandboxed window shows what its daemon read), filling its parent's box, `fit(ImageFit::Cover)` (default) or `Contain`, with the application's corners; `Image::avatar(id, name)` is a circle the foundation's `s12` across. No bytes yet: a still skeleton, reported busy; bytes that are not an image: a placeholder with `×`, reported as not shown. The same `Arc` is decoded once. Reports `Image` with its name |
| `Separator` | built, on component tokens | `Separator::horizontal()` and `Separator::vertical()`, `inset()`; spans its column or row, one border width thick in the subtle border colour, never squeezed |
| `Badge` | built, on component tokens | `Badge::new(label)`; `tone(Tone)`, `role(Role)`, `emphasis`, `appearance(BadgeAppearance)`, `outlined`, `icon`, `marked`, `decorate`. Every look has the same border, so a change of status never changes its size. A label longer than the column ends in `…`, like a bounded button, and the full label is in a tooltip on hover (ADR 0058) |
| `Checkbox` | built, on component tokens | `Checkbox::new(id, label)`; `checked`, `default_checked`, `indeterminate`, `disabled`, `invalid`, `required`, `density`, `on_change(bool)`. Space toggles, Enter does not; a click on the label toggles and focuses the box |
| `Switch` | built, on component tokens | `Switch::new(id, label)`; `checked`, `default_checked`, `disabled`, `density`, `on_change(bool)`. Space or Enter toggles; the thumb slides over the foundation's `normal` on its curve and starts where the switch is |
| `Tooltip` | built, on component tokens | `Tooltip::new(id, text, trigger)`. Above its trigger, below when there is no room; `placement`, `align`; opens on hover after the foundation's `hover_open` or at once on focus inside the trigger; stays open while the pointer is over it; Escape closes it; adds no tab stop |
| `Dialog` | built, on component tokens | `Dialog::new(id, title)`; `open`, `description`, `content(\|window, cx\| …)`, `action(\|window, cx\| …)`, `on_close`, `decorate` (`Autosize` or `FixedWidth`; the default is a fixed 480 px). The title wraps up to three lines, then ends in `…`; `Wrap` lifts the cap. Fades and rises into the centre, traps Tab and gives focus back on close; a click on the scrim or Escape closes it unless `close_on_backdrop(false)`, `close_on_escape(false)` or `close_button(false)` say otherwise; enters over the foundation's `enter`, leaves over `exit`. In a small window the body scrolls and the actions wrap, so none is pushed out of reach |
| `Input` | built, on component tokens | `Input::new(id, name)`; `value`, `default_value`, `placeholder`, `size`, `density`, `disabled`, `read_only`, `masked`, `invalid`, `required`, `leading_icon`, `trailing_icon`, `clearable`, `on_change(&str)`, `on_submit(&str)`. One line, on the text input of ADR 0038 (`docs/spec/odk-text-input.md`). Tab reaches it and selects all; the placeholder truncates with `…`; an unfocused value that does not fit ends in `…`, and a focused one scrolls inside a box that never moves |
| `Select` | built, on component tokens | `Select::new(id, name)`; `item(value, label)`, `disable(value)`, `placeholder`, `size`, `density`, `disabled`, `invalid`, `required`, `value`, `default_value`, `on_change(&str)`. A box the size of an `Input` showing the chosen label (or the placeholder, also for a value that matches no option) and `▾`, which opens a list below it (above when there is no room below) on the shared menu surface: as wide as the box at least, scrolling when it is taller than the window, with a check mark at the end of the chosen option's row; its labels start where the box's value does (`select_item`). The pointer opens it on press, like a native select; Enter, Space, Down and Up open it on the chosen option. Typing matches labels by prefix, ignoring case, on the closed box (which opens on the match) and in the list; the same letter again steps through its options and a pause of a second starts a new search. Space chooses and is never part of a search. A select with nothing to choose does not open. No groups, no searching and no multiple choice in v1. Inside a `Dialog` or `Sheet` its list is drawn above the modal (ADR 0039). |

## Button

The first component on component tokens (ADR 0040, `docs/spec/odk-component-tokens.md`). Everything it draws is `odk::ui::components().button(...)` for its appearance, density, layout and states: no legacy token, no palette colour, no colour computed while drawing.

**Variants.** `variant` takes a `ButtonVariant`.

| Variant | Role, emphasis | Use it for |
|---|---|---|
| `Primary` | accent, solid | the one main action of a view or a dialog |
| `Secondary` (default) | neutral, muted | ordinary actions |
| `Outline` | neutral, subtle, with an edge | a secondary action that must read as a button among quiet ones, such as a toolbar's |
| `Ghost` | neutral, subtle | quiet actions in dense places: a close `×`, a toolbar, a row's actions |
| `Destructive` | danger, solid | an action that destroys something; give it a word that says so |
| `Success` | success, solid | confirming something that completes |
| `Warning` | warning, solid | an action to think twice about that is not destructive |
| `Link` | accent, underlined text, no fill | an action that reads as a link in a sentence or a footer |

**Emphasis.** Each variant is a role at an emphasis; `emphasis(Emphasis::Muted)` keeps the role and changes the emphasis. Solid is a filled background with a neutral label; muted a tint of the role with text in the role's colour; subtle no fill until hovered. A muted `Destructive` is a less prominent destructive action. Every role takes every emphasis and every combination is checked for contrast in every theme. A `Link` has no fill in any state, so its emphasis changes nothing.

**Density and size.** A button is 28, 34 or 40 px high at compact, default and comfortable density, with 10, 12 or 16 px of padding and 14, 16 or 18 px icons. It takes the application's density (`odk::ui::configure`) unless given its own with `density`; a `Size` is a step from the density it would otherwise take (`Sm` one tighter, `Lg` one roomier, *provisional*). Text, icons and the spinner sit on one centred line at every density. Its width is its content's, and it never gives up width in a crowded row; with `full_width` it fills what it is given and a label that does not fit ends in `…`.

**Geometry.** Corners are the application's: 2 px (`CornerRadius::Subtle`, the default, ADR 0042) or 0 (`Square`), never more and never per button; hover, press, focus and loading keep them. Borders are 1 px in every variant, transparent where the variant has none, so no state moves anything. An icon-only button is a square as high as its density, not a circle. No shadow, no gradient.

**States.** Hover and press change the colours at once (GPUI 0.2.2 has no style transitions), press more than hover, in the same hue. The ring of a focused button is 2 px wide, 2 px outside it, in its role's focus colour (the accent's for a neutral button), with the button's corners; it is drawn over hover and press and moves nothing. A disabled button draws its variant's disabled treatment, takes no pointer or key, and leaves the Tab order. A loading button keeps its size, colours, focus and Tab stop, ignores presses, and shows a spinner in its foreground at its icon size: in its icon's place, keeping the label, or over the label when it has no icon. The spinner fades in and out over `motion.normal`, at once and standing still with reduced motion. A press that is released off the button does nothing.

**Keyboard.** Tab and Shift+Tab reach every enabled button once, in layout order. Enter and Space press the focused button on the key's release; a key held down presses it once. Inside a `Dialog` or `Sheet`, focus returns to the button that opened it when it closes.

**Icons.** `leading_icon` before the label, `trailing_icon` after it, `Button::icon(id, glyph, name)` alone: the density's icon size, the density's gap from the label. An icon is a character until ODK has an icon set; a number field's steppers paint their `−` and `+` instead, so the two match in any font (ADR 0078). An icon-only button takes its accessible name in its constructor, since its glyph does not say what it does.

**Larger targets.** Every button is at least 28 px, above WCAG 2.5.8's 24. `target_size(44.0)` makes the area that takes the pointer at least 44 px each way, with the button drawn at its own size in the middle of it and showing hover and press anywhere in it; that area takes its space in the layout.

**Theming.** Colours follow the Omarchy theme on the next frame after it changes, through the one theme snapshot every component reads; density, corners and size do not change with a theme. In a monochrome theme such as Vantablack the variants are greys of different strength, so `Destructive`, `Success` and `Warning` need a word, and the gallery gives them their status marks (`×`, `✓`, `!`).

**Accessibility.** It reports `Role::Button`, its label or accessible name, busy while loading and disabled. Text reaches 4.5:1 over every surface in every variant, emphasis and pointer state; a disabled button's text is `text.disabled`, plainly fainter and not held to 4.5:1, which WCAG allows for inactive controls, and its label still says what it is.

**Migration.** `variant(Variant::Primary)` is `variant(ButtonVariant::Primary)` (the enum `Variant` was removed, ADR 0047), and `size(Size::Sm)` is the density one step tighter. What changed is how they look, and an `×` button is better as `Button::icon`:

```rust
// Before
Button::new("close").label("×").variant(ButtonVariant::Ghost).size(Size::Sm).accessible_name("Close")
// After
Button::icon("close", "×", "Close").variant(ButtonVariant::Ghost).size(Size::Sm)

// New
Button::new("delete").leading_icon("×").label("Delete").variant(ButtonVariant::Destructive)
Button::new("archive").label("Archive").variant(ButtonVariant::Destructive).emphasis(Emphasis::Muted)
Button::new("next").label("Continue").trailing_icon("→").density(Density::Comfortable)
```

## Form controls

The fields (`Input`, `Textarea`, `InputGroup`, `SearchInput`, `PasswordInput`, `NumberInput`, a `Select`'s box), `Checkbox`, `Switch`, `RadioGroup` and `FormField` are on component tokens since 2026-10-23 (`docs/spec/odk-component-tokens.md`): each draws its style from `odk::ui::components()` and nothing else, its hover, press and ring through `interaction.rs`.

**Look.** Flat: a recessed fill, a 1 px edge, no shadow and no gradient. Corners are the application's, 2 px (`Subtle`, the default, ADR 0042) or 0 (`Square`), on every rectangle, check boxes included; a radio is a circle and a switch a pill, by nature. Text is `body`, labels `body_strong`, descriptions and messages `small`.

**Densities.** A field and a select's box are as high as a `Button` of the same density, 28, 34 or 40, with 10, 12 or 16 px of padding and 14, 16 or 18 px icons, and their text centred; so a button, a field and a select in one row line up exactly. Each takes the application's density, or its own with `density`, or a `size` step from it, as a `Button` does. A textarea is rows of text with the field's padding. A check box and a radio are 14, 16 or 18 across, a switch's track as high and 24, 28 or 32 wide. A density never changes a colour.

**States.** Each control asks `States` and draws what it says. Hover and press change colours at once, never the edge of a focused or wrong field. Focus is a 2 px ring 2 px outside the control, over any other state; a wrong value keeps its danger edge in focus and its ring is the danger colour, so focus and an error are both visible. Disabled ignores the pointer and the keyboard, leaves the Tab order and draws the disabled treatment, keeping a check box's mark or a radio's dot. Read-only keeps the value selectable and copyable, with a quiet edge and no hover. A select's open list keeps its box's focused edge. Checked, chosen and on stay visible under the pointer.

**Not by colour alone.** A ticked box shows `✓` and a mixed one `–`; a chosen radio has a dot; a switch's thumb changes side; a wrong value shows `!` at a field's end, after a check box's label and after a radio group's first option; a `FormField`'s error, warning and confirmation have the status marks `×`, `!` and `✓`. In a monochrome theme such as Vantablack every control is grey and still reads. Say why a value is wrong in a `FormField`'s error.

**Keyboard.** Fields keep the text input's keys (`docs/spec/odk-text-input.md`). A `Select` opens on Enter, Space, Down or Up, moves with the arrows, Home and End and typed letters, chooses with Enter or Space, closes with Escape without choosing, and gives focus back to its box. A check box toggles on Space, a switch on Space or Enter. A radio group is one tab stop on its chosen option; the arrows move and choose, skipping disabled options, and Space chooses. A press on a `FormField`'s label focuses its text field or select.

**Lists.** A select's list (and every menu) is a control's height per item; the item under the pointer has a light tint, the highlighted one (where the keyboard is) a stronger one, and the chosen one a check mark, whichever of the others it is too. A menu's check column leads its rows; a select's ends them, so the options line up with the value in the box.

**Field actions.** A field's own buttons (a search's clear button, a password's show button, a number's steppers, an input group's action) are `Button`s drawn inside the field, lower than it, so they never change its height. A press on one keeps the field's focus, caret and selection.

**Forms.** `FormField` lays out a label, an optional description, the control and an optional message, 6 px apart, and `form()` stacks fields 16 px apart. It passes its state to the control: `error` makes it invalid, `required` required, `disabled` disabled, and the description and message become its accessible description. Validation is a role (danger, warning, success, or neutral helper text), never a colour of its own.

**Theming.** A theme change recolours every control on the next frame and keeps what is typed and chosen, where focus is, the density and the corners.

**Accessibility.** Each reports its role (`TextInput`, `Combobox` and `Listbox`, `Checkbox`, `Switch`, `RadioGroup` and `Radio`), its name, its value (never a password's), and disabled, read-only, invalid, required, busy, mixed and selected as they apply, with its `FormField`'s description. A number field that has been left holding text that is not a number adds "Enter a number" to that description and draws the sentence under the field. GPUI 0.2.2 has no accessibility tree, so a screen reader does not read them; the tests do (ADR 0036).

**Migration.** Existing calls compile and keep their meaning; what changed is how they look. `size(Size::Sm)` on a field or a select is now the density one step tighter, as on a `Button`, rather than a smaller font. A check box's corners follow the application, and a switch is a pill.

```rust
// Before: a label, a field and a message laid out by hand, colour the only sign of an error.
v_flex()
    .child(Text::new("Email"))
    .child(Input::new("email", "Email").value(email))
    .child(Text::new("Enter an address with an @").muted())

// After
FormField::new("email-field", "Email")
    .required(true)
    .error("Enter an address with an @")
    .child(Input::new("email", "Email").value(email).leading_icon("@"))

// A search, a password and a number, each an `Input` with what it needs.
SearchInput::new("find", "Search settings").shortcut("Ctrl+K").on_submit(|text, _, _| {})
PasswordInput::new("pw", "Password")
NumberInput::new("seats", "Seats").min(1.0).max(25.0).on_change(|n, _, _| {}).on_commit(|n, _, _| {})
RadioGroup::new("plan", "Plan").item("personal", "Personal").item("team", "Team")
```

## Navigation and feedback

`Tabs`, `ToggleGroup` (the segmented control), `NavList`, `Breadcrumb`, `Badge`, `StatusIndicator`, `Alert`, `Progress`, `Spinner`, `Skeleton` and `Separator` are on component tokens since 2026-10-23 (`docs/spec/odk-component-tokens.md`); `NavList`, `StatusIndicator`, `Progress` and `Skeleton` are new. Each draws its style from `odk::ui::components()` and nothing else, its hover, press and ring through `interaction.rs`.

**Look.** Flat, quiet, precise: no gradient, no shadow, no pill. Rectangles have the application's corners, 2 px (`Subtle`, the default, ADR 0042) or 0 (`Square`); only a spinner and a status dot are round, by nature. Borders are 1 px; separators and a segmented control's frame are the quiet border colours, never a control's edge.

**Tabs.** Secondary text, primary on the selected tab with a thin accent indicator (the focus width, 2 px) on the list's baseline, and a quiet tint under the pointer; no box and no fill. A tab may have an icon before its label and a counter after it. Selected, hovered, focused and disabled are drawn apart: the indicator marks selection, the ring focus, so a focused tab that is not selected shows only its ring. By default the arrows select as they move; with `manual_activation` they move focus only and Enter or Space selects, for panels that are slow to build. The indicator fades between tabs over `normal`.

**Segmented controls.** `ToggleGroup` is ODK's segmented control: segments in one frame with no dividers, text, an icon alone (a square segment) or an icon before text. The one that is on has the selected fill, the neutral muted fill under the pointer, so a segment under the pointer never passes for it. It is not a radio group: the arrows move focus without choosing, and Space or Enter toggles (see Focus and keyboard). A frame is 28, 34 or 40 high, its segments its height less the borders.

**Navigation list.** `NavList` is a sidebar's navigation: rows of a control's height with an optional icon and counter, under optional group headings in tertiary text. The current page has the selected fill and an accent indicator along its leading edge; focus is a ring, apart from it. One tab stop: Tab reaches the current page, Up and Down move, Home and End jump, Enter, Space or a click opens; moving never opens a page. Groups do not collapse.

**Breadcrumbs.** Ancestors are secondary text, primary and underlined at rest; the current page is primary text and not a link, and is not underlined; separators are tertiary. No box and no fill, so a path never looks like a row of buttons. Truncation and the folded middle are as before.

**Pagination** is not built: v1 has no consumer for it (see Left out of v1).

**Badges.** Compact: one line of small text and its borders (20 px), never a control's height, quieter than a primary button. A short badge is as wide as its label. A label longer than the column ends in `…` and the full label shows in a tooltip on hover; a badge is not a tab stop. Any Role (`Neutral`, `Accent`, `Success`, `Warning`, `Danger`, `Info`) at any Emphasis (`Solid`, `Muted`, `Subtle`), with or without an edge; a `Tone` is its Role, muted. A status is said by its label; `marked()` adds the Role's status mark, and `icon` any glyph.

**Status indicators.** A dot in its status's Role and a label that is always shown, so the status never rests on colour: `Active` the accent, `Inactive` a hollow grey ring, `Success`, `Warning`, `Error` and `Info` their Roles. Decoration and text; no focus.

**Alerts.** The tone's status mark, a title, an optional description, an optional action and an optional dismiss button. Muted by default: a quiet tint of the tone, a 1 px edge, text in the tone's colour. Subtle has no fill, ordinary text and an edge in the tone's colour; solid is a filled panel, for what must not be missed. `compact` tightens the padding. The action and the dismiss button are `Button`s, reached with Tab and pressed with Enter or Space, their centres on the title's line. An alert never takes or moves focus.

**Progress.** A determinate bar: the bar in the accent's colour (or a `tone`'s) on its tint, 4 px thick, as wide as its column; optionally its name above it and its value at the end of that line. `value` out of `max` (100 by default); a value past either end, an infinity or NaN, or a `max` that is not above zero draws safely, as nothing or all done. The value shown is whole per cent, never `100%` before the work is done, or `value_text` such as "3 of 7 files". A paused bar is grey and keeps its value. A new value moves the bar over `normal`, at once with reduced motion. Work of unknown length has a `Spinner`.

**Spinners and skeletons.** A spinner is a square of the density's icon size (`size` a step from it), secondary text standing alone and the control's colour inside a `Button` or a field: one implementation. It turns once per `motion.cycle` and stands still, still shown, with reduced motion. Its glyph is a function of GPUI's clock, the same for every spinner, so it is drawn again once per glyph (a timer, not a frame request, shared by the spinners of a view) and not at all while it is scrolled or clipped out of view (checklist item 35). A skeleton is the shape and size of what it stands in for, lines of body text or a block, in the neutral muted tint, and stands still. Each has an accessible name, "Loading" by default, that says what is coming.

**Separators.** Horizontal or vertical, one border width in the subtle border colour; `inset()` stops a control's padding short of each end. Decoration: no focus, no pointer.

**Density.** Tabs, segments and sidebar rows follow it, 28, 34 and 40 px, with its padding, icon size and gap; so does a standing spinner's size. Badges, status dots, progress bars, skeletons and alerts do not: they follow their text and layout spacing.

**Theming.** A theme change recolours them on the next frame and keeps the selected tab, the chosen segment, the current page, the progress, where focus is, the density and the corners. In a monochrome theme such as Vantablack every Role is a grey: selection still reads by the indicator, the fill and primary text, a status by its mark, its label or its ring.

**Accessibility.** Tabs report `TabList` and `Tab` with selected and disabled, and a counter in the tab's name; a segmented control `RadioGroup` and `Radio` (or `Group` and `ToggleButton` when multiple); a `NavList` `Navigation` and a `Link` per row, the current page selected; a breadcrumb `Navigation`, `Link` and `CurrentPage`; a badge and a status indicator `Text` named by their label; an alert `Alert` with its title and description; a progress bar `Progress` with its value as text, busy while it works; a spinner and a skeleton `Progress`, busy. GPUI 0.2.2 has no accessibility tree, so a screen reader does not read them and no alert is announced; the tests read the descriptions (ADR 0036).

**Migration.** Existing calls compile and keep their meaning; what changed is how they look. A badge takes the application's corners, a separator is quieter, the selected tab loses its fill, segments lose their dividers. `Spinner::size` is now a density step, as a `Button`'s is.

```rust
// Before: a status badge, only four tones.
Badge::new("Failed").tone(Tone::Danger)
// After: the same call, or any Role at any Emphasis, with its mark.
Badge::new("Failed").tone(Tone::Danger).marked()
Badge::new("New").role(Role::Accent).emphasis(Emphasis::Solid)

// Tabs with icons, counters and manual activation.
Tabs::new("mail", "Mailboxes")
    .manual_activation(true)
    .tab("inbox", "Inbox", |_, _| inbox()).icon("inbox", "↓").counter("inbox", "12")
    .tab("sent", "Sent", |_, _| sent())

// A segmented control of icons, at its own density.
ToggleGroup::new("view", "View").icon_item("list", "☰", "List").icon_item("grid", "▦", "Grid")
    .density(Density::Compact)

// New: a sidebar, a status, a progress bar and a skeleton.
NavList::new("nav", "Settings").group("Workspace").item("general", "General").icon("general", "⚙")
    .item("members", "Members").counter("members", "4").value(page).on_change(|p, _, _| {})
StatusIndicator::new(Status::Active, "Online")
Progress::new("copy", "Copying photos").value(42.0).show_label(true).show_value(true)
Skeleton::text(3)
Alert::new("disk", "Disk almost full").variant(Tone::Warning).compact(true)
```

Here `Role` is `odk::theme::semantic::Role`; `odk::ui::Role` is accessibility's.

## Left out of v1

A combobox (a field whose list narrows as it is typed into; `Command` is the searchable list v1 has) and pagination (its starting point is in `docs/spec/odk-component-tokens.md`, "Adding a component"; nothing in ODK needs it yet). They are added through a change to this spec.

A component added to this spec gets its component tokens before it draws, built from the existing groups and mappings rather than colours or lengths of its own (`docs/spec/odk-component-tokens.md`, "Adding a component"). That section also says where the progress bar, skeleton, pagination and navigation item start from; the radio group and the number input were added that way.
