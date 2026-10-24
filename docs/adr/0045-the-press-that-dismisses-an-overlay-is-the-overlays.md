---
status: accepted
---

# The press that dismisses an overlay is the overlay's, and goes no further

ADR 0037 closed a menu on a press outside it and left the press to carry on, so clicking away from an open menu also pressed whatever button lay under the pointer (found by `checklist/menu.rs`, 2026-10-23). The overlays' brief asked that an outside click not activate an unrelated control. Every overlay that closes on an outside press (menus, a `Select`'s list, a `Popover` that has outside clicks on) now draws an invisible layer over the window, under itself, that takes the press of either main button and asks to close; nothing beneath is pressed, as in a native menu. A `Popover` can turn outside clicks off, and then it has no such layer and the window behind stays live. A modal's scrim already did this, and a hover card or tooltip, which no press closes, has none.

## Considered Options

- **Let the press through and ignore it in the control.** Controls cannot know that a menu has just closed; a frame is drawn between the press and the release, so no state tells them.
- **Close on the release instead.** The button under the pointer would still see the press.

## Consequences

The first click away from an open menu is spent closing it, so a second click on its trigger closes it and cannot reopen it in the same press. The wheel does not scroll the page behind an open menu. A `Popover` is not modal: it takes focus on open, does not trap Tab (Tab and Shift+Tab leave it beside its trigger), and the window behind it works while outside clicks are off.
