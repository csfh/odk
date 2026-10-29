---
status: accepted
---

# Releases and Bundles carry their third-party licences

ODK's programs and the applications built on it are made of hundreds of crates: 576 for the four programs of a release, under MIT, Apache 2.0, BSD, Zlib, ISC, Unicode, CC0, MPL 2.0 and others. On top of those, odk-ui embeds Remix Icon (ADR 0080). Most of these licences ask that their text travel with a copy, and Apache 2.0 asks for its NOTICE files too. A release tarball and a Bundle carried neither. The owner asked for them on 2026-10-29.

**Decision.** Every release and every Bundle carries the licences of what is compiled into it.

- **The tool.** `tools/third-party-licenses MANIFEST_PATH OUT PACKAGE...` writes them as one text file. It lists every crate the shipped packages reach through normal dependencies on x86_64 Linux, from `cargo metadata --locked --offline`; dev and build dependencies are left out, and so are the workspace's own members.
  - **What each crate gets.** Its name, version, licence expression and repository, then its licence and notice files. A crate that links native code is searched deeper, for the bundled library's licence (FreeType, zlib, libgit2, ring's BoringSSL).
  - **Path dependencies.** One with no file of its own takes the nearest one above it in its repository, which is how ODK's crates, seen from an application, are under ODK's `LICENSE`.
  - **Repeats and gaps.** A text several crates ship is printed once. A crate that ships no file is listed with its expression.
  - **Reproducible.** The file is the same bytes for the same `Cargo.lock`.
- **Assets.** What a package embeds that is not a crate is declared in its manifest under `[package.metadata.odk] third-party`, with the files that hold its licence, and the tool prints those first. odk-ui declares Remix Icon, so every application that links odk-ui lists it.
- **A release.** `package-release` takes the file from beside the programs (the tool writes it there; `package-release` itself runs no `cargo`). It puts the file and ODK's `LICENSE` under `odk/share/doc/odk/`, and `install.sh` installs them to `~/.local/share/doc/odk/` and removes them on uninstall. A release from before has none, and installs as before. The four programs do not link odk-ui, so a release lists no icons.
- **A Bundle.** The application builds the file in its `oma.toml` and includes it, with its own licence, as Atmos does under `share/licenses/`. `odk oma bundle` stays as it is (ADR 0032): it assembles what it is given and knows nothing of Cargo.

**Considered options.**

- **`cargo-about` or `cargo-deny`.** Either does this and more, but would be a tool to install beside a stock Omarchy and Rust for one file. The script is short, and its rules are written down here.
- **Links instead of texts.** A licence that asks for its text to travel is not satisfied by a URL.
- **Collecting licences in `odk oma bundle`.** It would have to learn Cargo, against ADR 0032's narrow scope.
