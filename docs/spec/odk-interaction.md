# ODK interaction contract

Normative. It must not change a decision recorded in an ADR; where the two disagree, the ADR wins. It collects what every interactive `odk-ui` component promises about states, input, focus, motion and callbacks, so that a new component is built to it and a built one can be held to it. The numbered items are the component checklist's (`component-checklist.md`), which test it; this document says what the rules are for and where the library cannot keep them. The developer's view is `docs/accessibility.md`.

Written for Phase 10 of the design language migration (2026-10-23, `docs/plan.md`). It records what the audit of Phases 6 to 9 found, since nearly all of it already held.

## Five things that are not one thing

| | What it is | Where it lives |
|---|---|---|
| **Visual state** | Hovered, pressed, focused, selected, checked, invalid, disabled: how a part is drawn | The component layer's `States` and `Interactive` (`odk_theme::component`), drawn by `interaction.rs` |
| **Behavioural state** | Loading, open, expanded, editing: what a control is doing | The component, or the application when the component is controlled |
| **Source** | Pointer, keyboard, programmatic focus | GPUI's events; a key click knows which key (`ClickEvent::Keyboard`) |
| **Motion** | A transition between two states | `interaction::progress` and `tween`, `overlay::layer_over`; the foundation's durations and curve |
| **Accessibility** | What a control reports about itself | `odk_a11y::Describe` |

None implies another: hover is not focus, focus is not selection, selection is not activation, disabled is not a dimmer enabled, and a busy control is not necessarily disabled-looking. `States` is a set of flags, not an enum, so states coexist; the component layer resolves a style from the flags and holds no table of combinations.

## State precedence

1. Disabled ignores the pointer and the keyboard, leaves the Tab order, draws no hover or press and no ring.
2. Pressed wins over hover for the fill, in the same hue.
3. Focus is its own layer: a ring outside the control (`focus_ring`, `RingTarget`), 2 px wide, `focus_offset` clear of the control's edge, in the role's focus colour, over any pointer state. It takes no space (item 32).
4. Invalid keeps its edge and mark while focused; the focus ring is drawn as well.
5. Selected and checked are drawn apart from focus and from hover, and a selected item under the pointer is still told from a hovered neighbour (`docs/checks.md`).
6. Open or expanded does not replace selection.
7. Loading keeps the control's size and colours, keeps focus, and ignores activation (`Button`).
8. Read-only is not disabled: a read-only field takes focus and its text can be selected.

## Pointer and hover

Hover and press change colour at once. GPUI 0.2.2 applies `.hover()` and `.active()` styles without a transition, and tracking the pointer in ODK to fade them would give up GPUI's release of a press outside the control and flicker on every press (`interaction.rs`, `docs/checks.md`, "Frame cost"). Phase 10 asked for hover over `fast`; it is instant, which the brief allows ("a correct instantaneous transition is preferable to a broken smooth transition"). What moves is what a component's own state drives: a switch's thumb, a tab's indicator, a button's spinner, a progress bar, an overlay.

A press that is released off the control does nothing. A press that closes an overlay goes no further (ADR 0045).

## Keyboard

Every control is reachable and operable from the keyboard alone (item 8). Tab and Shift+Tab move between tab stops; a composite control (a radio group, tabs, a segmented control, a list) has one tab stop and the arrow keys, Home and End move inside it (item 9).

| Control | Keys |
|---|---|
| `Button`, `Link` | Enter or Space presses it, on the key's release. A key held down presses once. |
| `Checkbox` | Space toggles; Enter does not (as a native checkbox). |
| `Switch` | Space or Enter toggles. |
| `RadioGroup` | Arrows move and choose, wrapping; Home and End; Space chooses the focused option; Enter does not. |
| `Tabs` | Left and Right move, wrapping, skipping disabled tabs; Home and End. A tab is selected as focus reaches it, or, with `manual_activation`, by Enter or Space. |
| `ToggleGroup` | Arrows move without choosing; Home and End; Space or Enter toggles the focused option. |
| `NavList` | Up and Down move; Home and End; Enter or Space opens the row. |
| `Select` | Enter, Space, Down or Up opens; arrows, Home, End and typing move; Enter or Space chooses and closes; Escape closes without choosing and gives focus back. |
| Menus (`DropdownMenu`, `ContextMenu`) | Enter, Space or Down opens; Up and Down move, skipping disabled items; typing jumps; Enter or Space chooses; Escape closes. |
| `Command` | Typing narrows; Up and Down move the highlight; Enter runs it; Escape closes. |
| `Dialog`, modal `Sheet` | Tab and Shift+Tab stay inside; Escape asks to close unless the dialog says otherwise; focus goes back to where it was. |
| `Popover` | Enter or Space on the trigger opens it and moves focus in; Tab leaves it (not modal); Escape or a press outside closes it and focus returns to the trigger. |
| `Tooltip`, `HoverCard` | Never take focus; open when the trigger has keyboard focus; Escape closes. |
| `Toast` | Never takes focus; Tab reaches its buttons; the clock stops while the pointer or focus is on the stack; Escape closes it while focus is in it. |
| `Input` family | Everything a text field does; Enter submits (`NumberInput` commits). |

Not built, and so not part of the contract: submenus, nonmodal dialogs, a combobox (`Command` is the searchable list), a dialog's default action on Enter (Enter presses the focused button and nothing else).

### One activation, one callback (item 38)

The callback runs when a press and its release are both on the control, or when the key that made a keyboard click is released. Not for a press released elsewhere, not for a key repeating (GPUI makes the click on the release, so a held key makes one), not for a disabled or busy control, and not for the press that closed an overlay. Controls where repeating is the point repeat: a `NumberInput`'s Up and Down, the arrow keys in a list, typing.

## Focus

- Every focus change a component makes goes through `move_focus`, so the ring redraws (item 12).
- The ring shows on any focus, pointer or keyboard. Whether it should show for the keyboard only is open (`docs/plan.md`, finding 6); Phase 10 does not settle it.
- Modal overlays move focus in on open, keep Tab inside and give focus back on close; if the trigger has gone, focus is left to GPUI. Nonmodal ones (popover, side-panel sheet) take or leave it as listed above; tooltips, hover cards and toasts never steal it (items 13, 33).
- A theme, density, corner or reduced-motion change moves nothing and loses nothing: focus, typed text, choices and open overlays survive (`checklist/forms.rs`, `navigation.rs`, `overlays.rs`).
- `odk::ui::focused_component(window, cx)` names the control that has focus, for diagnostics. It is recorded in debug builds only.

## Motion

Every duration and the curve come from the foundation (`odk-foundation-tokens.md`): `fast` 140 ms, `normal` 180, `slow` 240, `enter` 240, `exit` 160, `cubic-bezier(0.22, 1, 0.36, 1)` or linear; a component has none of its own (item 19; a source scan checks it).

| What | Over | Notes |
|---|---|---|
| Switch thumb and track | `normal` | Was `fast`; the brief asked for `normal` |
| Tab indicator | `normal` | Was `fast`; the brief asked for `normal` |
| Button's spinner in place of its icon | `normal` | Geometry kept |
| Progress bar to a new value | `normal` | A determinate value only |
| Menu and select list | in `normal`, out `exit` | Was `fast` both ways |
| Popover, hover card | in `normal`, out `exit` | |
| Tooltip | in `fast`, out `exit` | The brief names no tooltip; a hint that should not hold the pointer up |
| Dialog, sheet, command palette | in `enter`, out `exit` | A dialog rises a little, a sheet slides its width |
| Toast | in `normal`, out `exit` | Slides in a little from the edge |
| Hover, press, focus ring, a checkbox's mark, a field's edge | none | GPUI has no style transitions, above |
| Spinner | one turn per `motion.cycle` | Timer at the next glyph, not a frame loop |

No overshoot, bounce, scale or flashing. A transition reversed partway carries on from where it is (item 22); one cut short by a theme change, by reduced motion or by its component going ends where the state is and asks for no more frames (item 39). Time is GPUI's clock (item 31).

### Reduced motion

On when either asks (ADR 0048): the application (`odk::ui::set_reduced_motion`, `configure`), or the user, which is the compositor's word: Hyprland's `animations:enabled` is false. ODK asks Hyprland over its IPC socket and follows its `configreloaded`. A compositor that does not answer says nothing, and motion is not reduced. The user's own `~/.config/omarchy/shell.toml` is not a source of this preference.

A sandboxed application cannot reach Hyprland's socket and must not. `oma omarchy sync` writes Hyprland's answer into oma's copy of `shell.toml` as

```toml
[motion]
reduced = true
```

when animations are off, and leaves the key out otherwise. The sandbox mounts that copy at the usual path and sets `OMA_SANDBOX`, and the application reads the key only then.

Either one reduces motion and neither turns the other off, so there is no precedence to get wrong. The copy is watched like the text size; a change reaches open windows and overlays on the next frame. With reduced motion on, every transition is zero, every state change still happens, a spinner stands still but stays visible, a progress bar shows its value, and no key waits for an animation.

## Accessibility

What a component says about itself is `odk_a11y::Describe`: a role, a name, selected, disabled, busy, invalid, required, a description, a value, and its items for a composite (items 14 to 17). GPUI 0.2.2 has no accessibility tree, so nothing reads these on a screen reader yet; the tests read them, and the visible alternative is always there: an icon-only control carries `accessible_name` and a tooltip, a tone has a mark and a selection a fill or an underline (items 15, 18), and an error says it in text. `docs/accessibility.md` lists what is and is not covered.

Contrast is checked on every theme resolution, not by a component: text 4.5:1, a mark that shows a state, a focus ring and an essential edge 3:1, over every surface the control may be drawn on, and a theme that falls short is a failed reload (`odk-component-tokens.md`). `odk-theme/tests/components.rs` pins the pairings that interaction rests on and runs them over every fixture and installed theme.

## The contract

A component documents, in its doc comment (item 25), the following; "none" and "does not apply" are answers.

1. States it supports (`States` flags).
2. Input it accepts: pointer, keyboard, both.
3. Its keys (the table above).
4. Its focus: tab stops, roving, where focus goes on open and close.
5. Disabled: ignores the pointer and keyboard, leaves the Tab order, reports `disabled`.
6. Busy, if it starts work: keeps size, colour and focus; ignores activation; reports `busy`.
7. Pointer: press and release on it; released off it does nothing.
8. Its accessible name, and an `accessible_name` when it has only a glyph.
9. A theme, density, corner or motion change: nothing is lost or moved.
10. Its motion tokens.
11. With reduced motion: the end state at once.
12. Its callbacks: when each runs and how many times.

## What the audit found (2026-10-23)

Already held, and now tested together: one callback per completed activation, nothing for a disabled control, a press released elsewhere, or a held key, focus and values kept across a density, corner or motion change, overlays restoring focus, no frame asked for by a removed component, contrast of every pairing. Corrected: switch, tab indicator and menu durations (table above); the system's reduced-motion setting, which was a constant `false` and is now Hyprland's `animations:enabled` (ADR 0048; ADR 0046 is superseded). Added: the cross-control tests, `focused_component`, the Interaction page of the gallery with its event log, and the contract itself. Not changed because GPUI cannot: animated hover and press, a platform accessibility tree, a toast announcement to a screen reader.
