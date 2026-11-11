---
status: accepted
---

# ODK runs Plugins in Luau through mlua

ADR 0003 chose Luau through `mlua` with `Lua::sandbox()`, and ADR 0020 has a Plugin declare capabilities that the host grants. The owner put Plugins in v1 on 2026-11-11. No crate in the tree binds Luau yet, only `spikes/mlua-limits`. `mlua` would be a new dependency, so it is checked as `ureq` was (ADR 0066) before any `odk-plugin` code is written. The owner's rule of 2026-10-27 applies: ODK depends on nothing a stock Omarchy does not already install, crates are compiled in, and a new one is checked. This record is that check. It does not design the Plugin API.

## Decision

`odk-plugin` binds Luau with [`mlua` 0.12.2](https://docs.rs/mlua/0.12.2/mlua/), with the `luau` feature and no other, and creates every state with `Lua::sandbox(true)`. Luau's source is compiled in by `luau0-src`, so nothing is installed beside ODK and no system Lua or Luau is used. The crate is behind the `plugin` feature of the `odk` facade (ADR 0028), off by default, as `odk::db` is (ADR 0049).

`luau0-src` is `0.22.0+luau740`. `mlua-sys` 0.13.0 asks for `^0.22.0`, and 0.22.1 was yanked, so that is the version Cargo resolves. A newer Luau (`0.23.0+luau745`, 2026-11-03) needs a newer `mlua-sys`, and moving to it is a version bump to take when a release asks for it, since Luau's own fixes are what the sandbox stands on.

## Why mlua

Checked on 2026-11-11:

- **It is maintained.** `mlua` was first published on 2019-11-04, and 0.12.2 on 2026-10-03, after 0.12.1 (2026-08-29) and 0.12.0 (2026-07-05). `mlua-sys` 0.13.0 is also from 2026-10-03, and `luau0-src` 0.23.0 from 2026-11-03. They are [`mlua-rs/mlua`](https://github.com/mlua-rs/mlua) and [`mlua-rs/luau-src-rs`](https://github.com/mlua-rs/luau-src-rs).
- **It is used.** crates.io counts 8.7 million downloads of `mlua`, 2.7 million of them recent, 7.2 million of `mlua-sys` and 969 thousand of `luau0-src`.
- **The licence is MIT** for `mlua`, `mlua-sys` and `luau0-src`, the same as ODK's.
- **No advisory.** `crates/mlua`, `crates/mlua-sys` and `crates/luau0-src` do not exist in `rustsec/advisory-db` (the three paths answer 404, and a crate that has advisories, `smallvec`, answers 200 from the same query). A `cargo audit` of the whole tree, run by hand since CI was removed (2026-10-29), is the check for a later one.
- **It uses `unsafe`, as an FFI crate must.** `mlua` does not forbid or deny `unsafe_code` (`src/lib.rs` at v0.12.2 has neither attribute), unlike `ureq`. So the Luau sandbox is only as strong as Luau's sandbox mode and what the host puts in the state (ADR 0003: Plugins run in the application's process, and that sandbox is their only boundary). This is a reason to keep the API a Plugin gets small, and to test hostile Plugins (`docs/roadmap.md`, Track E), not a reason against the crate.
- **Little that is new.** `spikes/mlua-limits/Cargo.lock` has 32 packages beside the spike; 29 are already in the workspace's `Cargo.lock` at the same version. The three that are not are `mlua`, `mlua-sys` and `luau0-src`.
- **Nothing to install, and a compiler to build.** `luau0-src` builds Luau's C++ with `cc`, so a build needs a C++ compiler (`g++` and `base-devel` are installed on the development machine; whether a stock Omarchy has them was not checked, and only a build needs them). Running ODK needs nothing. The spike builds in release in 28 s on the development machine, and its whole binary (`mlua`, Luau and a small program) is 2.7 MB, 2.3 MB stripped.

## What was not checked

- **Other embeddable VMs.** ADR 0003 chose Luau over PUC Lua for its sandbox and types; no other VM was compared here, and this record does not reopen that.
- **Luau's own security record** beyond what ADR 0003 and `docs/checks.md` ("Luau limits through `mlua`") already hold: a memory limit stops a runaway allocation, an interrupt stops `while true do end`, and an interrupt does not fire inside a long call into C code.

## What this does not decide

The Plugin API of each capability, isolation between Plugins (a Lua state each, or removing `getfenv` and `setfenv`), memory and time limits, and error reporting stay open in `docs/spec/plugin-manifest.md`. The next ADR settles them (`docs/roadmap.md`, Track E, step 2). The spike found that the sandbox leaves `require` in place, so the host must replace it.

## Consequences

- Adding `mlua` runs `tools/check-deps` and updates `crates/deps/Cargo.toml` with what it prints. It is not a `cargo-machete` exception.
- `Cargo.lock` gains `mlua`, `mlua-sys` and `luau0-src`, which a `cargo audit` of the tree covers.
- ADR 0003 and ADR 0020 are unchanged. This record names the binding and its checks.
