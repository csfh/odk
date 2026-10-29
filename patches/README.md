# Patches

Local copies of third-party crates with a minimal fix, wired in through `[patch.crates-io]`. Each stays only until the upstream dependency chain no longer needs it.

| Patch | Why |
|---|---|
| `xattr` (0.2.3) | `gpui` 0.2.2 reaches it through `zed-async-tar`. It uses `libc::ENOATTR`, which current `libc` no longer defines on Linux, so it does not compile. The patch maps it to `ENODATA`, the Linux error for a missing attribute. The only change is in `src/sys/mod.rs`. |
| `gpui` (0.2.2) | Text that truncates (`text_ellipsis`, ODK's `truncate_end`) was drawn in two fonts when one layout measured it twice, narrower and then wider: `truncate_line` shortens the runs it is given in place, and the measure closure in `src/elements/text.rs` kept the one `runs` vector across measures, so the wider measure shaped the whole text with the shortened runs and everything after them fell back to another font (Atmos's dialog placeholders showed `SUP` in the font and `ER + F` in another). The patch gives each measure its own copy; it is the only change, in `src/elements/text.rs`. The published crate's `examples/`, the `[[example]]` entries in its `Cargo.toml` that name them, and its `Cargo.lock` are left out (ADR 0079). Drop it when ODK moves to a `gpui` with the fix. |
