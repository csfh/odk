---
status: amended by ADR 0040, ADR 0043, ADR 0047, ADR 0048 and ADR 0062
---

# ODK takes only colours from Omarchy; every size, width and duration is ODK's own

`odk-theme` reads colours from Omarchy's theme and nothing else. The palette in `colors.toml` and the control-state colours and opacities in `shell.toml` become ODK's colour tokens as before (ADR 0030). Spacing, font sizes, corner radius, border widths and durations are fixed by ODK and ignore `shell.toml`'s `[font]`, `[spacing]` and `*-border-width` keys. The user decided this on 2026-10-20, when Atmos, the first real application, asked for a look that Omarchy's units could not give it.

We chose this because a theme is a colour scheme to Omarchy's users, while sizes decide whether a layout holds. If a theme's font base or spacing scale reached every component, one theme could break another application's layout, and nobody would test that pairing. ODK's sizes are its own, as radius already was: square corners (radius 0), no shadows, a 1 px border, and translucent fills over the theme's background. Durations are ODK's too, and a reduced-motion flag lives with them.

## Consequences

ADR 0030 stands except that `shell.toml` now supplies colours and opacities only; it is marked amended. `docs/spec/odk-theme-tokens.md` lists the fixed values. A user who changes Omarchy's font size does not change ODK applications' text size; if that is wanted later, it should come from a system-wide text scale setting, not from the theme.
