---
status: amended by ADR 0032
---

# omalaunch has a narrow scope: it runs Bundles, nothing else

omalaunch verifies, installs, registers, sandboxes and starts valid Bundles. It does not build applications, host Plugins, or check for or download updates. Updating is the responsibility of the application itself, whether that is an ODK app, an Electron app or anything else, because omalaunch knows nothing about how a Bundle's program was built (ADR 0002). This departs from the original brief's "automatic updates built into the ecosystem" in exchange for a runner that stays small and has no network behaviour of its own.
