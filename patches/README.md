# Patches

Local copies of third-party crates with a minimal fix, wired in through `[patch.crates-io]`. Each stays only until the upstream dependency chain no longer needs it.

| Patch | Why |
|---|---|
| `xattr` (0.2.3) | `gpui` 0.2.2 reaches it through `zed-async-tar`. It uses `libc::ENOATTR`, which current `libc` no longer defines on Linux, so it does not compile. The patch maps it to `ENODATA`, the Linux error for a missing attribute. The only change is in `src/sys/mod.rs`. |
