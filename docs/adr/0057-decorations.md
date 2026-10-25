---
status: accepted
---

# A component's behaviour is changed by attaching decorations

Every component has a sensible default. A caller changes one behaviour by attaching a decoration, the way a class changes one thing about an element and leaves the rest. Truncation and overlay sizing are the first use (ADR 0058). A decoration is not a variant, a tone or a density: those choose a look from the component tokens. A decoration chooses a behaviour the component already knows how to draw.

## Decision

- **Attached by a method.** `Component::decorate(decoration)` takes the decoration by value and records it. A component that does not accept a decoration has no such method, so the call does not compile. There is no stringly class list and no untyped bag.
- **Typed by concern.** Two concerns exist. `TextDecoration` changes how text flows: `Wrap`, `EndEllipsis`, `MiddleEllipsis` and `Clamp(n)` (`n` of 0 is one line). `SizeDecoration` changes an overlay's size: `Autosize` and `FixedWidth(px)`. An overlay accepts `OverlayDecoration`, which is either concern, implemented on each decoration type (a blanket impl would overlap, since one type could be both). A button accepts `TextDecoration` only, so `Button::decorate(Autosize)` does not compile.
- **They compose across concerns and conflict inside one.** `Applied` stores at most one value per concern. A later decoration of the same concern replaces the earlier one. A decoration of the other concern leaves the first in place. A concern that was never set is the component's own default (`Applied::flow`, `Applied::size`).
- **The component says which decorations it accepts, and what a decoration means on it.** A menu row accepts `MiddleEllipsis` and maps `Wrap` and `Clamp` back to one line, because a row is one line. A dialog accepts both concerns. Tabs accept neither: their overflow is a horizontal scroll, and that is not a decoration (ADR 0058). Tooltip and hover card do not accept a decoration; their maximum widths stay the fixed measures they have.
- **`Describe` is not a decoration.** A decoration changes what is drawn. The accessible name stays the full string the component was given. A tooltip that shows a truncated string is visual recovery for the pointer and the keyboard; it is not a second name, and it adds no tab stop (ADR 0036, ADR 0058).
- **Tested as the mechanism and as each use.** `decoration.rs` checks that the later decoration of one concern wins and the other concern stays, and that a missing decoration is the default the component passes. Each component's checklist tests the behaviour that default and those decorations produce.

## Considered Options

- **A `Truncation` parameter on every constructor.** One enum cannot also size an overlay, and a parameter that most callers leave at the default is noise on every call. A decoration is absent until someone asks.
- **GPUI styles at the call site** (`line_clamp`, `text_ellipsis`, `whitespace_nowrap`). That is how text was clipped before, and it is what the gallery used to do. It bypasses the one helper, the tooltip and `Describe`, and GPUI's `truncate` clips inside a shrinking flex item (`docs/checks.md`).
- **One decoration trait for everything.** A button could then be given an overlay width. The type is what stops that.

## Consequences

`odk_ui` and its prelude export `Wrap`, `EndEllipsis`, `MiddleEllipsis`, `Clamp`, `Autosize` and `FixedWidth`. A new behaviour is a new decoration type and a new concern only when it does not fit these two. ADR 0058 is the first behaviour; it does not change this mechanism. ADR 0036 is unchanged: nothing here builds an accessibility tree.
