---
status: amended by ADR 0029
---

# GPUI is pinned and wrapped by an ODK UI layer, with a raw escape hatch

GPUI is developed inside the Zed repository and its API changes frequently. ODK pins GPUI to a specific commit and exposes its own UI abstraction on top, so most application code does not depend on GPUI types. ODK also provides a raw mode that exposes GPUI directly for code the abstraction does not cover. The pin is a single workspace-level dependency that the omalaunch consent dialog also uses directly (ADR 0016). The cost is maintaining the wrapper; the benefit is that GPUI upgrades are absorbed in one place.
