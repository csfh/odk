---
status: amended by ADR 0052
---

# omalaunch keeps the current and previous version, and never snapshots data

The Bundle in `~/Applications` is the current version. When a newer version replaces it, omalaunch moves the old directory to `~/.local/share/oma/previous/<identifier>/`, keeping one previous version, so rollback is swapping the two. Per-application data is shared across versions and is not snapshotted, so migrating data, and surviving a rollback, is the application's job. Deleting a Bundle from `~/Applications` removes its `.desktop` entry on the next scan but keeps its data, and any previous version, until the user runs an explicit purge, as macOS does.
