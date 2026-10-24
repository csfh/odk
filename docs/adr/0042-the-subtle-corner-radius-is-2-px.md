---
status: accepted
---

# The subtle corner radius is 2 px, not 0.8 px

ADR 0040 set the corner radius to 0 or 0.8 px, default 0.8, meant literally. Once `Button` drew it, the user could not see a 0.8 px corner at all, and asked on 2026-10-22 for a radius that renders. Measured on a headless display at scale 1, a corner's pixel loses only about 2% of the fill toward the surface at 0.8 px, 5% at 1, 17% at 1.5, 40% at 2, and all of it at 3, with the neighbouring pixels blending too (`docs/checks.md`). The user chose 2 px: the least radius that reads as a corner at scale 1, while the control still reads as square-cut; from 3 px it reads as a rounded web control, which the design language rejects. `CornerRadius::Subtle` is now 2 px and stays the default, `Square` stays 0, and the radius is still one setting for the whole application.

## Considered Options

- **1 or 1.5 px.** 1 px is as invisible as 0.8 at scale 1; 1.5 px shows clearly only at scale 2.
- **3 or 4 px.** Visible everywhere, but soft: the look of generic web controls.

## Consequences

ADR 0040 is amended: every rule it gives that names 0.8 px now means the subtle radius, 2 px. No rectangle is rounder than 2 px, and the parts that are round by nature (a switch, a spinner, a radio) are still shapes, not a larger radius. 2 px is a whole logical pixel, half a step of the 4 px spacing grid, and four device pixels at scale 2.
