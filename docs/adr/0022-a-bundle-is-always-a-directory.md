---
status: amended by ADR 0032
---

# A Bundle is always a directory, `<App>.oma/`, never a file

A Bundle is a directory named `<App>.oma/` holding the Manifest, the ELF and its resources. There is no archive or single-file form, and omalaunch does not read archives. The installed Bundle lives at `~/Applications/<App>.oma/` and runs in place. The Flea file explorer presents the directory as one application, with a "Show contents" action to browse it. Linux has no opaque-directory mechanism, so other file managers show a plain folder that opens on double-click instead of running; it still works, but is not launched by double-clicking. How an author transports a Bundle (zip, tar, an app's own installer) is not OMA's concern. This follows macOS `.app`, and was chosen over a tar+zstd file (ADR 0008) because it removes extraction, ephemeral directories and a packing tool.

## Consequences

Running a Bundle needs no extraction, so "Run once" runs the directory where it is and there is nothing to clean up (ADR 0018). A file-name MIME association cannot attach to a directory, so `.oma` is not registered as a MIME type (ADR 0014). A directory can be modified in place, so a future signature (ADR 0007) must cover per-file content hashes stored inside the directory.
