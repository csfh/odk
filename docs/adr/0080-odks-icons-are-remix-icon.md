---
status: amended by ADR 0083
---

# ODK's icons are Remix Icon, drawn as SVG

Until now ODK had no icon set: every icon and mark was a character in the monospace font (`docs/spec/odk-ui-components.md`). The font decides how a character looks, so the marks did not hold together. A select's `▾` was a small triangle. The search field's `⌕`, the password field's `◉` and the checkbox's `✓` came from whatever glyphs the font had. A number field's `−` was shorter than its `+`, which ADR 0078 fixed by painting two bars. The owner chose Remix Icon on 2026-10-29.

**Decision.** ODK's icons are a subset of Remix Icon, drawn by GPUI as SVG in a component's colour.

- **Version and licence.** The icons are from Remix Icon 4.8.0 (npm `remixicon`, tarball SHA-1 `52d16cc63f8557140ed39d5cb9c6403bccdddb68`), the last release under the Apache License 2.0. From 4.9.0 (January 2026) the package carries the Remix Icon License v1.0, which forbids redistributing the icons as an icon library and building a competing one. Taking an icon from 4.9.0 or later is therefore a licence decision, not a version bump. The files and their licence are in `crates/odk-ui/icons/remixicon/`, unchanged.
- **Which icons.** The subset is what ODK and its applications use, each `-line` unless the name says `-fill`. The public names are `odk::ui::Icon`: `ChevronDown`, `ArrowRight`, `Check`, `Minus`, `Plus`, `Close`, `Search`, `Eye`, `EyeOff`, `Star`, `StarFilled`, `Info`, `Success`, `Warning` and `Danger`. An icon is added by its file and a variant, never by loading a file at run time.
- **Drawing.** The SVGs are embedded in the binary and served by an asset source that `odk::ui::run` gives GPUI. An icon is drawn at the density's icon size, rounded to whole pixels, in the colour a character in its place would take. In a button that is the text colour in each pointer state, followed through the button's pointer group.
- **Where they replace characters.** A select's indicator is `ChevronDown`. A checked box and a list's chosen item show `Check`, and a partly checked box `Minus`. A number field's steppers are `Minus` and `Plus`, which supersedes ADR 0078's painted bars. The clear, dismiss and close buttons are `Close`, the search field's icon is `Search`, and the password field's show and hide buttons are `Eye` and `EyeOff`.
- **Characters remain.** `Glyph` is what a component takes where it shows an icon: an `Icon`, or a character as before, so `Button::icon(id, "⚙", name)` still works. ODK's other marks stay characters for now: status marks, a wrong value's `!`, a radio's chosen `●` in a menu, spinners and keycaps.

**Considered options.**

- **Painting each mark**, as ADR 0078 did for the steppers. Bars work for a minus and a plus, but not for a chevron, a magnifier or an eye.
- **Remix Icon 4.9.1, the latest.** Its licence's restrictions on redistributing the icons as a library sit badly with a toolkit that hands icons to applications. 4.8.0 has the same shapes under Apache 2.0.
- **An icon font.** It would need a font at run time beside Omarchy's, and a character in it would suffer the font-matching problems this decision removes.
