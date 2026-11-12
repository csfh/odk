---
status: accepted
---

# GPUI leaves in one breaking switch of the `odk` facade

ADR 0089 rebuilds `odk-ui` on Rune and says GPUI leaves with `odk::gpui` and `patches/gpui` when the port is done. It does not say how the facade changes for an application. The two toolkits' APIs differ in kind: a GPUI handler takes `&mut Window` and `&mut App` and a component is a GPUI element; an `odk-view` handler takes `&mut T`, the application's own state, and `show(hosted)` returns a Rune element (`docs/checks.md`, the Rune slices). A scout counted the work left on 2026-11-11 (`docs/roadmap.md`, Track A). The owner chose, on 2026-11-11, one breaking switch over two toolkits side by side.

## Decision

**`odk::ui` becomes `odk-view`'s API in one change, and `odk::gpui`, `odk-ui`, `patches/` and the `gpui` pin are deleted in that change.** There is no feature that keeps GPUI, no `odk::view` beside `odk::ui`, and no period in which both build. The crate versions are 0.x, which allows it (ADR 0064).

- `odk` depends on `odk-view` where it depended on `odk-ui`, and `odk::ui` re-exports it under the names the components already have (`Button`, `Input`, `DataTable`, and the rest), so an application's imports change little and its handlers and views change as the Rune slices describe.
- The change lands when the work packages of `docs/roadmap.md` that it needs are done, not before: the theme-following host (done), the helpers `odk-ui` has and `odk-view` lacks, `FileChooser`, `focused_component`, and a way to draw the user's font. It does not wait for several windows if Rune's `Windows` cannot be used by then; an application with one window is the case it has to serve.
- The checklist's behaviour tests are ported to `odk-view` as tests of its own before `odk-ui` is deleted, so no rule it keeps goes untested. A rule Rune gives for free, or cannot give, is recorded in `docs/checks.md` as the slices have done.
- Anything in the repository that builds on GPUI goes in the same change: the six GPUI stories (the `rune-` ones remain), `crates/odk/tests/migration.rs` (rewritten on `odk-view`), the benchmark (rewritten for Rune), and the entries of `crates/deps` that only GPUI's tree needed, found by `tools/check-deps`. `image`'s WebP, GIF and BMP features stay (`docs/checks.md`).

ADR 0004 (a raw escape hatch to GPUI) and ADR 0029 (the GPUI pin) are superseded by this change, and ADR 0089 is finished by it.

## Consequences

- An application written against `odk::ui` before the switch does not compile after it. `docs/migration.md` is rewritten for the move, with the handler and view changes shown.
- There is no way to try an application on both toolkits. The Rune stories are the showcase, and the GPUI ones are deleted, not kept.
- The work is checked by `cargo tree -i gpui` printing nothing, every Rune story seen under `tools/headless-shot`, and the idle CPU of a story under `tools/idle-cpu`.
