---
status: amended by ADR 0070
---

# Text truncates by a component's default, and a decoration changes it

Long text was clipped, wrapped or ellipsized differently in each component, and a clipped string was often unreachable. ADR 0057 is how a caller changes a default. This ADR is the default, and what the decorations do.

This does not meet WCAG 1.3.1, 4.1.2 or 1.4.4. GPUI 0.2.2 exposes no accessibility tree (ADR 0036, unchanged). `Describe` keeps the full string and the tests read it, but a screen reader gets nothing, so a name, a role and a relationship are not exposed. The text size still stops at 20 px, 167% of the 12 px default (ADR 0043), short of 1.4.4's 200%. A tooltip is visual recovery for someone who can see the control. It is not an accessible description.

## Decision

**One rule.** Prose wraps: body text, a heading, a form's label and description, an alert, a toast's body, a checkbox, a switch, a radio, a tooltip. A one-row control ends in `…` through `truncate_end` (`overflow_hidden`, `line_clamp(1)`, `text_ellipsis`), the public replacement for `single_line`. Nothing calls GPUI's `truncate`. `truncate_lines` is the same for a cap of more than one line.

**When the text does not fit, the full string is shown in a tooltip** on hover and on keyboard focus. The tooltip is drawn only when a measurement says the text overflowed, and it adds no tab stop. The measurement is one frame behind: a probe records overflow and asks the window to draw again only when the answer changes, so an idle control does not redraw (checklist item 35). `Describe` keeps the full string either way. `Wrap` shows the whole string in the control, so there is no tooltip.

A menu row's keyboard focus is the highlighted item. That row already tracks a roving handle, and the tooltip opens when the handle is inside it. A command row is not focused: focus stays in the field, and the tooltip opens while that row is the highlighted command. Neither adds a tab stop.

**Titles that are not tab stops stay hover-only.** A dialog title and a sheet title are headings, not tab stops, and nothing here makes them tab stops. The same rule covers every other truncated string that is not already a tab stop and is not a highlighted row: `Text`, `Heading`, `Badge`, a menu heading, a `StatusIndicator`, and the current page of a breadcrumb. A crumb that is a link is already a tab stop, so its tooltip also opens on focus. Nothing is added to the tab order to carry a tooltip.

**Escape.** The tooltip dismisses on Escape and does not stop the key, which is how every `Tooltip` already behaves. A menu list is an ancestor of its row, so that same Escape reaches the list and closes the menu. The tooltip does not take a press of its own: one Escape closes the menu, and the tooltip goes with it. That dismissal does not stick after the menu has left the tree, so the next time the menu is shown a truncated highlighted row opens its tooltip again. A command row's tooltip is not on the field's key path. The palette's modal handles Escape, stops it and closes the palette, and the tooltip goes with the panel. A toggle segment's tooltip dismisses and the segment stays, because the group does not close on Escape. The dismissal holds while that segment keeps focus, as it does for any tooltip.

A tooltip inside a menu or command list cannot be `deferred`. The list already is, and GPUI cannot nest `deferred`. Those tooltips are lifted to the window's host, the same way an overlay inside a modal is (ADR 0039). The list's own overlay stays deferred.

**Middle truncation is opt-in.** `MiddleEllipsis` is a decoration. Nothing infers it from the string. It shortens on grapheme boundaries, measured with `text_width`, keeping both ends; the left side keeps the extra grapheme. If even `…` does not fit, the result is `…`. The command palette's URL row and the gallery's typography page opt in. A menu row and a command row stay one line: `Wrap` and `Clamp` on those items are drawn as the end ellipsis.

**Line caps.** A dialog title and a sheet title wrap up to 3 lines and then end in `…` (`Clamp(3)`). `Wrap` lifts that cap. `Clamp(n)` sets another. The body of a dialog, a sheet, an alert and a toast wraps without a cap.

**Overlay size is a decoration too** (amending ADR 0043). An overlay takes an explicit size or `Autosize`. Both are capped at the window, less the margin overlays already use.

| Overlay | Default | What `Autosize` does |
|---|---|---|
| Dialog | fixed 480 px | that measure times the text scale, capped at the window. It does not grow to the body |
| Sheet | the caller's `extent`, or 400 px | that extent times the text scale. `FixedWidth` replaces the extent |
| Menu, `Select` list | `Autosize` | as wide as the longest label, at least the trigger (or 192 px), then the window. A label that still does not fit ends in `…` |
| Toast | `Autosize` | 336 px times the text scale, capped at the window. The stack scrolls when it is taller than the window |
| Command | fixed 560 px | 560 px times the text scale |
| Popover | fixed 360 px | 360 px times the text scale |
| Tooltip, hover card | fixed 240 px and 336 px | no decoration; they stay fixed |

A fixed width does not scale with the text size. `Dialog::decorate(FixedWidth)` and `Dialog::decorate(Autosize)` are the same concern, so the later call wins.

**Tabs are the exception.** A tab list that does not fit scrolls sideways, and the selected tab is brought into view. Tabs take no truncation decoration. A decoration that wrapped or ellipsized them would remove the exception, so they do not accept one.

**Toasts.** The stack is collapsed by default. Identical on-screen toasts — the same title, description, role and action label — are one card, with a count of how many are on screen (not how many are waiting). The same `ToastId` still updates in place. The limit still counts toasts, not cards. Hovering the stack or moving keyboard focus to it expands the stack, one card per toast, and pauses the timers; leaving, or moving focus on, collapses it and the timers run again. That is the same condition that already paused the clocks. Dismissing a grouped card dismisses its newest member. The collapsed and the expanded column scroll inside the window when they are still too tall. A toast's description joins `Accessible.name` as `"{title}: {description}"`, the way an alert's does.

**Fields.** An `Input` that is not focused shows an ellipsis when the value is wider than the box, painted over the end of the line; focusing it scrolls, as before. A `Textarea` wraps, and its placeholder wraps up to the field's row count and then ends in `…`. A hint, a counter and a progress value take at most 40% of the row (`TAIL_SHARE`) and then end in `…`, so they do not overflow once the label has gone; the label still yields first.

**Data table.** A cell ends in `…` and shows the full text in a tooltip on focus. `DataTable` is still blocked on pointer capture and a virtualised list; this is the rule for when it is built, not an implementation.

**The gallery** draws its text through `Text`, `Heading` and the other components, with decorations where the page is showing truncation. It does not call GPUI's text styling itself. The code panel wraps in the monospace role inside a vertical scroll: a scroll on one axis clips the other, so a horizontal scroll cannot also wrap. GPUI refuses a text run that contains a newline, so each line of the panel is its own `Text`. Wrapping text takes no overflow probe: it cannot be truncated, and a probe on every line would ask for a frame.

**Where the tooltip is wired.** `Button` (when the button is width-bounded; a button at its natural width grows), `Badge`, `Text`, `Heading`, the `Select` value, menu rows, menu headings, command rows, breadcrumb crumbs, `StatusIndicator` and toggle segments. A menu row, a command row and a toggle segment open on hover and on highlight or focus. A menu heading, a status, the current page of a breadcrumb, and a dialog or sheet title open on hover only: they are not tab stops, and they are not given one.

## Considered Options

- **A heuristic for middle ellipsis** (a URL, a path, an unbroken token). Callers know which strings those are. A heuristic truncates ordinary words in the middle.
- **Grow every overlay with the text size.** ADR 0043 kept borders, radii and raw widths out of the scale so a layout stays put. Autosize is the opt-in for the overlays whose content is the text; a dialog stays a fixed measure unless the caller asks.
- **Make every truncated control a tab stop** so the tooltip opens from the keyboard. That adds stops for text and badges, and changes the tab order of every page that has one. Hover covers them; `Describe` keeps the string for the tests.

## Consequences

ADR 0043 is amended: a fixed overlay width still does not scale, and an `Autosize` overlay's measure does, capped at the window. `docs/spec/odk-ui-components.md`, checklist item 23, `docs/spec/odk-text-input.md`, `docs/spec/odk-foundation-tokens.md`, `docs/spec/odk-overlays.md`, `docs/accessibility.md` and `docs/checks.md` follow this ADR. ADR 0036 is not amended.
