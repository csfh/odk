---
status: accepted
---

# A theme that cannot be loaded never stops an ODK application

Until now `odk::ui::run` panicked when Omarchy's current theme could not be loaded, and `docs/spec/odk-theme-tokens.md` said `odk-theme` "does not invent a palette". A malformed theme a user made themselves, a theme read halfway through `omarchy-theme-set`, or a machine without Omarchy then stopped every ODK application before its first frame. That already happened once: every ODK application panicked on the two Aether themes (`docs/checks.md`). The user asked on 2026-10-22, in the brief for the source layer of ADR 0040, that a broken custom theme must never crash an ODK application, with this order of fallbacks.

An application draws with the first of these that exists:

1. **The theme it follows**, loaded and valid.
2. **The last theme that loaded.** A reload that fails, for any reason, changes nothing that is drawn. The failure is kept for diagnostics and the next reload tries again.
3. **The emergency palette**, only when no theme has loaded since the application started. It is a fixed, monochrome dark palette in `odk-theme`: a few greys, with every status colour the same grey as the text, so it adds no hue and claims no meaning the user's theme did not give. It is not a theme and is never offered as one. Diagnostics report that it is in use and why.

Nothing fills in single keys. A theme missing a required key, or with a malformed colour under any key ODK reads, is rejected as a whole and the fallbacks above apply; ODK never draws part of a theme with an invented value. Which colour stands in for an optional key that a theme leaves out is for the semantic layer of ADR 0040 to decide, not the loader.

## Considered Options

- **Keep the panic.** It is honest, but it turns a theme-author mistake, or a reload that lands mid-swap, into a crash of every ODK application on the machine.
- **Fall back to a copy of a stock theme**, such as Tokyo Night. It looks like a real theme, so a wrong path or a broken theme goes unnoticed, and it invents hues a monochrome theme never had.
- **Fill each missing or malformed key from a default.** It hides broken themes behind half-right colours and makes the loader choose meanings, which is the semantic layer's job.

## Consequences

- `odk::ui::run` no longer panics on a theme. An application on the emergency palette works and can show `odk::ui::theme_diagnostics`.
- The emergency palette goes through the same parser and the same readability checks as every stock theme.
- Since the semantic layer of ADR 0040 was built (2026-10-22), a theme whose semantic colours cannot be resolved counts as a theme that cannot be loaded: the fallbacks above apply, so a palette is never published without its semantic colours. The resolver first adjusts a surface no text can be read on rather than refuse the theme, so every installed theme resolves (`docs/spec/odk-semantic-tokens.md`); what is refused is a theme no text could be read in at all, or colour options a theme cannot meet. Since the component layer was built (the same day), so does a theme whose component colours fail their checks (`docs/spec/odk-component-tokens.md`, "Validation"); those checks use only pairs the semantic layer already guarantees, so no installed theme fails them.
- The rest of `docs/spec/odk-theme-tokens.md` stands. `docs/spec/odk-source-palette.md` defines the keys, the loader, discovery and reload.
