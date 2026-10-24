---
status: amended by ADR 0035
---

# `odk-theme` maps Omarchy's theme files onto ODK's own semantic tokens

Omarchy's current theme is two files under `~/.local/state/omarchy/current/theme/`: `colors.toml`, a palette (`background`, `foreground`, `accent`, `red`, and so on), and `shell.toml`, which holds surface and control-state settings, a font base size and a spacing scale. `odk-theme` reads both and produces ODK's own small set of semantic tokens, such as surface, text, accent, status colours and the states of a control. Components (ADR 0027) use only those tokens and never an Omarchy key name.

We chose this over exposing Omarchy's files one for one because the mapping then lives in a single place: if Omarchy renames a key or changes a file, only `odk-theme` changes, not every component. It is also how shadcn works, with semantic tokens such as background, foreground and primary in front of raw colours. The cost is that the mapping must be written and kept up to date. When `shell.toml` is missing or lacks a value, `odk-theme` derives it from the palette, as Omarchy's own shell does.

## Consequences

The token list and its mapping from the two files are to be written as a spec before `odk-theme` is built. The format of both files is Omarchy's and may change; `docs/checks.md` should record which Omarchy version the mapping was checked against.
