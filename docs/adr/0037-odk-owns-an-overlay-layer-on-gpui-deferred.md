---
status: amended by ADR 0039, ADR 0045
---

# ODK's overlay layer: overlays are drawn where they are declared, on GPUI's `deferred`

Menus, hover cards, sheets and toasts all float above the window, and GPUI 0.2.2 has only the raw parts for that: `deferred`, which paints an element after everything else, and `anchored`, which places it at a point and keeps it inside the window. The rest of the overlay layer is ODK's, inside `odk-ui`.

- **Where an overlay lives.** A component draws its overlay inside its own element tree, lifted with `deferred` and placed with `anchored`. There is no window-wide registry of open overlays. Their open state belongs to the component when it is uncontrolled (menus, hover card), or to the application when it is controlled (sheet). Toasts are the one exception. They have no trigger to live under, so each window keeps a toast stack, and `root()` draws it.
- **Layer order.** Each overlay has a `deferred` priority: popovers (menus, hover cards) are lowest, modals (sheets) are above them, and toasts are above everything, even a modal.
- **Dismissal.** Escape closes the topmost overlay that has focus. A press outside a menu closes it, and so does a click on a sheet's scrim. A controlled overlay only asks to close, through `on_close`, and the application decides.
- **Focus.** An overlay that takes focus (menus, sheets) remembers what had focus when it opened. It moves focus into itself and gives focus back when it closes, if focus was still inside. A modal keeps Tab and Shift+Tab inside itself by handling the focus actions on its own element. Hover cards and toasts never take focus.
- **Transitions.** GPUI animates only an element that is appearing. ODK keeps a closing overlay in the tree until its exit has run (`Presence`), animates opacity and position with `motion.fast`, and drops the transition entirely when `motion.reduced` is set.

We chose this over a single overlay host at the window root, which every overlay would be registered with. A host needs a registry keyed by window, a way for each component to send its content there, and a second source of truth for open state. Drawing in place keeps each component self-contained and is how GPUI's own users build popovers. We also rejected a third-party overlay crate for the reason in ADR 0027.

## Consequences

Each component drives its own transition by asking for animation frames while one runs. A modal does not make the rest of the window inert to the pointer beyond what its scrim covers. Since a sheet's scrim covers the whole window, that matters only for future non-modal overlays. GPUI does not redraw when focus moves, so ODK redraws after every focus change it makes (`docs/checks.md`).
