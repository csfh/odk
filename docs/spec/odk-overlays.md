# ODK overlays

Normative. How `odk-ui`'s overlays are built, placed, layered, dismissed and moved (ADR 0037, amended by ADR 0039 and ADR 0045), and what each of them does. Colours, geometry and durations are tokens (`odk-component-tokens.md`, `odk-foundation-tokens.md`); this spec is behaviour. Phase 9 of the design language (2026-10-23) brought every overlay onto the component tokens.

## Architecture

- **One layer, no registry.** An overlay is drawn where it is declared, lifted above the window with GPUI's `deferred` and placed with `anchored`. Inside a modal it is lifted to the window's host instead (`overlay.rs`, `lift`; ADR 0039). Nothing else owns open overlays: a component keeps its own open state (uncontrolled) or the application does (controlled).
- **Shared parts** (`overlay.rs`): `Presence` (how open an overlay is, so a closing one is still drawn until its exit has run), `Transition` (a length in and a length out), `layer_over` (presence plus moving focus in on open and back on close), `trap_focus`, `modal` and `docked` (a modal panel over a scrim; a panel with no scrim), `dismiss_catcher` and `Dismiss` (see Dismissal), `Layer`. `placement.rs` holds `Placement`, `Align` and `place`.
- **Components differ on purpose.** A tooltip takes no focus, a menu takes focus and has the keyboard, a modal contains focus, a toast takes none. They share the parts above and nothing makes them one configurable thing.

## Layers

From lowest: the page; popovers (menus, popovers, hover cards, tooltips, side panels); modals (dialogs, modal sheets, the command palette); what opens inside a modal (the window's host, drawn above every deferred overlay); toasts. The order is `deferred` priority, not a z-index: `Layer::{Popover, Modal, Raised, Toast}`.

- A modal's scrim takes the pointer, so a tooltip's or a menu's trigger behind it is not hovered and a tooltip never opens above a modal.
- A toast is above a modal and takes the pointer through its scrim; it never takes focus, so the dialog keeps it.
- Overlays of one layer keep the order they were declared in.

## Placement

`Placement::{Top, Bottom, Left, Right}` and `Align::{Start, End}` (both `#[non_exhaustive]`) say which side of its anchor an overlay prefers and which edge of the anchor it lines up with. `place` returns where `anchored` puts the overlay and a spacer as large as the anchor, so that `anchored`'s flip about its one point lands the overlay on the far side of the anchor and never over it. The window's edges do the rest: an overlay that does not fit flips to the other side, and is shifted back inside the window when neither side holds it. An overlay is never wider or taller than the window less a margin, and a menu under its trigger is never taller than the larger of the room above and below. The gap to the anchor is `spacing.s1`. The anchor's bounds are measured at layout (`bounds_probe`), so an overlay follows its anchor from the next frame after it moves or the window resizes. A context menu has a point, not bounds: it opens with its corner at the pointer and flips about it. Centred alignment is not offered: `anchored` places a corner, and the overlay's size is not known before layout.

## Dismissal

- **Escape** closes the innermost overlay that has focus and goes no further (`stop_propagation`), so Escape in a menu in a dialog closes the menu only.
- **A press outside** closes a menu, a `Select`'s list and a `Popover` (unless its outside clicks are off) and is the overlay's: a full-window layer under the overlay takes the press of either main button, so nothing beneath is pressed (ADR 0045). A hover card, a tooltip and a side panel have no such layer; a modal's scrim is one.
- **The scrim** of a modal closes it unless it says otherwise.
- Each overlay says which it has. `Dialog` and `Sheet`: `close_on_escape`, `close_on_backdrop` (and `Dialog::close_button`). `Popover`: `close_on_escape`, `close_on_outside_click`. A destructive confirmation turns the backdrop off; a busy dialog turns everything off, so only the application closes it.
- Choosing an item closes a menu. A controlled overlay only *asks* to close, once per request, and the application decides.
- A closed overlay keeps nothing: its keyed state is dropped with its element, and the stores kept per window (`Toasts`, the lifted overlays) forget a window that has closed the next time a toast is raised (nothing runs when a window closes, and what is kept is small).

## Focus

- **Menus** open with focus on the first enabled item; Up and Down move between enabled items and wrap, Home and End jump, a letter jumps to the next item that starts with what was typed (a pause of a second starts afresh), Enter or Space chooses, Escape or Tab closes. Focus goes back to what had it. Headings, separators and disabled items are never reached. Hover, the keyboard's highlight and the chosen item are three things, drawn apart.
- **Popover** takes focus into its panel on open and gives it back to the trigger on close. It is not modal: Tab leaves the panel to the control after its trigger, Shift+Tab to the trigger, and the window behind it stays live.
- **Dialog and modal sheet** take focus, keep Tab inside, take the pointer and keys of the window behind, and give focus back. Initial focus is the panel itself, so the first Tab reaches the first control. Escape and the scrim follow the policy above.
- **Side panel** (`Sheet::modal(false)`), **tooltip**, **hover card**, **toast**: take no focus on open. A side panel closes on Escape while focus is inside it; a toast on Escape while focus is inside it.

## Components

- **`DropdownMenu`, `ContextMenu`, `Menu`** (`menu_list.rs`). Items: `MenuItem` with `hint` (shortcut text, muted, in the item's colour when highlighted; it only labels, the application binds the key), `icon` (a glyph, as ODK has no icon set), `checked`, `radio` (a dot, reported as a radio item), `disabled`, `danger`, `on_select`. `Menu::heading` is a label over a group, not an item; `separator`. A dropdown opens below its button, above when there is none room below; a context menu at the pointer, or below its trigger with the Menu key or Shift+F10; one surface for both and for a `Select`'s list. No submenus in v1.
- **`Popover::new(id, name, trigger)`**: `title`, `content(|window, cx| …)`, `placement`, `align`, `open` / `default_open` / `on_open_change` (controlled or not), `close_on_escape`, `close_on_outside_click`. Content-driven width up to a named maximum (360 px). Opens on a press, Enter or Space (on the release).
- **`Tooltip`**: opens after the foundation's `hover_open` of rest or at once on focus in the trigger, stays while the pointer is on it, closes `hover_close` after, closes on Escape; `placement`, `align`. Never takes focus, adds no tab stop, catches no click meant for the trigger. It labels; what a task needs is visible text or a popover.
- **`HoverCard`**: the same triggers as a tooltip, on the popover token, `placement`, `align`.
- **`Dialog`**: title (a `Heading`), description (a `Text`), content, actions (the application's own `Button`s, so a destructive one is `ButtonVariant::Destructive`), the policy above. Loading and validation are content: a busy dialog turns the ways out off and disables its buttons (`Button::loading`); one that failed validation stays open and shows an `Alert`. At most 480 px wide and a margin less than the window; a tall body scrolls, the header and actions stay.
- **`Sheet`**: `Side::{Left, Right, Bottom}` (a top sheet is not offered: `Side` is not `#[non_exhaustive]`, and adding a variant would break an application's `match`), `extent`, `modal` (true: dims, traps, blocks; false: a side panel), `close_on_escape`, `close_on_backdrop`. Attached to its edge with a border on the side that faces the window and no corner of its own.
- **`Toast`**: `Toast::new(title)` with `variant(Tone)` or `neutral()`, `description`, `action(label, handler)`, `persistent()`, `timeout(Duration)`, `with_id(ToastId)`; `toast(window, cx, t)` raises it. `ToastId::named(name)` makes an id an application can name; every `Toast` has a fresh one (`Toast::id`), and `dismiss_toast(window, cx, id)` dismisses it. Raising a toast whose id is on screen or waiting updates it in place and starts its clock again. At most three show (`set_toast_limit` changes it), the rest wait in order. A toast stays for the foundation's `toast` (5 s), twice that with an action; a persistent one until dismissed. The clocks stop while the pointer is over the stack or focus is in it, and are not restarted by a theme change. The mark and its colour come from the role; a neutral toast has no mark. Every status also has a mark and words, so none shows by colour alone.
- **`Command`**: the palette's panel is the dialog's token; its rows are menu items; its shortcuts are hints.
- **`Kbd`**: on the key token, square keys.

## Motion

Durations are the foundation's (`odk-foundation-tokens.md`) and all are zero with reduced motion, every state change still happening: a tooltip appears over `fast` and goes over `exit`; a menu fades in over `normal` and out over `exit` (the same as a popover's; it was `fast` both ways until Phase 10); a popover, hover card and toast over `normal` and `exit`; a dialog, a sheet and the command palette in over `enter` and out over `exit`. The curve is the foundation's. A dialog and a toast rise or slide a little (`spacing.s2`, `s4`), a sheet slides its width. No overshoot, no scale, no blur. Reversing partway does not jump. Nothing asks for a frame while idle (checklist item 35), and no timer outlives its overlay except a toast's wake-up, which only refreshes the window.

## Theme

Every colour, border, radius and spacing is a component token read once per render (`ComponentTokens::{menu, menu_item, popover, tooltip, dialog, sheet, toast, command, kbd, trigger_focus}`), so a theme change shows on the next frame and moves nothing: open overlays stay open on the same item with the same focus, a dialog keeps what was typed, a toast keeps its deadline. No overlay reads a palette, computes a colour or watches a file. In a monochrome theme overlays stay told apart by their 1 px border, a highlight a step stronger than a hover, marks and words.

## Not supported

Submenus; nonmodal dialogs (none existed); a top sheet; centred placement; announcements to assistive technology (GPUI 0.2.2 has no accessibility tree: every overlay reports its role and name through `odk_a11y::Describe`, ADR 0036, and nothing reads them); a real pointer's hover and press fills in the headless checks.

## Migrating

- `toast(window, cx, Toast::new("Saved"))` still compiles and means the same; `Toast::persistent`, `timeout`, `neutral`, `with_id` and `dismiss_toast` are additions. A toast with an action now stays twice as long.
- `Dialog` and `Sheet` take the same builders; their entrance is `enter` (240 ms) and exit `exit` (160 ms), not one `fast`. `Dialog::close_on_escape`, `close_on_backdrop` and `close_button`, `Sheet::modal`, `close_on_escape` and `close_on_backdrop` are additions, true by default as before.
- A `DropdownMenu` now flips above its trigger and a `Select`'s list may too; a press that closes a menu no longer reaches what is under it.
- `MenuItem::hint` draws text in the hint colour, not keycaps. `Kbd` is unchanged for callers.
- A tooltip or hover card no longer takes a legacy border for its trigger's focus: a hover card's trigger shows the component layer's ring.
- `Menu::heading`, `MenuItem::radio` and `icon`, `Popover`, `Placement`, `Align` and `Tooltip::placement`/`align`, `HoverCard::placement`/`align` are new. `odk_a11y::Role::MenuItemRadio` is new.
- Breaking: none for callers of the public builders. Internally `overlay::layer` and `overlay::transition` are gone (`layer_over` and `Transition::of`); `ToneColours` is gone with the legacy toast.
