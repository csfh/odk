---
status: amended by ADR 0052
---

# Opening a Bundle from outside `~/Applications` offers Install, Run once or Cancel

When the user opens a Bundle directory that is not in `~/Applications`, for example one extracted into `~/Downloads`, omalaunch shows the consent dialog (ADR 0016) with the application's name and requested permissions. Install copies the directory into `~/Applications` (it does not move it, so the original is left alone), records the consent (ADR 0023), registers it and launches it. Run once launches it where it is, keeps no consent record and installs nothing. Its data directory is a private tmpfs inside the application's mount namespace that disappears when the application exits, so there is nothing to clean up, and it gets no `updates` permission because it has no installed identity. Cancel does nothing. Only Install writes anything to the user's home. Consent for Bundles that are already in `~/Applications` is governed by the consent record (ADR 0023), not by this flow.
