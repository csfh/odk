# ODK text input

Normative detail for ADR 0038. It governs `Input` first, then `Textarea`, `InputGroup`, `SearchInput`, `PasswordInput`, `NumberInput` and `Command`, which share its engine. Every component built on it still passes `component-checklist.md`. An ADR wins over this spec.

Status: all are built. `Input` is `crates/odk-ui/src/input.rs`, with the model in `editor.rs` and the geometry in `text_layout.rs`; `Textarea` (`textarea.rs`) is `Input` with rows; `SearchInput`, `PasswordInput` and `NumberInput` (`search_input.rs`, `password_input.rs`, `number_input.rs`, with the numbers in `number.rs`) are `Input` with what each adds; `InputGroup` (`input_group.rs`) and `Command` (`command.rs`) are built on `Input`. Since 2026-10-23 all draw the component tokens of ADR 0040 (`docs/spec/odk-component-tokens.md`), but `Command`'s panel.

## The model

A pure Rust type in `odk-ui`'s `editor` module (`editor.rs`), with no GPUI types.

- **Text** is a `String`. Offsets inside the model are UTF-8 bytes and always on grapheme-cluster boundaries. UTF-16 appears only in the `EntityInputHandler` implementation, which converts in both directions.
- **Selection** is an `anchor` and a `head`; a caret is a selection where they are equal. Movement with Shift moves the head and keeps the anchor.
- **Marked range** is the IME's pre-edit text, or none.
- **Words** follow Unicode word segmentation (UAX 29), with the `unicode-segmentation` crate, which GPUI already builds through `cosmic-text`. Adding it to the workspace is followed by `tools/check-deps`.
- **Lines** are separated by `\n`. Moving up or down keeps a goal column in pixels, so a short row in between does not lose the caret's place. The model has no pixels: the field keeps the goal column and finds the target offset in its layout, and clears the goal on any move that is not up or down and on any edit.
- **Setting the value from outside.** `value` and `default_value` follow `Checkbox`: controlled, or uncontrolled with `default_value`; `on_change(String)` reports every edit. A controlled value that differs from the model's text replaces it, clamps the selection and clears the undo history. A value that merely echoes `on_change` changes nothing.

## What an `Input` and a `Textarea` accept

| | `Input` | `Textarea` |
|---|---|---|
| Lines | one | many; `rows` sets the visible height, and it scrolls beyond that |
| Enter | calls `on_submit`, inserts nothing | inserts a line break |
| Pasted line breaks and tabs | each becomes one space | kept |
| Too long for the box | scrolls sideways to keep the caret in view | wraps, and breaks inside a word that does not fit |
| Line breaks from outside | | `\r\n` and `\r` become `\n`, in a value, a default value, a paste and the IME |

Other options on both: `placeholder`, `density` (the field's own) and `size` (a step from the density, as for a `Button`), `disabled`, `read_only` (selectable and copyable, not editable), `masked` (below), `invalid` (the danger edge and ring, and a `!` at the end of an `Input`), `required` (reported, and marked by a `FormField`) and the accessible name, which a field with no visible label takes in its constructor, as every component does: `Input::new(id, name)`. `on_change` hears the text after each edit, and after an IME composition ends, not during it.

An `Input` also takes `leading_icon` and `trailing_icon` (characters, at the density's icon size and gap, in the field's icon colour) and `clearable`, a clear button at its end while it holds text it can change, which empties it as one edit. A `Textarea` takes `min_rows` (another name for `rows`) and `max_rows`: with it the box grows with its text, a row at a time, from `rows` up to `max_rows`, then scrolls. Without it the box is `rows` high whatever it holds.

## Keys

Bindings live in a `TextInput` key context, installed by `run` with the others, so Tab and Shift+Tab keep moving focus. Shift extends the selection with any movement.

| Keys | Does |
|---|---|
| Left, Right | one grapheme; with a selection and no Shift, collapses it to that side |
| Ctrl+Left, Ctrl+Right | one word |
| Home, End | start and end of the line: the text between two `\n`, not a row of a wrapped one |
| Ctrl+Home, Ctrl+End | start and end of the text |
| Up, Down, PageUp, PageDown | `Textarea` only, in the `Textarea` key context. Up and Down move by row (a wrapped line is several), keeping the goal column; above the first row they go to the start of the text, below the last to its end. A page is the rows the box shows, less one |
| Backspace, Delete | one grapheme, or the selection |
| Ctrl+Backspace, Ctrl+Delete | one word |
| Ctrl+A | select all |
| Ctrl+C, Ctrl+X, Ctrl+V | copy, cut, paste through the system clipboard |
| Ctrl+Z; Ctrl+Shift+Z and Ctrl+Y | undo; redo |

There are no Emacs bindings. A text input never handles Tab, which moves focus, and only a `SearchInput` handles Escape, and only while it holds text, which Escape clears; otherwise Escape reaches the overlay around it, so a field in a `Dialog` or `Sheet` still closes it (ADR 0037). A `NumberInput` binds Up and Down, in its own `NumberInput` key context, to its steps.

## Mouse

- A click puts the caret there. Shift+click extends the selection.
- Double-click selects a word; triple-click selects all the text in an `Input`, and the line (without its break) in a `Textarea`. In a `Textarea` the wheel scrolls it, unless it is already at that end, when the wheel goes on to what is around it. Double-click in a masked field selects all, since its words are not shown.
- A drag extends the selection, and keeps doing so when the pointer leaves the box (ADR 0038). A click lands on the nearest grapheme boundary: GPUI's own lookup compares only where glyphs start, so a click on the left half of the last character would land after it.
- Middle-click pastes the primary selection at the click.

## Focus

One tab stop (checklist 9). Reaching it with Tab selects all its text; a click puts the caret where it landed. When it loses focus it keeps its selection but does not draw it, shows the start of its text, and takes an IME composition as it stands. While it has focus its text scrolls just far enough to keep the caret in view; a wheel scroll holds until the caret or the text changes. A `Textarea` keeps its scroll position when it loses focus; an `Input` shows its start. Typing, focusing and selecting never change the box's size or position (checklist 32): an `Input` is its density's control height (28, 34 or 40), one line of text and the padding that centres it, and a `Textarea` is `rows` lines high (three by default) with the same padding; only a `Textarea` given `max_rows` grows, and only as its text does.

A field's buttons (its field actions: a clear button, a show button, steppers) keep the field's focus: a press on one neither moves the field's caret nor takes focus, so the selection and an IME composition survive. The show button is a tab stop after the field, since the keyboard has no other way to do what it does; the clear button and the steppers are not, since Escape, Ctrl+A and Backspace, and Up and Down do their jobs.

## Clipboard and primary selection

- Ctrl+C and Ctrl+X use the system clipboard. Making a non-empty selection with the mouse or keyboard writes the primary selection, the Wayland convention; a programmatic change of selection does not.
- **Masked** fields (`masked`, for passwords) draw one mark per grapheme. They never write the clipboard or the primary selection, and Ctrl+C and Ctrl+X do nothing. Word movement goes to the start or end of the text, since word boundaries would leak the text's shape. The field's `Accessible` reports no value and no selection.

## Undo

The history keeps each edit with the selection before and after it. A run of typed characters is one step until the caret moves, the selection changes, a non-typing edit happens, or a space or line break is typed. Deleting backwards and deleting forwards each coalesce the same way. A paste, a cut and an IME commit are each one step. IME pre-edit text is never in the history. The history holds at most 1000 steps. Nothing in it depends on a clock (checklist 31).

## Drawing

- A wrap belongs to the row after it: the offset where a row begins is drawn, clicked and moved to on that row. Rows wrap one caret width short of the box, so the caret after the last character of a full row stays inside it.
- A selection is one box for each row it touches. A line break it covers is drawn `BREAK_EMS` (0.4 em, ODK's own) wide, so a selected empty line shows.
- A `Textarea` wraps its placeholder; an `Input` truncates it.

Everything below is the field's style from `components().field(...)` (or `textarea`), looked up once per render; nothing is a legacy token since 2026-10-23.

- The caret is two border widths wide, in the style's caret colour (text primary), and steady: it does not blink. A blink would keep every focused field redrawing for nothing; it can be added later as a `motion` token that stops under reduced motion.
- The selection is filled with the style's selection background (`surface.selection`), only while the field has focus, and the selected text is drawn in its foreground (`text.on_selection`), which the component checks hold to 4.5:1 on it.
- Placeholder text is the style's placeholder colour (`text.placeholder`, `text.disabled` when disabled); a disabled field's text is the disabled treatment's. Text that is too long for a placeholder ends in `…` through `single_line` (`odk-ui-components.md`).
- The box is the style's geometry (height, padding, corner radius, border width, `body` type) and its colours at rest, under hover and pressed, through `interaction.rs`'s `pointer_states`; a focused field has its focused edge in all three, and a wrong one its danger edge, so the pointer never hides them. Focus adds the ring of checklist 11 outside the box, `focus_offset` away, in `border.focus` or `border.danger`.
- IME pre-edit text is underlined in the text colour, with the caret inside it where the IME puts it. The platform's candidate window is placed from `bounds_for_range`.

## Accessibility

`odk-a11y` has `Role::TextInput`, and `Accessible` has `value: Option<String>`, `multiline` and `read_only`. A masked field reports `value: None`. Nothing reads them yet (ADR 0036).

## InputGroup

An `Input` with things around it: text before or after (`prefix`, `suffix`: `https://`, `kg`) and one button at the end (`action`: Copy, Search). `InputGroup::new(id, name)` takes every option of `Input` (`value`, `default_value`, `placeholder`, `size`, `disabled`, `read_only`, `masked`, `on_change`, `on_submit`) and passes it to the `Input` inside.

- **One box.** The group draws a field's box, from the same style and helper as an `Input` (`field_box`): its geometry, its colours under the pointer and its focus ring, while the `Input` inside has focus; the `Input` inside is bare: no box of its own (a crate-private option of `Input`). The border is always there, so focusing moves nothing. The group is exactly as high as an `Input` of its density, however tall its button's label is, and never changes height (checklist 32).
- **Addons.** `prefix` and `suffix` are text in the field's addon colour (text tertiary, disabled when disabled), drawn inside the box next to the text, `space.1` from it. Each takes at most `ADDON_SHARE` (ODK's own, a quarter) of the box and ends in `…` beyond that, and the field keeps at least that share, so long addons give way first. They are decoration: nothing reads them, and a press on one puts focus in the `Input`. A handle belongs to the element that made it, so the group makes the field's focus handle (or takes a `FormField`'s) and gives it to the `Input` (crate-private), to know where focus is for the ring and to move it.
- **The button.** `action(Button)` is a field action: drawn inside the box at its end, after the suffix, the thumb gap from the border and lower than the field (`field_action`), so a small group no longer grows to hold it. The group gives it its own density and disables it when the group is disabled; its variant, label, accessible name and `on_click` are the application's. It is a tab stop of its own, after the `Input`.
- **Keys.** Those of `Input`, with Enter calling `on_submit`; Tab goes on to the button, which Enter and Space press.
- **Reports.** The `Input` inside reports `Role::TextInput` with the group's name; the group adds nothing of its own.

## Command

A palette: a modal panel with an `Input` over a list of commands that narrows as the user types. It is an overlay on the modal layer of ADR 0037, like a `Dialog` without a title or actions, and it is controlled: the application owns `open` and `on_close`.

- **API.** `Command::new(id, name)` (`name` is what the panel is called to assistive technology); `open`, `on_close`, `placeholder` (default `Search…`), `empty` (what shows when nothing matches, default `No results`) and `item(CommandItem)`. `CommandItem::new(label)` takes `hint` (a key chord, as a `MenuItem`'s), `keywords` (more text to search, not shown), `disabled` and `on_select`. No groups and no nested lists in v1.
- **Matching.** The query is split at whitespace. An item matches when every term, lowercased, is a substring of its label and keywords lowercased. An empty query matches everything. Matches keep the order the items were added in; nothing is ranked.
- **Focus stays in the `Input`.** Opening moves focus into it, with an empty query (an earlier one is not kept), and closing gives focus back to where it was. The rows are menu items on component tokens (`menu_item`): the highlighted row has the highlight's fill, a row under the pointer the hover's, and both are a control's height; focus never moves to the list, so there is one tab stop and Tab stays in the panel. The panel is the dialog's, on the overlay component tokens.
- **Keys.** Typing filters, and the highlight goes to the first enabled match each time the query changes. Down and Up move it between enabled matches, wrapping; it scrolls into view. Enter, when it comes up, closes the palette and then runs the highlighted item's `on_select`; with no match it does nothing. Escape, a click on the scrim and a click outside ask to close. Home, End, Ctrl+A and the rest belong to the `Input`. Hovering a row highlights it; a click on an enabled row selects it as Enter does; a disabled row draws `text.disabled` and is skipped.
- **Drawing.** The panel is `COMMAND_WIDTH` wide at most (ODK's own, 560), never wider than the window less a margin, centred across the window and hanging from `COMMAND_TOP` (ODK's own, 18% of the window's height) down, so that the field stays where it is as the list narrows (checklist 32), with the `Input` on top over a border, then the list, which is at most `COMMAND_LIST_HEIGHT` high (ODK's own, 320) or what the window leaves, and scrolls beyond that. Labels truncate with `…`. It fades in and out as a `Dialog` does.
- **Reports.** The panel is `Role::Dialog`, the `Input` inside `Role::TextInput`, the list `Role::Listbox` and the rows `Role::Option`, the highlighted one selected (`item_accessibles`).
- **Enter acts when it comes up.** GPUI presses a focused button on the release of Enter or Space. A palette that closed on the press would give focus back to the button that opened it (or to whatever had focus before) in time to receive the release, and press it. So the palette runs the highlighted command on the release of the Enter it saw pressed, while focus is still in the field.

## Search, password and number fields

Each is an `Input` with what it adds, so it has the same box, density, states, keys and pointer behaviour, and looks like every other field.

- **`SearchInput`** has a search icon before its text and a clear button after it while it holds text. Escape clears it while there is text, as one edit Ctrl+Z undoes, and stops there; in an empty field Escape goes on to what is around it, so it still closes a `Dialog` (ADR 0037). `shortcut(chord)` shows the chord at its end while it is empty and not focused; it only labels the field. `loading(true)` shows a spinner at its end and reports it busy; the field stays editable. Enter calls `on_submit`.
- **`PasswordInput`** is masked and has a show button at its end (`revealable(false)` takes it away), named "Show password" or "Hide password". Showing changes only the drawing: the text, the caret, the selection, focus and the box's size stay, and the text is still a password: it stays out of the clipboard, the primary selection, the input method and `Describe`, and word movement still goes to the ends. ODK never logs or reports it.
- **`NumberInput`** has a step down and a step up button at its end. Typing is free: `-`, `1.` or `.5` stand on the way to a number, and nothing is clamped while it is typed; `on_change` hears `None` while the text is no number. Up and Down and the buttons step by `step`, rounded to its decimals so 0.1 never drifts, stop at `min` and `max`, and the button that cannot move is disabled. Enter and leaving the field commit: a number is clamped and tidied (`2.50` becomes `2.5`, `-0` becomes `0`), as one edit; text that is no number is kept, and the field shows itself invalid until it is one. A controlled `value` replaces the text only when it changes, so a half-typed number is never overwritten by an echo. What the text means is `number.rs`'s, apart from drawing: an optional sign and digits with at most one point, never an exponent, `inf` or `NaN`.

## Testing

- **Model tests** (`editor.rs`, and the offset mapping in `input.rs`) need no window: grapheme clusters (combining marks, emoji with joiners, Japanese), conversion to and from UTF-16 across characters outside the BMP, word boundaries, goal columns and undo coalescing.
- **Checklist tests** (`checklist/input.rs`, and `textarea.rs`, `search_input.rs`, `password_input.rs`, `number_input.rs`, `input_group.rs` and `forms.rs` beside it) run on GPUI's test platform. Typing uses `simulate_input`, bindings use `simulate_keystrokes`, and the mouse uses `simulate_mouse_down`, `simulate_mouse_move` and `simulate_mouse_up`, with a move that ends outside the box for the drag. The test platform keeps a clipboard and a primary selection, so both are read back. The worst-case tests of item 23 use long unbroken text, Japanese and emoji.
- **IME pre-edit** has no hook on the test platform, which stores the input handler but gives no way to feed it. Tests call `replace_and_mark_text_in_range` and `unmark_text` on the entity directly, exactly as the platform would. How Wayland delivers pre-edit is checked by hand (`docs/checks.md`).

## Not in v1

Rich text, spell checking, completion beyond `Command`, a searchable `Select`, dragging selected text, search and replace, a limit on length, input masks, a caret that blinks, X11 input methods, and visual-order caret movement in right-to-left text.
