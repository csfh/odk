# ODK component tokens, version 1

Normative for the component layer of ADR 0040, accepted on 2026-10-22. It must not change a decision recorded in an ADR; where the two disagree, the ADR wins.

Built in `odk-theme` (`component/`) and published through `odk-ui`'s store on 2026-10-22. `Button` draws these styles since the same day (step 7 of `docs/plan.md`, "Design language migration"), and so do the buttons inside `Alert`, `Toast`, `Dialog`, `Sheet` and `InputGroup`. The form controls draw them since 2026-10-23 (step 8, the form controls' brief): `Input`, `Textarea`, `InputGroup`, `SearchInput`, `PasswordInput`, `NumberInput`, `Select` and its list, `Checkbox`, `Switch`, `RadioGroup` and `FormField`; so do the list that `DropdownMenu` and `ContextMenu` share with `Select`, and `Command`'s field and rows. `Text` and `Heading` draw them since the same day. The navigation and feedback components draw them since 2026-10-23 too (the brief for navigation and feedback): `Tabs`, `ToggleGroup`, `Breadcrumb` and the link it shares with `DropdownMenu`, `Badge`, `Alert`, `Spinner`, `Separator`, and, new through a change to `docs/spec/odk-ui-components.md`, `NavList`, `StatusIndicator`, `Progress` and `Skeleton`. The overlays draw them since 2026-10-23 too (Phase 9, `docs/spec/odk-overlays.md`): `Dialog`, `Sheet` and `Command`'s panel, `Popover` (new), `HoverCard`, `Tooltip`, `Toast`, `Kbd`, and the trigger rings of `HoverCard`, `Popover` and `ContextMenu` (`trigger_focus`). Every component draws the component layer, and `root()`, `Space` and the `Shell` read the semantic and foundation layers, since the legacy `Tokens` were removed (ADR 0047). Values marked *provisional* are ODK's own choice and still need the user's confirmation.

## Architecture

```text
colors.toml ─► Palette ─► resolver ─► SemanticColours ──┐
                                                         ├─► ComponentTokens ─► components
FoundationConfig ─────────────────► FoundationTokens ───┘
```

The component layer adds no colour and no length of its own. Two rules make that structural rather than a habit:

- **Colours come only from the semantic layer.** A component's colour is a `ColourRef` (`odk_theme::semantic`): a surface, text or border token, one part of one treatment (`roles.<role>.<emphasis>.<state>.<part>`), a role's focus colour, or `none`. Which reference each component uses, in each variant and state, is fixed in `component/`'s code; no reference can name a colour the resolver did not make and check. So no component colour depends on density or corner radius, and a monochrome theme's components are grey.
- **Geometry comes only from the foundation.** Every length is a foundation token or follows from them (see Geometry), so no length depends on the theme.

`ComponentTokens` holds one `Arc<SemanticColours>`, one `Arc<FoundationTokens>` and the generation of the theme snapshot the colours came from. Building it copies two pointers. A lookup such as `button(...)` resolves a few references and adds a few lengths: no colour maths, no OKLCH, no file and no allocation. `component::resolve(&FoundationTokens, &SemanticColours, &ResolveOptions)` is the pure function for tests and tools: it validates the colours (see Validation) and joins the two.

## States

`States` holds one flag for each state a control can be in: `hovered`, `pressed`, `focused`, `disabled`, `loading`, `selected`, `highlighted`, `checked`, `indeterminate`, `invalid`, `read_only` and `expanded`. Any combination can be described; a component reads only the flags it has. There is no enumeration of combinations. Precedence is decided in `States` and in each component's one mapping from a state to colours, nowhere else:

1. **Disabled** wins over everything. The pointer changes nothing, the focus ring is not drawn, and the colours are the semantic `disabled` treatment, never a lowered opacity of another. A disabled control that is selected or checked keeps showing it (a check box keeps its mark, a selected tab its indicator), in disabled colours.
2. **Pressed** wins over hover.
3. **Focus** is not a state of the colours. The focus ring is drawn over whatever the colours are, with hover or press, and changes none of them.
4. **Invalid** keeps its edge (`border.danger`) under the pointer and in focus, and its ring is the danger role's focus colour.
5. **Selected and checked** stay visible under hover and press: a selected tab keeps its indicator, a ticked check box its mark.
6. **Loading** keeps the resting colours and the focus ring and ignores the pointer, so a busy control neither looks pressed nor can be pressed twice. It never changes a control's size.
7. **Expanded** keeps a field's focused edge on a select whose list is open, and makes its indicator primary text. It never changes the role. A focused or expanded field has its focused edge in all three pointer treatments, so hovering or pressing it never takes the edge away.
8. **Highlighted** (where the keyboard is in a list) is drawn apart from hover and from selected (see Menus).

A style gives its colours as `Interactive<Treatment> { default, hover, pressed }`, the three pointer states GPUI applies itself through `.hover()` and `.active()`, with the static flags already applied: all three are the disabled treatment while disabled and the resting one while loading, so wiring all three is always safe. `current` is the treatment for the flags given, `responds` says whether the pointer gets hover and pressed and a press acts, and `focus_visible` whether to draw `focus`.

## Geometry

| Token | Derived from |
|---|---|
| Control height | the density's `height`: 28, 34, 40 |
| Horizontal padding | the density's `padding_x`: 10, 12, 16 |
| Vertical padding | what is left of the height once one line of the control's type role and both borders are taken, halved, so the text is centred at every density: a button (type `button`, 18 px lines) has 4, 7 and 10 |
| Content height | one line of the type role |
| Icon size, gap | the density's `icon_size` (14, 16, 18) and `gap` |
| Corner radius | `radius.default` for controls, `radius.overlay` for overlays, `radius.small` for marks, `radius.none` for items inside another's frame |
| Border width | `border_width`: 1 |
| Focus ring | `focus_width` (2) wide, `focus_offset` (2, *provisional*) outside the control |
| Inset | `thumb_gap` (2, *provisional*): between a control's inner edge and a part nested inside it, a switch's thumb, a radio's dot, a field action |

`ControlGeometry { height, padding_x, padding_y, gap, icon_size, line_height, radius, border_width, type_role }` holds a control's box; `SurfaceGeometry` an overlay's frame; `CheckGeometry`, `RadioGeometry` and `SwitchGeometry` the three marks.

- **Not everything is a control's height.** Check boxes are squares of the density's icon size, with the application's corner radius like every control (the form controls' brief, 2026-10-23; until then `radius.small`), their tick or dash the box less its borders. Radios are circles of the same size, the chosen one's dot inset by `thumb_gap` from the inside of the border: 8, 10 and 12. A textarea's height is `rows × line height + 2 × (padding + border)`, so one row is a field's height. Badges and keys are one line of their type role and their borders high; a badge has the application's corners (the brief for navigation and feedback, 2026-10-23; until then `radius.small`), a key square ones. Overlays, alerts and toasts follow layout spacing, not density.
- **Navigation follows density; feedback mostly does not.** Tabs, segments and sidebar rows are a control's height (28, 34, 40) with its padding, icon size and gap. A status dot is the grid's 8 px step (`space.2`) and a progress bar 4 px thick (`space.1`) at every density (*provisional*); a skeleton line is one line of body text, its bar the text's size; a spinner standing alone is the density's icon size.
- **A field action** (a button inside a field) is the field's height less its borders and `thumb_gap` on each side: 22, 28 and 34. It has the padding of the density one step tighter, whose controls are about its height (*provisional*), the field's icon size, and an icon alone is a square. So a field with a button keeps its height, and the button's ring, 4 px outside it, lies over the field's edge.
- **A switch** has a track as high as the density's icon (14, 16, 18), a thumb that fills it less the border and `thumb_gap` (2, *provisional*) on each side (8, 10, 12), and room to move its own diameter and one more gap: 24, 28 and 32 wide.
- **Icon-only buttons** are squares as wide as they are high, the icon centred: 28, 34 and 40 px targets, above WCAG 2.5.8's 24. A text button has no minimum width.
- **Shapes.** Every rectangle takes the application's corner radius: 0 with `CornerRadius::Square`, exactly 2 with `Subtle` (ADR 0042). A part whose roundness says what it is, a switch's track and thumb, a spinner, a radio and a status indicator's dot, is a `Shape::Pill` or `Shape::Circle`, not a larger radius (the user's answers of 2026-10-22 and the brief for navigation and feedback of 2026-10-23, `docs/spec/odk-foundation-tokens.md`). Nothing else is round. `Spinner` draws braille characters, so its circle applies only once it is drawn as a ring. A progress bar is a rectangle with the application's corners, so at `Subtle` its 4 px ends are rounded by the 2 px radius, and square at `Square`.
- **`Size`.** Every lookup takes a `Density`: the control's own or the application's (`ComponentTokens::density(own)`). A control's `Size` is a step from the density it would otherwise take (*provisional*, decided with the button on 2026-10-22 and still to be confirmed by the user): `Md` is that density, `Sm` one step tighter and `Lg` one step roomier, stopping at compact and comfortable (`Size::density`, `Density::smaller`, `Density::larger`). So a size never invents a height, an application's density still moves a control with a size, and a density the control was given wins over both. Under the default density, `Sm`, `Md` and `Lg` are 28, 34 and 40.

## Inventory

Each group belongs to the components named, which are its only consumers. "Text" is `text.<token>`, "role" a role's treatment, "focus" a role's focus colour.

| Lookup | Consumers | Colours | Geometry |
|---|---|---|---|
| `button(appearance, density, layout, states)` | `Button`; the buttons in `Alert`, `Toast`, `Dialog`, `Sheet`, `InputGroup` | the role's treatment at the emphasis (see Buttons); an outline's edge `border.strong`/`border.hover` for neutral, the role's focus colour otherwise; ring `border.focus` for neutral, the role's focus colour otherwise | control box, type `button`; `ButtonLayout` `Text`, `LeadingIcon`, `TrailingIcon` or `IconOnly`; `spinner_size` the icon size |
| `field(density, states)` | `Input`, `SearchInput`, `PasswordInput`, `NumberInput`, `InputGroup`, `Command`'s field | `surface.recessed`, text primary; edge `border.strong`, `border.hover` under the pointer, `border.focus` focused (under the pointer too), `border.danger` when invalid (under the pointer and in focus), `border.default` when read-only; disabled the neutral muted disabled treatment; placeholder `text.placeholder`; selection `surface.selection` and `text.on_selection`; caret text primary; required mark and invalid mark (`!`, `FIELD_INVALID_MARK`, drawn as the warning icon, ADR 0083) the danger role's text colour; icon text tertiary; addon text tertiary; ring `border.focus` or `border.danger` | control box, type `body`; a leading or trailing icon is `icon_size`, `gap` from the text |
| `field_action(appearance, density, layout, states)` | the buttons inside fields: a search's clear button, a password's show button, a number's steppers, an `InputGroup`'s action | the button's for `appearance` (a ghost button, or the application's for an input group) | see Geometry |
| `textarea(density, rows, states)` | `Textarea` | the field's | the field's, `rows` lines high |
| `select(density, states)` | `Select`'s box | the field's; indicator the field's icon colour: text tertiary, primary when expanded | the field's |
| `menu()` | `DropdownMenu`, `ContextMenu`, `Select`'s list (one surface, `menu_list.rs`) | overlay surface (`surface.overlay`, text primary, `border.default`); separator `border.subtle`; heading text tertiary | padding 4, separator margin 4, `radius.overlay` |
| `menu_item(density, states, danger)` | the items of the above and `Command`'s list | see Menus | control height, padding 8, gap `menu_item_gap`, no border |
| `select_item(density, states)` | the options of a `Select`'s list | a menu item's | a menu item's, with its padding the field's inset less the menu's border and padding, so an option's label starts where the box's value does at every density |
| `checkbox(density, states)` | `Checkbox` | clear: `surface.recessed`, edge `border.strong`/`border.hover`; ticked or indeterminate: the accent's solid fill, its label as the mark, the accent's focus colour as the edge; invalid edge `border.danger` and the invalid mark after the label in the danger role's text colour; disabled the neutral muted or accent solid disabled fill with a `text.disabled` mark | square of the icon size, `radius.default` |
| `radio(density, states)` | `RadioGroup`'s options | the check box's mapping, a chosen radio being a ticked box: the chosen one the accent's solid fill with a dot of its label colour; a wrong group's invalid mark once, after its first option | circle of the icon size, dot inset by `thumb_gap` |
| `switch(density, states)` | `Switch` | off: neutral muted fill, edge `border.strong`/`border.hover`, thumb its text colour; on: the accent's solid fill, the accent's focus colour as the edge, its label as the thumb; disabled the disabled fills with a `text.disabled` thumb | see Geometry; pill track, round thumb |
| `tab(look, density, states)` | `Tabs` (`NavLook::Text`) | text secondary, primary when hovered, pressed or selected, over `surface.hover`/`surface.pressed`; `NavLook::Surface` adds the selected fill when selected (see Selection); indicator the accent's focus colour, `focus_width` thick, `text.disabled` on a disabled selected tab; baseline `border.subtle`; an icon and a counter in the label's colour | control height, no border, `radius.none`; icon the density's, `gap` from the label; counter type `small` |
| `segmented(density)`, `segment(density, states)` | `ToggleGroup` | frame `border.default`; a segment text primary, hover and pressed tints, the selected fill when on (see Selection), which a disabled segment keeps in its disabled treatment | frame a control box; a segment its height less the borders, an icon alone a square |
| `nav_item(density, states)`, `nav_list(density)` | `NavList` | a row: the tab's mapping in its surface look, the current page (`selected`) the selected fill with the accent's focus colour as an indicator along its leading edge, `focus_width` wide; group headings text tertiary | a row a control's box with no border and `radius.default`; headings type `small`, inset by the rows' padding, 4 above their first row and 16 after the last row of the group before |
| `nav_link(states)` | `Breadcrumb`'s items, `DropdownMenu`'s link trigger (`link.rs`) | text secondary, primary and underlined at rest, under the pointer and while pressed; the current page (`selected`) text primary, not a link and not underlined; a step that is not a link text secondary; separator text tertiary | type `body`, gap 8; underline `border_width` thick |
| `popover()` | `Popover`, `HoverCard` | overlay surface, text secondary for descriptions | padding `panel_padding`, gap 8, title `body_strong` |
| `tooltip()` | `Tooltip` | overlay surface | padding 8 × 4, type `small` |
| `dialog()`, `sheet()` | `Dialog`, `Sheet` | overlay surface; backdrop `surface.scrim` | padding `dialog_padding`, gap `form_gap`, title `heading`; a sheet is against a window edge, so its radius is 0 |
| `trigger_focus()` | the ring around a `HoverCard`'s, `Popover`'s or `ContextMenu`'s trigger when the trigger is not a control with a ring of its own | `border.focus`, `focus_width` wide, `focus_offset` outside | |
| `toast(role)` | `Toast` | overlay surface; mark the role's text colour | padding `panel_padding`, gap 8, title `body_strong` |
| `command()` | `Command` | the dialog's, with `field` and `menu_item`; the empty line text tertiary | the dialog's |
| `badge(appearance)` | `Badge` | the role's treatment at the emphasis, at rest; an outline as for buttons; an icon or status mark in the foreground | one line of `small` and its borders, padding 8, `radius.default`; an icon the text's size, 4 from the label |
| `alert(role, emphasis)` | `Alert` | solid or muted: the treatment, every text its foreground; subtle: no fill, title text primary, description secondary, edge the role's focus colour, mark the role's text colour | padding `panel_padding`, gap 12, `radius.default`; `compact`: padding 12 × 8, gap 8; the mark the density's icon size on the title's line |
| `status(role)` | the marks in `Alert`, `Badge` (`marked`) and `Toast` | the role's text colour, and a mark (`status_mark`): `i`, `✓`, `!`, `×`, which components draw as the status icons (`Icon::status`, ADR 0083) | |
| `status_dot(role)` | `StatusIndicator` | the dot the role's text colour, filled; the neutral role (inactive) a hollow ring in text tertiary; the label text secondary | a circle 8 across (`space.2`), the ring `border_width`; label type `body`, 8 from the dot |
| `progress(role, states)` | `Progress` | the track the role's muted fill, the bar the role's focus colour (a `Mark` against the track); label text primary, value text secondary; disabled: the neutral muted disabled fill, a `text.disabled` bar and text | 4 thick (`space.1`), `radius.default`; label and value type `small`, `field_gap` (6) above the bar |
| `skeleton()` | `Skeleton` | the neutral muted fill: decoration, held to no contrast | a line one line of `body` (20) with a bar the text's size (13); `radius.default`; still |
| `spinner(density)` | `Spinner` | text secondary standing alone; inside a control, the control's foreground | the icon size; one turn per `motion.cycle`; still under reduced motion |
| `separator()` | `Separator` | `border.subtle`: decoration, never a control's edge | `border_width` thick; `inset` a control's padding at the application's density |
| `resize_handle(states)` | `Resizable`'s handles (ADR 0069) | `border.subtle` at rest, as a separator; `border.focus` while hovered or dragged, held to `ui_contrast` as a mark; the ring `border.focus` | takes `border_width` of room; drawn `focus_width` thick while active, over the panels; a pointer target `target_min` across, centred, taking no room |
| `table(density)`, `table_row(states)`, `table_header(sortable, states)` | `DataTable` (ADR 0070) | rows: no fill, `surface.hover` and `surface.pressed` under the pointer, the selected fill (a segment's, so a hovered row never passes for the selected one), text primary; headers: secondary text, primary under the pointer when they sort (a link's mapping), secondary when not; the rule under the header `border.default`; the empty text secondary; the ring `border.focus` | a row is a control's height at the density, with its padding and gap; headers in the `small` type role; the rule `border_width` |
| `chart()` | `Chart`'s inks (ADR 0071) | six series, the Roles' focus colours in a fixed order (accent, info, success, warning, danger, neutral), held to `ui_contrast` as marks, and their muted fills as tints; a ramp of five: the neutral muted fill, the accent's muted fill at rest and under hover, its solid fill and its focus colour; text primary and secondary; grid `border.subtle`, axis `border.default`; `surface.canvas` for gaps and halos; the ring `border.focus` | strokes `border_width`, `focus_width` and twice that; heights `s8`, twice `s12` and four times `s12` (ADR 0075); text in the `small` type role |
| `image()` | `Image` (ADR 0072) | the placeholder is the skeleton's fill (the neutral muted tint) with a tertiary `×`, held to `text_contrast` | the application's corner radius; an avatar `s12` across, a circle (`Shape::Circle`); the mark in the `small` type role |
| `item(density)` | `Item` | title text primary, description text secondary, as a form field's label and description | title `body_strong`, description `small`; above and below `s1`, `s2` or `s3` by density, nothing at its sides; `s1` between its lines, `s3` between its words and each action, `s2` above its footer |
| `kbd()` | `Kbd` | `surface.sunken`, text primary, `border.default` | one line of `mono` and its borders, padding 4, `radius.small` |
| `form_field(states)` | `FormField` | label text primary (disabled when the control is), description text secondary, required mark the danger role's text colour, optional mark text tertiary | label `body_strong`, description and message `small`; `field_gap` (6) between label, description, control and message; `form_gap` (16) between fields |
| `text(secondary)` | `Text` | text primary, or text secondary for `muted()` text | type `body`; no box |
| `text_mono(secondary)` | `Text::mono()` | the colours of `text(secondary)` | type `mono`; no box |
| `heading(level)` | `Heading`, and through it the titles of `Dialog` and `Sheet` | text primary | type by level (`heading_role`): 1 (and 0) `title`, 2 `heading`, 3 and higher `subtitle`; no box |
| `form_message(role)` | `FormField`'s message | an error (`Danger`), a warning or a confirmation (`Success`) in the role's text colour with its status mark; helper text (`Neutral`) text secondary, no mark | type `small`, the mark its size |

**Not applicable.** v1 has no pagination (`docs/spec/odk-ui-components.md`), so there are no tokens for it: a group without a consumer is not built. It is added with its component, through a change to that spec, and its group is built from the existing ones (see Adding a component). The radio group and the number input were added this way on 2026-10-23: a radio is the check box's mapping on a circle, and a number input a field with field actions. So were the progress bar, the skeleton, the navigation item (`NavList`) and the status indicator, the same day, from the starting points below. Search and password inputs are fields too (a leading icon and a clear button; `masked` and a show button). No combobox is built: `Command` is ODK's only searchable list, and a combobox would be a field over a menu list, both of which exist.

## Buttons

`ButtonVariant` (`#[non_exhaustive]`) has the eight variants of ADR 0040. Each is a `ButtonAppearance { role, emphasis, outline, link }`:

| Variant | Role | Emphasis | |
|---|---|---|---|
| `Primary` | Accent | Solid | |
| `Secondary` (default) | Neutral | Muted | |
| `Outline` | Neutral | Subtle | with an edge |
| `Ghost` | Neutral | Subtle | |
| `Destructive` | Danger | Solid | |
| `Success` | Success | Solid | |
| `Warning` | Warning | Solid | |
| `Link` | Accent | Subtle | a link: underlined, no fill in any state, `text.link` |

Any role goes with any emphasis: `ButtonVariant::Destructive.appearance().with_emphasis(Emphasis::Muted)`, or `ButtonAppearance::new(Role::Success, Emphasis::Subtle).outlined()`. Every combination is validated, not only the eight. A link has no fill in any state, so its emphasis changes nothing; that is the only combination that is not drawn differently. `ButtonAppearance::name` is the name diagnostics list an appearance under (`"danger muted"`, `"neutral subtle outline"`, `"accent link"`); the eight variants are listed under their own names as well. `Button::variant` takes `impl Into<ButtonVariant>`, and `Button::emphasis` changes the variant's emphasis and keeps its role. The legacy `Variant` and its `From` impl are gone (ADR 0047); `Primary`, `Secondary` and `Ghost` keep their names as `ButtonVariant`s.

## Selection

A selected segment and a sidebar's current page have the **selected fill**: the neutral muted treatment as it is under the pointer, 5 JND from the canvas, as a menu's highlight is (below). An unselected item under the pointer is `surface.hover`, 2 JND, and the resting neutral muted fill is 3 JND, one JND from that hover; with the resting fill, hovering the segment beside the selected one looked like selecting it (found by `checklist/toggle_group.rs` in Tokyo Night, and true of every theme, since the semantic layer steps in JNDs). So the selected fill stays 5 JND under the pointer, pressed it is the muted pressed fill, and at least 2 JND lie between selected and hovered in every theme (`crates/odk-theme/tests/components.rs`). Tabs, which have no fill, show selection by their indicator and primary text.

## Menus

Hovered, highlighted and selected are three different things in a list:

| | Fill | Text |
|---|---|---|
| At rest | none | primary |
| Hovered | `surface.hover` (2 JND) | primary |
| Pressed | `surface.pressed` (4 JND) | primary |
| Highlighted | the neutral muted fill under hover (5 JND) | its foreground |
| Highlighted and pressed | the neutral muted fill pressed (7 JND) | its foreground |
| Selected | as above, with a check mark in the text colour | |
| Disabled | none | `text.disabled` |

A danger item uses the danger role's subtle treatments, and its muted ones when highlighted. A hint is text tertiary, and the item's own text colour on a highlight or in a danger item; it is drawn as text (not keycaps) and follows the pointer through its row's group. A heading over a group of items is text tertiary and is not an item. The theme's `selection` is not used for the highlight: in many themes it is within a JND of the overlay surface (Tokyo Night's `#292e42` is 0.008 from the hover tint on its overlay, Vantablack's is the overlay itself), so it would vanish.

## Validation

`component::validate(&SemanticColours, &ResolveOptions)` checks every colour pair every component draws, from the same mappings the lookups use, so what is checked is what is drawn. Every button appearance (the eight variants, then every role at every emphasis, with and without an edge, and every role as a link) and every other component's variants and states are checked.

| Measure | Pair | Minimum |
|---|---|---|
| `Text` | the foreground on the background | `text_contrast` (4.5) |
| `Mark` | a mark on the background: a switch's thumb, a tab's indicator, a select's indicator, a spinner, a focus ring | `ui_contrast` (3) |
| `Edge` | the border against the surface around it: a field's, a check box's, a switch's, an outline | `ui_contrast` (3) |

- **Measured as drawn.** A translucent background is composited over each surface it may sit on first (`drawn_contrast`), and the least result counts. Controls are measured over every opaque surface (`canvas`, `recessed`, `sunken`, `raised`, `overlay`); menu items, overlay text and marks over `overlay`, where they are drawn. A focus ring is measured against the surface around the control, since it is drawn outside it.
- **Not held to a minimum:** disabled styles (WCAG exempts inactive controls; disabled text stays between 2:1 and 3:1 against the canvas, `docs/spec/odk-semantic-tokens.md`), decorative borders (`border.subtle`, `border.default`, a read-only field's edge, separators) and the scrim.
- **Every pair is one the semantic layer already guarantees.** The mappings use only combinations the resolver checks (a role's text in its own fills, text tokens under `surface.hover` and `surface.pressed`), so validation can only fail where the semantic layer was bypassed. It still runs, independently.
- **A failure is a failed reload.** `ThemeManager` validates the components with every resolution, and the emergency palette and `ThemeSnapshot::from_toml` do too, so every snapshot's semantic colours have passed. A theme whose component colours fail is `Error::Component`, and ADR 0041's fallbacks apply: nothing partial is published.

## Diagnostics

`ComponentTokens::diagnostics(&ResolveOptions) -> Vec<StyleDiagnostic>` lists every style: component, variant, state, density (`None` where the geometry does not follow it), role and emphasis, the final background, foreground and border with the `ColourRef` each came from, height, shape or radius, type role, and each measure's contrast, the minimum and the surface it is least on. `Display` writes one line per style:

```
button primary hover, default (accent solid): background #87adff (roles.accent.solid.hover.background), foreground #1a1b26 (roles.accent.solid.hover.foreground), border #87adff (roles.accent.solid.hover.border); height 34, radius 2, type button; text 7.69:1 (needs 4.5:1, least on surface.canvas)
```

It is built on request, for a tool such as the gallery, never while drawing; the options are the snapshot's (`theme_snapshot().resolution.options`).

## Publishing

- **The store.** `odk-ui` keeps the component tokens in the same slot as the theme snapshot, written only by `publish`, so `odk::ui::components()` always has the generation of `odk::ui::theme_snapshot()` and a view never sees the colours of one theme with another.
- **A theme change** (a reload or new colour options) publishes a snapshot whose semantic colours and component checks were made together on the background thread. The component tokens take its semantic colours and keep the foundation's snapshot; nothing is rebuilt but two pointers.
- **A foundation change** (`odk::ui::configure`: density, corner radius, reduced motion, fonts) builds the foundation tokens once and republishes the same snapshot: the component tokens keep the very same semantic colours (`Arc::ptr_eq`), and no colour is resolved, looked at or checked.
- **Every window** redraws on the next frame after either.

## In GPUI

`odk::ui::StyledExt` puts a token on an element, so a component never converts a colour or a length itself: `treatment(&Treatment)` sets the background, the text colour and the border's colour, and `control_geometry(&ControlGeometry)` the height, padding, corner radius, border width, gap and type role. Both work inside `.hover()` and `.active()`.

Inside `odk-ui`, `interaction.rs` holds what every component on component tokens shares, so none writes `.hover()`, `.active()` or a ring of its own (a field's box adds these through `input::field_box`, and a check box, a switch and a radio their label row through `toggle::labelled`):

- `pointer_states(el, &s.colours, s.responds, pointer)` puts the resting treatment on, and the hover and pressed treatments when the control responds. GPUI 0.2.2 keeps an element's text style as one value: a `.hover()` or `.active()` refinement that sets only the text colour replaces the whole of it, so the text loses its type role, changing size while pressed and being painted at the wrong size while hovered (`docs/checks.md`). Each refinement therefore carries the element's own text style with its state's colour, and `pointer_states` is called after everything else that sets the text style. `StyledExt::treatment` inside `.hover()` has the same trap.
- `focus_ring(&s.focus, radius, border)` is the ring: an absolutely positioned layer `focus_offset` outside the control, `focus_width` wide, with the control's own corner radius rather than a rounder one, no fill and no listeners. It takes no space, so focusing moves nothing (checklist item 32). It is drawn while `s.focus_visible`, from the control's focus handle at render: on any focus, since GPUI 0.2.2 cannot tell keyboard focus from a click's, as before (`docs/plan.md`, finding 6).
- `progress(id, on, MotionDuration, …)` is a transition that the component's own state drives, on the foundation's durations and easing, reversible without a jump and instant with reduced motion. A `Button`'s spinner fades with it, and a tab's indicator between tabs. `tween(id, target, MotionDuration, …)` is the same for a value, a progress bar's.
- `RingTarget` paints a composite control's ring from outside the focused item: the item takes `probe()`, which records its bounds as it is laid out, and the control's outermost element `ring(...)` as its last child, which paints the same ring `focus_ring` draws around those bounds after everything else. GPUI 0.2.2 lays out and prepaints a whole window before it paints any of it, so the ring is where the item is on the same frame. A ring drawn as the item's own child was covered by the next item's fill (a selected segment, the next sidebar row) and cut by a row that clips (a tab list scrolls, a breadcrumb truncates; in GPUI 0.2.2 an element that hides or scrolls either axis clips both). Tabs, segments, sidebar rows and breadcrumb links use it. `link_states` is `pointer_states` for a link, with its underline under the pointer.

Hover and press change colour at once. GPUI applies `.hover()` and `.active()` instantly and has no style transitions; following the pointer in ODK instead would give up GPUI's release of a press outside the control, and its hover listener reports no hover while a button is held, so a fade built on it would flicker on every press.

```rust
let c = odk::ui::components();
let s = c.button(appearance, density, layout, States::NONE.focused(focused).disabled(d).loading(l));
let mut face = div().id(id).relative().control_geometry(&s.geometry);
face = pointer_states(face, &s.colours, s.responds, &Pointer::Own);
if s.focus_visible {
    face = face.child(focus_ring(&s.focus, s.geometry.radius, s.geometry.border_width));
}
```

`Button` reads `components()` once per render and looks its style up once (`Button::style`); that lookup is the only place it gets a colour or a length.

## Rules for components

These bind a component from the change that moves it onto component tokens (step 7 on). They are item 34 of `docs/spec/component-checklist.md`, which a source scan checks for every component that has moved.

- Take every colour and length from a component token when one exists. Never name a palette key, read `colors.toml`, or compute a colour, an OKLCH value or a contrast in `render`.
- Do not hardcode a colour, a control height or a corner radius, and do not define a radius of your own; a round part is a `Shape`.
- Do not decide state precedence in a component: ask with `States` and draw `colours`, `current`, `responds` and `focus_visible` as given.
- Do not build a colour resolver, a theme cache or token tables of your own; read `odk::ui::components()` inside `render`.
- A component without a token group gets one here, with its consumer named, before it draws, built as Adding a component says.

## Adding a component

A new component, or a part no group covers, gets its tokens in this layer before it draws, and they are built from what exists, in this order:

1. **Reuse a group.** A component that is a variant of one with a group takes that group's lookup, with a new variant or flag if it needs one, not a lookup of its own.
2. **Reuse a mapping.** Otherwise its new lookup calls the existing colour mappings (`controls::button`, `field`, `checkbox` and `switch`; `navigation::tab`, `segment` and `link`; `overlay::item`; `feedback::badge` and `alert`) for its states, and the existing geometry helpers (`geometry::control`, `item`, `inline`, `surface`, `check` and `switch`) for its box.
3. **Write a mapping** only when none fits, from `ColourRef`s, `States` and foundation tokens alone. It adds no colour, length, radius or threshold of its own. A colour or length that does not exist yet is a change to `docs/spec/odk-semantic-tokens.md` or `docs/spec/odk-foundation-tokens.md` first, never a value here.

In the same change, the component gets its row in Inventory, naming its consumer, and its entries in its module's `catalogue`, from the same mapping its lookup uses. A new module's `catalogue` is called from `catalogue::catalogue()`, which lists the modules by hand: entries nothing collects are never checked. `validate` and `diagnostics` then cover it, and so do the theme tests in `crates/odk-theme/tests/components.rs`, which run them over every theme.

Where each component v1 left out starts from (Not applicable). These are starting points, not decisions: each is settled with its component, and validation decides whether it holds in every theme.

| Component | Starts from |
|---|---|
| Progress bar (built) | was: the track the neutral muted fill, the bar the accent's solid fill. Built instead with the role's muted fill as the track and its focus colour as the bar: the semantic layer guarantees a role's focus colour at `ui_contrast` against every muted fill of that role over every surface, and nothing else, while a solid fill may be under 3:1 against the canvas (`docs/spec/odk-semantic-tokens.md`), so the first pairing could have refused a theme. In a monochrome theme both are greys. Only determinate: work of unknown length has a `Spinner` |
| Skeleton (built) | the neutral muted fill, decorative like a separator; the geometry of what it stands in for; it stands still, so it needs no loop |
| Pagination | `nav_link` for the pages, the current one `selected`, or `segmented` and `segment` for a row of them; previous and next are `button` at `ButtonLayout::IconOnly` |
| Navigation item (built, `nav_item`) | `tab`'s mapping with `NavLook::Surface` (text secondary; the selected fill and the accent indicator when selected), on `item`'s geometry with the application's corners, since a row stands on its own rather than inside another's frame |
| Status indicator (built, `status_dot`) | the role's text colour from `status`, on a circle; the neutral role a hollow ring, so on and off differ in shape |

## Checked

On 2026-10-22, under the default options:

- **Every theme passes every check:** the fixtures (Tokyo Night, Nord, Catppuccin Mocha and Latte, Vantablack, the incomplete custom theme), the emergency palette, all 22 stock themes, and all 23 of the user's own (reported, not asserted) (`crates/odk-theme/tests/components.rs`).
- **Monochrome stays monochrome:** every colour of every style in Vantablack, White and the emergency palette is grey.
- **No new colour:** every colour of every style is the semantic colour its reference names.
- **Nothing visible changed when the layer was built:** the gallery's headless screenshot was byte-for-byte the one before it, until `Button` moved onto it.

On 2026-10-22, with `Button` drawing them (`crates/odk-ui/src/checklist/button.rs`, and `tools/headless-shot` on the gallery):

- **A button draws its lookup.** In Tokyo Night, Nord, Catppuccin Mocha and Latte and Vantablack, every variant and an emphasis of each status role draw exactly `components().button(...)` for their arguments, and a Vantablack button is grey.
- **Geometry.** Buttons are 28, 34 and 40 high at the three densities, icon-only buttons are squares, icons are the density's icon size and gap from the label and sit on its line, corners are exactly 2 or 0, and the ring lies 4 px outside the button.
- **Seen.** The gallery's Button section, headless, in the user's theme, Catppuccin Latte and Vantablack, with the ring after Tab and with square corners chosen by keyboard. Radii of 0.8, 1, 1.5, 2, 3 and 4 px at scales 1 and 2, which chose 2 px (ADR 0042).

On 2026-10-23, with the form controls drawing them (`crates/odk-ui/src/checklist/forms.rs` and each control's module, and `tools/headless-shot` on the gallery's Forms section):

- **The controls draw their lookups.** In every fixture, an input at rest, invalid, read-only and disabled, a textarea, a select's box, a ticked and a mixed check box, a switch that is on, a radio of each density and the select's list items draw exactly their `components()` lookups, and in Vantablack all of them are grey, a wrong field keeping its `!`.
- **One height system.** A `Button`, `Input`, `Select`, `InputGroup`, `SearchInput`, `PasswordInput` and `NumberInput` in one row are exactly 28, 34 and 40 high at the three densities and share one centre; the field actions inside them are lower than the field and on its centre. Corners are exactly 0 or 2 on fields, selects and check boxes; radios are circles and switches pills.
- **Seen.** The Forms section, headless, in the user's theme, Catppuccin Latte and Vantablack, with the worst-case content, a field's ring after Tab and a select's open list.

On 2026-10-23, with the navigation and feedback components drawing them (each component's module in `crates/odk-ui/src/checklist/` and `navigation.rs`, and `tools/headless-shot` on the gallery's Navigation and Feedback sections):

- **They draw their lookups.** In every fixture, every badge Role at every emphasis with and without an edge, every alert tone at every emphasis, the segments on, off and disabled, the progress bar in each role, paused and disabled, every status, the skeleton and the spinner draw exactly their `components()` lookups; in Vantablack every colour is grey and a status still reads by its mark, its label or its shape.
- **Geometry.** Tabs, segmented controls and sidebar rows are 28, 34 and 40 high at the three densities and their colours do not move with it; badges, alerts, progress bars, skeletons and sidebar rows take 0 or exactly 2 px corners; a separator is one border width; a status dot an 8 px circle.
- **Live themes.** A theme change recolours them and keeps the selected tab, the chosen segment, the current page, the progress, where focus is, the density and the corners.
- **Seen.** Both sections, headless, in the user's theme (`dazzle-dusk`), a private Catppuccin Latte and a private Vantablack, and with the worst-case content: the ring on a focused tab apart from the selected one and on a sidebar row over its neighbours, painted from outside.

Not checked: hover and press on screen (a headless display has no pointer) (`docs/checks.md`).

## Open questions

1. The provisional values: `focus_offset` and `thumb_gap` of 2 px (now also a field action's inset and a radio's dot), menu items as high as a control (now seen in every menu and a select's list), a field action's padding from the density one step tighter, the badge's and key's padding, a status dot's 8 px and a progress bar's 4 px, a compact alert's padding, and a text skeleton's last line at 60% of its column.
2. Whether a control's `Size` as a step from its density (Geometry) is what the user wants; it is *provisional*.
3. Still open: whether `Tooltip` should be the inverse surface (`text.primary` fill, `text.inverse` text), which the semantic layer offers, rather than the overlay surface every other overlay shares.
4. Whether a wrong value is marked by `!` (`FIELD_INVALID_MARK`), at a field's end and after a check box's label or a radio group's first option, rather than the danger role's `×`, which a field already uses for its clear button. Without a mark, a monochrome theme's grey danger edge would be the only sign.
