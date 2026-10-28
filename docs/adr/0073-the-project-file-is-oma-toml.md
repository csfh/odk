---
status: accepted
---

# The project file is `oma.toml`

ADR 0051 named the file that tells `odk oma dev` how to build a project `dev.toml`. In a project's directory, beside `manifest.toml`, `Cargo.toml` and whatever else the project keeps, `dev.toml` does not say which tool reads it. On 2026-10-28 the user asked for `oma.toml`.

**Decision.** The project file is `oma.toml`, with the keys, paths and rules of ADR 0051 unchanged. It is never put in the Bundle; the Manifest, `manifest.toml`, is what ships. A directory with a `dev.toml` and no `oma.toml` is an error that says to rename it, so a project from before this does not get the bare "not found". A Manifest key in `oma.toml` (`identifier`, `name`, `version`, `description`, `icon`, `categories`, `publisher`, `permissions`, `daemon`) is an error that says it belongs in the manifest, which goes into the Bundle.

**The confusion it invites.** By convention `<tool>.toml` is a project's manifest (`Cargo.toml`, `pyproject.toml`), and here it is not: the two errors above are what keep a Manifest key out of `oma.toml` and stop `oma.toml` passing for the Manifest. `oma-dev.toml` was proposed as the name that says both whose file it is and that it is not shipped; the user chose `oma.toml`.

This amends ADR 0051's file name; the command, its order and its other rules stand.
