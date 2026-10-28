---
status: accepted
---

# The version is the workspace's, and a release's tag says the same

ADR 0063 left every crate at `0.0.0` and made the release tag what `install.sh` reports, so the pre-releases `v0.1.0-rc.1` and `v0.1.0-rc.2` installed an `odk` whose `odk --version` said `0.0.0`, and `odk oma --version` the same. The owner asked on 2026-10-27 for that to be fixed. This record is the choice made on their behalf, and is open to change.

There is one version, `version` in `[workspace.package]` of the root `Cargo.toml`, and every crate under `crates/` takes it (`version.workspace = true`). The examples are applications and keep their own. It is the version of the next release, now `0.1.0-rc.3`, and `odk --version`, `odk oma --version` and the crates' metadata all say it. A release sets it first, with `Cargo.lock`, in a commit on `main`, and then tags that commit with `v` and the version. `.github/workflows/release.yml` builds, runs the built `odk --version`, and publishes nothing when it does not say the tag without its `v`. `tools/package-release` still writes the tag as `VERSION`, so what `install.sh` reports and what `odk --version` says agree.

## Considered Options

- **The version from the tag at build time** (an environment variable read with `option_env!`, or `git describe`). No commit before a tag, but `Cargo.toml` would still say `0.0.0` and a local build would print a version no release has: two sources where one will do.
- **The check in `tools/package-release`.** The installer's test packs the real `odk` as release `1.2.3` and others, and a check there would refuse them.

## Conflicts with earlier records

- ADR 0063 is amended: its last consequence, that `odk --version` prints the crates' `0.0.0`, no longer holds. The rest of it stands.

## Consequences

- `crates/odk-cli/tests/version.rs` fails when a crate has a version of its own, and when `odk --version` is not the workspace's.
- A tag made before this, `v0.1.0-rc.1` or `v0.1.0-rc.2`, says `0.0.0`, so the workflow's `workflow_dispatch` can no longer publish it again.
- A project that builds on ODK by path, as Atmos does, rewrites its `Cargo.lock` when ODK's version moves.
