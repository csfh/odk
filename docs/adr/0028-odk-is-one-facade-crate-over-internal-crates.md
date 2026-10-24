---
status: amended by ADR 0036 and ADR 0049
---

# Applications depend on one `odk` crate, which re-exports internal crates as modules

ODK is built from several crates but applications name only one, `odk`, a thin facade that re-exports the others as modules: `odk::ui`, `odk::theme` and later `odk::plugin`. We chose this over one big crate with feature flags, which cannot stop the theme code from reaching GPUI, and over making applications depend on each crate separately, which lets them drift onto different versions.

| Crate | Role |
|---|---|
| `odk` | The facade. Re-exports `odk-ui` as `ui`, `odk-theme` as `theme`, a `prelude`, and the pinned GPUI as `odk::gpui`, the raw escape hatch of ADR 0004. `odk::plugin` is added behind a feature when the Luau host exists (ADR 0003, ADR 0021). |
| `odk-theme` | Reads Omarchy's current theme into typed tokens and reports changes (ADR 0019). Does not depend on GPUI, so it builds and tests without a window. |
| `odk-ui` | The components (ADR 0027), built on GPUI and `odk-theme`. |

Applications never put `gpui` in their own `Cargo.toml`; `odk::gpui` is the only way to it, so they cannot end up on a different GPUI version from the one ODK is tested against. The internal crates are implementation detail: their names and boundaries may change without a major version of `odk`.

The OMA crates never depend on ODK, and `omagate` keeps using GPUI directly (ADR 0013, ADR 0016).
