---
status: amended by ADR 0042
---

# ODK's design language is four token layers over the Omarchy palette

Omarchy supplies the palette; ODK defines what each colour means, how strong it is, and how a control is shaped and moves; GPUI draws the result. We split what `odk-theme` produces into four layers, each computed only from the one before: source tokens (the active theme's `colors.toml`, and its `shell.toml` where Omarchy wrote one), foundation tokens (geometry, density, type, spacing, motion, all ODK's own), semantic tokens (colours by role and emphasis, made by an adaptive resolver that checks contrast and works in OKLCH), and component tokens (the final style a component draws with). Components read component tokens only. The resolver lives in `odk-theme`, which has no GPUI dependency, so it is tested without a window.

The user set these values on 2026-10-21. Radius is 0 or 0.8 px (default 0.8; 2 px since ADR 0042), borders 1 px, the focus indicator 2 px. Control heights are 28, 34 and 40 px for compact, default and comfortable density. Durations are 140, 180 and 240 ms with the easing `cubic-bezier(0.22, 1, 0.36, 1)`. Semantic roles are accent, neutral, danger, success, warning and info, each at solid, muted and subtle emphasis. Text contrast is at least 4.5:1 and essential non-text contrast at least 3:1. A monochrome theme such as Vantablack keeps its single hue; the resolver never adds one.

## How it was accepted

It disagreed with ADR 0035 and `docs/spec/odk-theme-tokens.md`, which fixed radius at 0 and durations at 120 and 200 ms on 2026-10-20, so it stayed `proposed` and no code drew its values until the user asked for the first change that alters what is drawn. On 2026-10-22 the user's brief for the button redesign asked for that change, with this ADR's values, and so accepted it, as the answer of 2026-10-21 below had agreed. In that change `Button` draws the component layer, ADR 0035 is amended by this ADR, and `docs/spec/odk-theme-tokens.md` keeps its sizes and durations only for the components that have not moved yet.

## Answers of 2026-10-21

- The user said `shell.toml` can be used "if it's written by Omarchy". The resolver derives control colours from `colors.toml` alone when the file is missing, as `odk-theme` does today. Whether a `shell.toml` that a theme ships itself is used, and whether `odk-theme` can tell who wrote a file, are not settled (`docs/plan.md`).
- `Size` stays per instance and may act differently under each density.
- 0.8 px is meant literally.
- The user thinks this replaces the decision of ADR 0035, said as "I think so", and then agreed that the status stays `proposed` until the first change that alters what is drawn. That change marks this ADR `accepted` and ADR 0035 `amended by ADR 0040`, together with the new constants and the spec table.
- Button variants are a new `#[non_exhaustive] ButtonVariant`; `Button::variant` takes `impl Into<ButtonVariant>` and `From<Variant>` kept existing calls compiling. `Variant` was removed with the legacy tokens (ADR 0047).

## Answer of 2026-10-22

- In the brief for the component layer, the user exempted parts whose roundness says what they are from the radius rule: a switch's track and thumb, and a spinner, are a pill or a circle. The same day the user added the radio. They are shapes, not a larger corner radius, and no rectangle is rounder than 0.8 px. Like the rest of this ADR, it is drawn only once the ADR is accepted.

## Consequences

ADR 0035's rule stands in spirit: only colours come from Omarchy. What changes is what ODK does with them, and the sizes and durations ODK chooses. The foundation layer is built as `odk_theme::foundation`, with the values above and the rest of its tokens in `docs/spec/odk-foundation-tokens.md`. The source layer is built as `odk_theme::Palette`, with its discovery, reload and fallbacks (ADR 0041), in `docs/spec/odk-source-palette.md`; the legacy tokens are built from it too. The semantic layer is built as `odk_theme::semantic`, its resolver working in OKLCH with the contrast minimums and the monochrome rule above, in `docs/spec/odk-semantic-tokens.md`, and published in every theme snapshot. The component layer is built as `odk_theme::component`, in `docs/spec/odk-component-tokens.md`: each component's colours are references into the semantic layer, checked with every resolution, and its geometry follows from the foundation; `odk::ui::components()` publishes it with the theme snapshot.

`Button` is the first component that draws the component layer, and only its tokens. The others kept drawing the legacy tokens, with ADR 0035's square corners and durations, until each moved, one per change, through `docs/spec/component-checklist.md`; they all have, and ADR 0047 removed the legacy tokens. `docs/plan.md` ("Design language migration") holds the audit, the target module layout and the order of work. A control's `Size` is a step from its density, `Sm` one tighter and `Lg` one roomier (`docs/spec/odk-component-tokens.md`, *provisional* until the user confirms it).
