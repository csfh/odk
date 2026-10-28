---
status: accepted
---

# A button can listen for a chord

Atmos's Keybindings page records a shortcut: the user presses "Record shortcut", then the chord, and the page writes it in Hyprland's words. The QML window does it with an invisible item that takes focus and reads `Keys.onPressed`. An application built from the prelude alone has no key event to read: no component reports a key press, `Input` reports text, which drops the modifiers, and reaching for GPUI's key listeners is what the prelude-only rule forbids. `Kbd` only draws a chord.

**Decision.** A `Button` can listen. `listening(true)` makes the focused button take the next key press, with the modifiers held, as a `Chord` (`key`, the platform's name for it, and `control`, `alt`, `shift`, `platform` for Super, `function`) for `on_chord`, instead of pressing. While it listens:

- a key held down reports once, and a lone modifier reports nothing (the platform sends no key press for one);
- Escape reports `None`, a cancel (checklist item 10), and Tab still moves focus (item 9), so the user is never trapped; focus leaving while it listens reports `None` once;
- a key it took does not press it when released, so Enter and Space can be recorded although they press a focused button on their release.

The application owns the state, as it does a `Switch`'s: it starts listening in `on_click`, says so in the label ("Listening…", which is what assistive technology reads), and stops when `on_chord` has the chord it wants or hears a cancel. It turns the `Chord` into its own words; ODK names no key for any configuration language.

**Considered options.**

- **A component of its own, a chord field.** It would be a button in every respect but this one: the same press, focus ring, states and name. A mode of `Button` keeps one focusable control.
- **`Input` reporting chords.** A field edits text; taking keys away from it while it shows text would make it neither.
- **Recording Escape as a key.** The QML window does, but then nothing cancels, and the checklist asks that Escape close what was opened.
