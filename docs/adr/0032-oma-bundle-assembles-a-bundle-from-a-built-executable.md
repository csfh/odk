---
status: amended by ADR 0050
---

# `oma bundle` assembles a Bundle from a built executable and a Manifest

`oma` has a command that creates `<Name>.oma/` from a built executable and a `manifest.toml`: `oma bundle <executable> --manifest <manifest.toml> [-o <dir>]`. It copies the Manifest, puts the executable at the path the Manifest names and sets its executable bit, copies the icon the Manifest names from beside it, validates the result like any Bundle, and moves it into place only if valid. It never replaces an existing Bundle. It compiles nothing, knows nothing about Cargo or how the executable was made, and does no archiving or signing, so ADR 0010's rule that `oma` does not build applications still holds.

This amends ADR 0013 ("There is no packing tool, because a Bundle is just a directory") and ADR 0022, which listed a packing tool among the things the directory form removes. A Bundle is still just a directory and can be made by hand; the command only spares authors the copying and catches a mistake at once instead of at install time. We considered a separate `oma-bundle` program that `oma bundle` delegates to, and a `cargo oma bundle` subcommand, to keep any build knowledge out of `oma`. Neither is needed while the command does not compile; a Cargo-aware wrapper can be added later, in `oma` or ODK, without changing this one.

Extra resource files are copied into the directory by hand afterwards, and `oma inspect` checks the result again.
