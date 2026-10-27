# Packaging

What an install puts down besides the programs, and how a release is made. Nothing here is run
from the repository except `tools/package-release`.

`install.sh` (ADR 0063) installs for one user, with no root:

| Installed as | From |
|---|---|
| `~/.local/bin/odk` | the `odk` program (`crates/odk-cli`) |
| `~/.local/lib/odk/odk-oma`, `oma-consent`, `odkd` | the programs behind it |
| `~/.config/systemd/user/odkd.socket`, `odkd.service` | `systemd/user/` |

It then enables `odkd.socket` for the user's session; the first connection starts `odkd`, and an
upgrade restarts a running one. `odkd.service` names the program with `%h`, so the units hold
no user name. `sh install.sh --uninstall` removes exactly these files.

A tag `v*` runs `.github/workflows/release.yml`, which builds the four programs and runs
`tools/package-release`, which makes `odk-x86_64-linux.tar.gz`, its `.sha256` and a copy of
`install.sh`; those three are the release. The installer's test builds a tarball with the same
script, so the layout cannot drift.

Before the tag, `version` in `[workspace.package]` of the root `Cargo.toml` is set to the tag
without its `v`, with `Cargo.lock`, in a commit on `main`. The workflow publishes nothing when the
built `odk --version` says another (ADR 0064), so `v0.1.0-rc.1` and `v0.1.0-rc.2`, which say
`0.0.0`, cannot be published again.

Text for a system install, which nothing installs yet (ADR 0053, ADR 0056, ADR 0060 on pull
request #9):

| File | Would be installed as |
|---|---|
| `binfmt.d/odk.conf` | `/usr/lib/binfmt.d/odk.conf`, the `binfmt_misc` line that runs `/usr/lib/odk/odk-oma` with the path of a `.oma` file; it needs root |
| `mime/oma.xml` | `/usr/share/mime/packages/oma.xml`, the type `application/x-oma` |
| `applications/oma.desktop` | `/usr/share/applications/oma.desktop`, the handler `Exec=/usr/bin/odk oma run %f` |

A per-user install rewrites that `Exec` to the home directory's `odk`. None of these three is
installed by `install.sh`, because nothing makes a single-file `.oma` yet and `odk oma` refuses
one.
