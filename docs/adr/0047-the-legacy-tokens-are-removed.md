---
status: accepted
---

# The legacy tokens are removed; `shell.toml` is no longer a colour source

`odk_theme::Tokens` (surface, text, accent, status, border, control, space, font, radius and motion) was the one token set ADR 0030 and ADR 0035 gave components, and ADR 0040 kept it only until every component had moved to the four layers. Every component has (plan step 8), and the last readers were `root()`, `Space` and the window's `Shell`, which read the window colour, the text colour and the five spacing steps. Plan step 9 removes it: `Tokens` and its parts, `ThemeSnapshot::tokens`, `odk::ui::tokens()`, the `Radius {sm, md, lg}` that was 0 everywhere, and `Variant`, whose three buttons are `ButtonVariant`'s and whose three badges are `BadgeAppearance`'s. `Space` stays and reads the foundation's spacing (`space.1`, `.2`, `.3`, `.4` and `.6`, the same pixels at every text size); `root()` and the `Shell` draw the semantic canvas and primary text. `Tone` stays: it is how an application names a status, and it maps onto `Role`.

With it goes the one thing only the legacy tokens read: the `[controls]` keys of a theme's `shell.toml` (control fill, border and their alphas). Nothing drew them since the form controls moved, so nothing visible changes, and ADR 0040 already derives every control colour from `colors.toml` alone. That leaves open the question ADR 0040 recorded as the user's, whether a `shell.toml` a theme ships itself is used for colour ("if it's written by Omarchy", 2026-10-21): this ADR answers it with no. The user accepted that on 2026-10-25. To answer yes later, a reader of those keys is added to the semantic layer, which can be done without touching a component. Of `shell.toml` ODK reads `[font] base-size` (ADR 0043) and, only in oma's sandbox copy, `[motion] reduced` (ADR 0048), and nothing else. A theme whose `shell.toml` is not TOML is still refused (ADR 0041), as before, so a theme that fails today fails tomorrow.

This breaks public API on purpose, in the one change that was planned for it: `odk::theme::Tokens` and the types under it, `odk::theme::ThemeSnapshot::tokens`, `odk::ui::tokens()`, `odk::ui::Variant` and `Badge::variant`. The replacements are `odk::ui::foundation()`, `theme_snapshot()` (its `semantic`) and `components()`, `ButtonVariant` and `Badge::appearance`; `docs/spec/odk-theme-tokens.md` has the table.

## Considered Options

- **Keep `Tokens` as a thin shim over the new layers.** Nothing in the workspace would read it, and a second, differently named set of values would drift from the layers it mirrors; ADR 0040's reason for four layers was to have one.
- **Keep `Variant` for compatibility.** It maps onto `ButtonVariant` and `BadgeAppearance` one to one, so the loss is a rename at each call, and keeping it would leave two names for the same thing in the prelude.

## Consequences

`root()` and the `Shell` draw the semantic `text.primary`, which is `foreground` in every stock theme but rose-pine, where the resolver's contrast-checked choice is two steps of 8-bit colour darker (6.87:1 against the canvas instead of 6.66:1); text an application sets without `Text` is drawn in it. A change to a theme's `[controls]` keys alone no longer publishes a new snapshot, since nothing draws it.
