---
status: amended by ADR 0062
---

# ODK text follows Omarchy's font and text size

ADR 0035 kept every size ODK's own and left the question open: "if that is wanted later, it should come from a system-wide text scale setting, not from the theme." Omarchy now has one. `omarchy display text size` writes `[font] base-size` (9 to 20 px, 12 by default) to `~/.config/omarchy/shell.toml`, which layers over the theme's own `shell.toml`, and `omarchy font set` writes the monospace family into fontconfig. The user asked on 2026-10-23 for ODK applications to follow both. We do, with these rules:

- **The text size is a scale, not a size.** `base-size / 12`, with the user's value over the theme's, clamped to 9 to 20 as Omarchy clamps it, and 1 when neither file has one. The foundation's type sizes and line heights, its control metrics (height, padding, icon size, gap) and its spacing and layout steps are multiplied by it and rounded to whole logical pixels, so the proportions of ADR 0040 hold and nothing clips. Borders, the focus ring and its offset, the thumb gap, radii and durations do not scale: they are about crispness, not text. The legacy `Tokens.font` and `Tokens.space` scale the same way until their components move.
- **Only `base-size` is read.** The per-token overrides in `[font]` (`caption`, `body`, ...), `[spacing]` and the `*-border-width` keys stay ignored, so a theme still cannot reshape an application's layout; the one number that changes it is the user's own, set for all their text.
- **UI text is set in Omarchy's font too.** Omarchy has one font setting, the monospace family, and its shell, terminals and launchers are all set in it. With no `FontConfig.ui` an application's UI text uses it, as the `Mono` role already did. An application that wants another face names it (`.SystemUIFont` for GPUI's default); this is the opt-out. The family is looked up again when `~/.config/fontconfig/fonts.conf` changes, so `omarchy font set` shows in a running application as `omarchy display text size` does (`docs/checks.md`).
- **The source is read like the theme.** `ThemeManager::current()` reads `~/.config/omarchy/shell.toml` as well and the watcher follows it, so a change shows in a running application; named themes and directories read no user file, so tests do not depend on the machine's setting.
- **Inside oma's sandbox the same files appear as oma's copy of them** (ADR 0044): the real ones are not visible there, and a mount of `shell.toml` would go stale, since `omarchy display text size` replaces it by renaming over it.

## Considered Options

- **Scale type only.** Fixed 28/34/40 px controls clip at larger sizes, the layout risk ADR 0035 was written to avoid.
- **Leave UI text in GPUI's default face.** What 0035 and 0040 did. It made every ODK application look foreign beside Omarchy's own shell; the user chose to follow Omarchy by default.

## Consequences

ADR 0035 is amended: sizes are no longer wholly ODK's own. `docs/spec/odk-foundation-tokens.md` ("Text size", "Typography"), `docs/spec/odk-theme-tokens.md` and `docs/spec/odk-source-palette.md` ("Text size") say how. Component tests that assert exact pixels run at scale 1, which is what `FoundationTokens::from_config` builds; only the user's own file gives another. A monospace face is wider than a proportional one, so UI text wraps sooner.
