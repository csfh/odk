# ODK's text input is our own editing model, on GPUI's `EntityInputHandler`

`Input`, `Textarea`, `InputGroup` and `Command` all need a caret, a selection, IME, the clipboard and scrolling, and GPUI 0.2.2 supplies only the platform end of that: `EntityInputHandler` (UTF-16 ranges, marked text, the bounds of a range), `Window::handle_input`, shaped and wrapped text layout, and the clipboard and primary selection. Everything between those and a working field is ODK's, inside `odk-ui`.

- **An editing model with no GPUI in it.** An `editor` module (not `text`, which is the `Text` component's) holds the text, the selection as UTF-8 byte offsets on grapheme boundaries, word and line movement, the IME marked range and the undo history. It tests without a window. A GPUI entity and element implement `EntityInputHandler` over it and convert to UTF-16 only there.
- **One engine for both shapes.** `Input` is one line that scrolls sideways; `Textarea` wraps and scrolls vertically. The model handles several lines from the start, so `Textarea` is not a rewrite.
- **Mouse selection needs no pointer-capture foundation.** GPUI's `on_mouse_move` fires only while the pointer is over the element, so a drag would stop at the edge of the box, which is what GPUI's own example does. The text element instead registers a window-level mouse listener with `Window::on_mouse_event` while it paints and a drag is on, and that listener is not filtered by hover. `Input` therefore does not wait for the drag foundation that `Resizable` and `DataTable` need.
- **IME is Wayland's `text-input-v3` only**, through `replace_and_mark_text_in_range`. Other platforms are not targets (ADR 0001).
- **Order.** `Input` is built first and proves the engine. `Textarea`, `InputGroup` and `Command` follow on it.

We chose this over adopting a third-party text input, for the reason in ADR 0027: it would bring its own theme and component set. We chose it over copying GPUI's `examples/input.rs`, which is a starting point only: one line, no scrolling, no word movement, no undo and no tests. We kept the model inside `odk-ui` rather than a new crate, so ADR 0028's crate table does not change; it has no GPUI types, so it can move out later if it needs to. The behaviour (keys, clipboard, undo, masking, caret, accessibility and tests) is in `docs/spec/odk-text-input.md`.

## Consequences

Caret movement, grapheme handling and IME are ODK's to get right and to keep right, and the first version moves the caret in logical order, not visual order, in right-to-left text. `Select` does not need this foundation, since it matches keystrokes against its items. A searchable `Select` would. Real IME and primary selection cannot be exercised on the headless test platform, so they are listed in `docs/checks.md` and need a hand check in the gallery.
