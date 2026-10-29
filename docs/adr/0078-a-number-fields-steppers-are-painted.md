---
status: accepted
---

# A number field's steppers are painted

A `NumberInput` steps down and up with two buttons at its end, `−` and `+`. ODK has no icon set, so every icon is a character, and the font decides how a character looks. Omarchy's monospace font, JetBrains Mono, draws `−` (U+2212) narrower than `+`: on Atmos's Idle and Input pages, and in the gallery's number fields, the two steppers side by side are visibly different sizes. Another character only moves the problem: how long a font draws a dash beside its `+` is the font's choice, and the user picks the font.

**Decision.** The two steppers paint their marks instead of setting them in the font. A minus is one bar. A plus is that bar and the same bar turned upright, so the two marks cannot differ. The bar is 14/24 of the button's icon size long and 2/24 thick, at least one pixel, in whole pixels centred in the icon's square. These are ODK's own proportions, those of a common icon set's minus and plus. The bars take the button's text colour in each state (at rest, under the pointer, pressed, disabled), as a character would. Each button keeps its character as its icon, which is what the mark stands for, and keeps its accessible name ("Decrease", "Increase").

Every other icon stays a character until ODK has an icon set. A radio's dot and a switch's thumb were painted before this decision.

**Considered options.**

- **Another character.** None is drawn as long as `+` in every font.
- **An icon set now.** It would take a format, a renderer and a set of shapes to fix two of them. It is still the way to go once more than these two need it, and these two would join it.
- **Leave it.** The field is one of the most common controls on a settings page, and the mismatch shows on every one.
