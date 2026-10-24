# The consent dialog helper is `oma-consent`, and `oma consent` runs it

The dialog helper of ADR 0025, named `omagate` in ADR 0026, is named `oma-consent`, and `oma` has a command `oma consent <target>` that runs it: it shows the consent dialog for a Bundle and records the answer in the consent record (ADR 0023). `oma run` and `oma install` use the same step when a Bundle requests something its record lacks, so there is one consent path (ADR 0016). `oma consent` is what a user types to be asked ahead of time. It is not a dialog-less way to grant; `oma install --accept-permissions` stays the only one (ADR 0016).

`oma-consent` stays a separate binary that `oma` finds next to its own executable and runs, as ADR 0025 decided, and it does not become part of the `oma` binary: the init is `oma` re-executed (ADR 0024), so linking GPUI into `oma` would put the whole UI toolkit into every running application. Only the name and the `oma consent` entry point change. A crate with the same name holds the window code and uses the pinned GPUI directly (ADR 0016); it does not depend on ODK.

This amends ADR 0025 and ADR 0026. Everything in ADR 0025 about the request and answer, the fail-closed behaviour and why there is no daemon still holds.
