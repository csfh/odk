# Packaging

What an install puts down besides the programs, and how a release is made. Nothing here is run
from the repository except `tools/package-release`.

`install.sh` (ADR 0063) installs for one user, with no root:

| Installed as | From |
|---|---|
| `~/.local/bin/odk` | the `odk` program (`crates/odk-cli`) |
| `~/.local/lib/odk/odk-oma`, `oma-consent`, `odkd` | the programs behind it |
| `~/.config/systemd/user/odkd.socket`, `odkd-files.socket`, `odkd.service` | `systemd/user/` |
| `~/.local/share/mime/packages/oma.xml` | `mime/oma.xml` |
| `~/.local/share/doc/odk/LICENSE`, `THIRD-PARTY-LICENSES` | the repository's `LICENSE`, and what `tools/third-party-licenses` writes for the four programs (ADR 0081) |

It then enables `odkd.socket` and `odkd-files.socket` for the user's session; the first
connection to either starts `odkd`, which tells the two apart by their names (`host` and
`files`, ADR 0068), and an upgrade restarts a running one. A release from before the file
chooser has no `odkd-files.socket`, and the script then enables `odkd.socket` alone. It runs `update-mime-database` on the user's MIME directory, so a
file manager knows a `.oma` file as a kind of disk image (ADR 0065). The units follow
`XDG_CONFIG_HOME` and the type follows `XDG_DATA_HOME`. `odkd.service` names the program with `%h`, so the units hold
no user name. `sh install.sh --uninstall` removes exactly these files.

A release is built here, with no CI: `cargo build --release --locked -p odk-cli -p oma -p oma-consent -p odkd`, then
`tools/third-party-licenses Cargo.toml target/release/THIRD-PARTY-LICENSES odk-cli oma oma-consent odkd` (ADR 0081), then
`tools/package-release target/release <dir> <tag>`, which makes `odk-x86_64-linux.tar.gz`, its `.sha256` and a copy of
`install.sh`; those three are the release, uploaded with `gh release create <tag> <dir>/*`. The installer's test builds a
tarball with the same script, so the layout cannot drift.

Before the tag, `version` in `[workspace.package]` of the root `Cargo.toml` is set to the tag without its `v`, with
`Cargo.lock`, in a commit on `main`, and the built `odk --version` must say it (ADR 0064) before anything is uploaded.

Text for a system install, which nothing installs yet (ADR 0053, ADR 0056, ADR 0060 on pull
request #9):

| File | Would be installed as |
|---|---|
| `binfmt.d/odk.conf` | `/usr/lib/binfmt.d/odk.conf`, the `binfmt_misc` line that runs `/usr/lib/odk/odk-oma` with the path of a `.oma` file; it needs root |
| `mime/oma.xml` | `/usr/share/mime/packages/oma.xml`, the type `application/x-oma`, a sub-class of the raw disk image |
| `applications/oma.desktop` | `/usr/share/applications/oma.desktop`, the handler `Exec=/usr/bin/odk oma run %f` |

A per-user install rewrites that `Exec` to the home directory's `odk`. `install.sh` installs the
type for the user, and not the `binfmt_misc` line or the handler, because nothing makes a
single-file `.oma` yet and `odk oma run` refuses one. Until the handler is installed, the
default for the type is a disk image's, Disk Image Mounter, so double-clicking a `.oma` file
mounts it (ADR 0065).
