---
status: amended by ADR 0025
---

# The omalaunch consent dialog uses GPUI directly, not through ODK

omalaunch asks the user to consent whenever a Bundle requests a permission missing from its consent record (ADR 0023): at first install or first launch, and when an upgrade adds permissions. The dialog is a small GPUI window that uses the pinned GPUI directly, because the OMA crates must not depend on ODK (ADR 0013). A non-interactive mode fails closed: with no dialog, a permission missing from the consent record makes omalaunch exit non-zero and launch nothing. The only way to grant without a dialog is `omalaunch install --accept-permissions`, which records consent for exactly the permissions the Bundle requests at that moment, never for a later upgrade's additions. We rejected a terminal prompt, which is poor when launched from a launcher, and a notification with actions, which is weak for a long permission list.
