---
status: amended by ADR 0053
---

# Bundles are unsigned in v1, with room left for signatures

The first version of `.oma` has no signature verification: every Bundle is treated as unsigned. A signature can be added later as a file inside the Bundle directory covering per-file content hashes (ADR 0022), but no trust model (developer keys, trust on first use, or a registry) is chosen yet. Until one exists, the permission prompt at install is the only trust gate. Authenticating updates is the application's concern (ADR 0010), not omalaunch's.
