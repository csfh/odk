# Accessibility starts as a hook: components describe themselves through `odk-a11y`

The pinned GPUI (ADR 0029) has no accessibility tree. ODK components still need a role and an accessible name, so a later accessibility layer does not mean revisiting each component. A small internal crate, `odk-a11y`, defines `Role`, `Accessible` (role, name, selected, disabled, busy) and a `Describe` trait that every component implements. The checklist tests read these descriptions. The platform does not: there is still no tree to put them in, so a screen reader gets nothing. When GPUI or a library such as AccessKit gives us a tree, `odk-ui` hands the descriptions over, and components keep their API. `odk-a11y` does not depend on GPUI, so it builds and tests without a window, like `odk-theme`.

We chose a separate crate over a module of `odk-ui` so that a future bridge to an accessibility library can depend on it without pulling in GPUI, and so that the vocabulary is versioned on its own. We rejected waiting for GPUI. Components written without roles would all have to be revisited, and that is the work this hook avoids.

## Consequences

ADR 0028's crate table gains `odk-a11y`, re-exported as `odk::a11y`; ADR 0028 is marked amended. Screen readers get nothing from ODK applications until the bridge exists. `docs/checks.md` records that GPUI 0.2.2 has no accessibility support.
