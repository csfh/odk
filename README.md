# ODK and OMA

Two projects for Omarchy (Arch Linux, Hyprland, Wayland only):

- **ODK**, the Omarchy Development Kit: Rust applications with a GPUI interface that follows the user's Omarchy theme, and an embedded Luau machine for plugins. Applications depend on one `odk` crate.
- **OMA**, Omarchy Managed Application: a bundle format and `oma`, the program that verifies, installs, sandboxes and starts one. A Bundle is one `.oma` file (ADR 0052). The code still installs a directory (`<App>.oma/`) until it catches up. `oma` does not depend on ODK.

Pre-release: every crate is 0.0.0. Stable Rust (edition 2024), Linux x86_64.

## Where to start

| To | Read |
|---|---|
| Build an application | `stories/button.rs` (a whole window in a page of code), then `docs/design-language.md` |
| Move an application from the earlier design system | `docs/migration.md` |
| Add a component | `docs/contributing-components.md`, `docs/spec/component-checklist.md` |
| See a component | `stories/` (one story per component, so far `button` and `item_group`); the owner runs one with `tools/story NAME` |
| Know whether it is ready | `docs/release-readiness.md`, `CHANGELOG.md` |
| Understand a decision | `docs/adr/`, with `GLOSSARY.md` for the terms |
| Work with an agent in this repository | `CLAUDE.md` |

## Install

The signed install (ADR 0060, section 8; ADR 0086) is not published yet: no release has been signed with the odk release root. Once one is, `install.sh` and `install.sh.sig` are downloaded from `https://odk.csfh.dev` and checked against the root's fingerprint as `packaging/README.md` says, before `sh install.sh`.

Until then, `v0.1.0-rc.3` is on GitHub, installed by its own, unsigned script (ADR 0063):

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://github.com/csfh/odk/releases/download/v0.1.0-rc.3/install.sh | sh
```

Either puts `odk`, `odk oma` and the `odkd` daemon under your home directory; no root.

## Commands

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
tools/check-deps          # after changing a dependency
```

Looking at a story without opening a window on your desktop: `tools/headless-shot` runs it under a private headless compositor, and `tools/idle-cpu` measures what it costs while idle.

## License

MIT. See [LICENSE](LICENSE). Copyright (c) 2026 Christoffer Hallas.

If you send a pull request or other contribution, you agree to the [CLA](CLA.md).
