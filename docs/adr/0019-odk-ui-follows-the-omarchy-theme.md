---
status: amended by ADR 0062
---

# The ODK UI layer follows the Omarchy theme and updates live

The ODK UI layer exposes Omarchy's current theme to applications and notifies them when it changes, so ODK applications look native and never hard-code colors. This is part of the UI layer's contract, not something each application implements. How the layer learns of a theme change is an implementation detail to be settled later; a watch on the theme the sandbox baseline exposes (ADR 0012) is the likely approach, because Omarchy's hooks run outside the sandbox and none are installed (ADR 0009).
