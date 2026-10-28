---
status: accepted
---

# A drag holds the window until the button is released, and every drag has keys

Resizable panels, sliders and scroll bars move with the pointer for as long as the button is down, wherever the pointer goes, and `docs/spec/odk-ui-components.md` kept them blocked until that was designed. GPUI 0.2.2 has no pointer capture: an element hears a move only while the pointer is over it. `Input` already selects by dragging past its box with listeners on the whole window (ADR 0038). Atmos needs a list and a detail beside it, sized by the user and kept.

## Decision

**A drag is a left press on a part that drags, then every move, until the left button is released.** While it lasts the component registers listeners on the window (GPUI's `on_mouse_event`, during paint), which hear every move and the release wherever the pointer is, as `Input`'s selection does; they are registered only during a drag, so a component at rest costs nothing (checklist item 35). On Wayland the compositor sends the release to the window that had the press, so a drag that leaves the window ends when the button comes up. The pointer shows the drag's cursor over the whole window while it lasts. Escape during a drag puts back what was there when it began and ends it. A drag reports once, at its end: the component moves as the pointer does, and its callback hears the result when the button is released, so an application that keeps the result writes once per drag.

**Every drag has keys.** The part that drags is a tab stop, and the keys do in steps what the pointer does continuously, so nothing needs a pointer (checklist item 8). Each key press is a completed change and is reported.

**`Resizable` is the first.** A group of panels in a row or a column, `Resizable::horizontal(id, name)` or `vertical`, each panel a `ResizablePanel` with a `default_size`, `min_size` and `max_size` as fractions of the group, the content its own. Between two panels is a handle: a line one border wide in the separator's colour, with a pointer target `geometry.target_min` across centred on it, drawn over the panels' edges so it takes no room, the accent's colour while hovered, dragged or focused, and the resize cursor. A handle is a tab stop of its own, as a link in a path is; the arrow keys along the group's axis move it by a twentieth of the group, Home and End put the panel before it at its smallest or largest. A panel never goes below its smallest or above its largest, and neither does its neighbour, which takes the difference. `on_layout(sizes)` hears the fractions after a drag ends and after a key moves a handle, which is when to keep them; `default_size` puts them back. Sizes are fractions, so a group keeps its proportions when the window changes size, and no pixel number is in its API (an application built from the prelude alone cannot name one). There is no collapsing, no nesting rule beyond a group inside a panel, and no motion: a resize follows the pointer or the key at once.

**Accessibility.** The group reports `Group` with its name, and each handle `Separator`, focusable, with the size of the panel before it as its value (`40%`), as WAI-ARIA's window splitter does.

## Considered Options

- **GPUI's `on_drag`.** It is drag and drop: it carries a value and draws a view under the pointer, and the drop target receives it. A resize has no payload and no target.
- **Listen only on the handle.** The pointer leaves a thin handle at once; the resize would stop whenever the user moves faster than a frame.
- **Report every move.** An application that keeps the sizes would write at every frame of a drag. A drag reports at its end, as a key press does.
- **Sizes in pixels.** A panel of a fixed width would not follow the window, and the API would need a length type an application built from the prelude cannot name.
