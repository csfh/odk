---
status: amended by ADR 0031, ADR 0032 and ADR 0052
---

# One Cargo workspace; the OMA crates and the ODK crates do not depend on each other

The repository is a single Cargo workspace. The OMA side has a small `.oma` format library (Manifest parsing and Bundle directory validation) that tools such as the Flea file explorer can depend on without pulling in omalaunch, plus the `omalaunch` binary. There is no packing tool, because a Bundle is just a directory (ADR 0022). The ODK crates sit in the same workspace but have no dependency on the OMA crates, matching ADR 0002 and ADR 0010: ODK apps can be packaged as Bundles, but neither side needs the other to build.
