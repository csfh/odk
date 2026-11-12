---
status: accepted
---

# A sandboxed window finds its font in the directories the sandbox shows

ADR 0035 and ADR 0043 set UI text in Omarchy's font, and `odkd` reports the family the user chose (ADR 0062). Rune loads a family from bytes given before the window opens and has no call that loads one afterwards (`docs/checks.md`), and it cannot find an installed family by name. A sandboxed window cannot ask fontconfig and, since ADR 0062, sees none of Omarchy's own files. The owner chose, on 2026-11-11, to show the font directories read-only and let the window find the file, over having `odkd` send the bytes or dropping the Omarchy font.

## Decision

**The window finds the family's files itself, in directories the sandbox shows read-only.** `/usr` and `/etc/fonts` are already shown (`docs/spec/sandbox-baseline.md`), so the system's fonts are there. One mount is added: the user's `~/.local/share/fonts`, bound read-only at the same path for every application and with no permission to ask for, when the directory exists. A font names shapes and nothing else, and the directory is the user's own, so the mount widens what an application can read by files whose only use is to be drawn. It is not one of ADR 0012's six named directories, and ADR 0012 is amended by it in the same way ADR 0062's `odk/` directory was.

**The lookup is `odk_view::Face::installed(family)`.** It searches `/usr/share/fonts`, `/usr/local/share/fonts`, `~/.local/share/fonts` and `~/.fonts` for font files (`ttf`, `otf`, `ttc`, `otc`) whose name holds the family's letters and digits, lowercased; reads each one's name table; keeps the upright faces whose family is the one asked for (its typographic family when it has one); and loads the regular face, the one nearest weight 400, and the bold face, the one nearest 700 from weight 600 up, when there is one. It follows a link to a file and not a link to a directory. It uses `read-fonts`, already in the tree through Rune (ADR 0066's check: no new code is compiled in).

**`odk_view::run` follows `odkd`'s family unless the caller gives a face.** When no such family is installed, or `odkd` is not there, the window keeps Rune's bundled one, as the emergency palette stands in for a theme (ADR 0041).

## Consequences

- A font installed or changed while a window is open applies at the next start: Rune cannot load it later. The window says so in its log and draws on.
- `~/.config/fontconfig/fonts.conf`, which names the family, is still read only by `odkd`. The window does not parse fontconfig's configuration; a family it finds by name only is the one fontconfig would have named.
- The search reads file names first and parses only the files that match, so a large `/usr/share/fonts` costs a directory walk at start.
- An italic file is not used for the regular face, and a family with no regular upright face is not found.
