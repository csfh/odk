# ODK and OMA

Two projects for Omarchy (Arch Linux, Hyprland, Wayland only):

- **ODK**, the Omarchy Development Kit: Rust applications with a GPUI interface that follows the user's Omarchy theme, and an embedded Luau machine for plugins. Applications depend on one `odk` crate.
- **OMA**, Omarchy Managed Application: a bundle format and `oma`, the program that verifies, installs, sandboxes and starts one. A Bundle is one `.oma` file (ADR 0052). The code still installs a directory (`<App>.oma/`) until it catches up. `oma` does not depend on ODK.

Pre-release: every crate is 0.0.0. Stable Rust (edition 2024), Linux x86_64.

## Where to start

| To | Read |
|---|---|
| Build an application | `examples/hello`, then `docs/design-language.md` |
| Move an application from the earlier design system | `docs/migration.md` |
| Add a component | `docs/contributing-components.md`, `docs/spec/component-checklist.md` |
| See every component | the gallery, `docs/gallery.md` |
| Know whether it is ready | `docs/release-readiness.md`, `CHANGELOG.md` |
| Understand a decision | `docs/adr/`, with `GLOSSARY.md` for the terms |
| Work with an agent in this repository | `CLAUDE.md` |

## Commands

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
tools/check-deps          # after changing a dependency
```

Looking at the gallery without opening a window on your desktop: `tools/headless-shot` and `tools/visual-scenarios` run it under a private headless compositor (`docs/gallery.md`). `tools/idle-cpu --all` is the gate for a change to the gallery.

## License

MIT. See [LICENSE](LICENSE). Copyright (c) 2026 Christoffer Hallas.

If you send a pull request or other contribution, you agree to the [CLA](CLA.md).
