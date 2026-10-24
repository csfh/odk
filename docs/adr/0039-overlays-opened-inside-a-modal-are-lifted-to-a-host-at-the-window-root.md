# An overlay opened inside a modal is lifted to a host at the window root, not deferred where it is declared

This amends ADR 0037, which draws every overlay where it is declared, lifted with GPUI's `deferred`. GPUI 0.2.2 cannot draw a `deferred` element inside another one: `Window::defer_draw` asserts "cannot call defer_draw during deferred drawing". A `Dialog` and a `Sheet` are deferred, so opening any popover (`Select`, `DropdownMenu`, `ContextMenu`, `Tooltip`, `HoverCard`, a breadcrumb's folded menu) inside one panics, and so does opening a modal from inside a modal. Found while building `Select`. The user decided on 2026-10-21 to fix it rather than live with it.

An overlay that is declared inside a modal's content is no longer deferred. It is handed to one host per window, `RaisedHost`, which the window's own `Shell` draws. Everywhere else ADR 0037 stands: overlays are drawn where they are declared, lifted with `deferred`.

- **What counts as inside.** The modal wraps its panel in a small element that counts, while the panel is laid out, how many modals the layout is inside of. An overlay asks `lift` to place it: at count 0 it is `deferred` in place, as before; otherwise `lift` keeps it for the host. No overlay calls `deferred` itself.
- **The host.** It is one `deferred` element at a priority above the modals and below toasts. When it is prepainted it takes what `lift` kept this frame and lays each element out at the window's origin, in window coordinates. It repeats until nothing new arrives, since a dialog inside a sheet's content is only built while the sheet's content is laid out. Each round it takes is ordered by how many modals the overlays are inside of, then by layer, and the host prepaints and paints in that order, so the pointer finds what is painted on top. An overlay inside a modal is above that modal, and below a modal opened from inside it; what is lifted while a round is laid out comes after that round. Overlays that tie keep the order they were lifted in.
- **Every ODK window has the host.** `odk::ui::open_window` opens a window with a `Shell` as its root view: the theme's base surface around the application's view, with the toast host and this host beside it. This replaces ADR 0037's `root()` drawing the toast stack: `root()` is now only the top element of the application's view. A window opened with GPUI's own `open_window` has no `Shell`, and asking it for a toast or for an overlay inside a modal panics, naming `open_window`, instead of never showing.
- **Layer order.** Popovers, then modals, then what opens inside a modal, then toasts. Outside a modal nothing changes.
- **No open state.** The host holds only the elements kept this frame, which are dropped when it has drawn them, one entry per overlay id. Open state stays with the component or the application, as in ADR 0037.

## Considered Options

- **Draw every `Dialog` and `Sheet` from a host at the window root, not deferred.** This is the proper fix we agreed to first. We did not build it because GPUI paints and hit-tests every deferred element after the whole normal tree: a modal drawn by a host in the normal tree would sit below every popover in the window, so a menu still fading out after its item opened a dialog would be above the scrim and take clicks. `root()` was also the application's own scroll container, with applications appending their content after what it held, so a host there could not be the last thing painted.
- **Draw a popover inside a modal in place, not deferred.** The dialog's body scrolls and clips it, so a `Select` near the bottom would be cut off. It would leave checklist item 33 failing.

## Consequences

- A window must be opened with `odk::ui::open_window` for an overlay opened inside a modal, or a toast, to show; GPUI's own `open_window` is the raw escape hatch (ADR 0004) and panics if asked for either.
- A lifted overlay is not a descendant of the modal in GPUI's dispatch tree. Key contexts and `on_action` handlers on the modal's ancestors do not reach it, and the modal's Escape and Tab handling does not see keys pressed inside it. Menus and the select list handle Escape and Tab themselves, so Escape closes the popover before the modal. The modal's `contains_focused` is false while focus is in a lifted popover.
- The element state of a lifted overlay's content is keyed under the host, so two overlays with the same id open at once would collide. Ids were already meant to be unique.
- If GPUI allows nested `deferred` elements, `lift` becomes `deferred` again and the host goes away.
