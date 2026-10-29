# Packaging

What an install puts down besides the programs, and how a release is made. Nothing here is run
from the repository except `tools/package-release`.

`install.sh` (ADR 0063, ADR 0086) installs for one user, with no root:

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

A release is built here, with no CI (ADR 0086):

1. `cargo build --release --locked -p odk-cli -p oma -p oma-consent -p odkd`.
2. `tools/third-party-licenses Cargo.toml target/release/THIRD-PARTY-LICENSES odk-cli oma oma-consent odkd` (ADR 0081).
3. `tools/package-release target/release <dir> <tag>`. It writes `odk-x86_64-linux.tar.gz`, and a copy of `install.sh` with
   the release's version (the Unix seconds now) and the tarball's length and SHA-256 written into it.
4. `gpg --decrypt <root seed>.gpg | target/release/odk-oma sign-release <dir>`. It reads the release root's 32-byte seed
   on standard input and writes `release` and `release.sig` (Ed25519ctx) and `install.sh.sig` (plain Ed25519). It prints
   the root's fingerprint and its public key as PEM, which go on the owner's own site, never on `odk.csfh.dev`. It refuses
   unless the tarball's `odk-oma` is the one running and that binary's compiled release root is the seed's key. Run
   `target/release/odk-oma` itself, not `odk oma`: the installed copy is not the one being released.
5. Upload the five files in `<dir>` to `https://odk.csfh.dev/`.

Until the owner's public key is compiled into `odk-oma`, step 4 refuses, and says what to compile in. To put it in, once:

1. `gpg --decrypt <root seed>.gpg | target/release/odk-oma sign-release <any dir>`. It refuses, and prints the root's
   key id, its fingerprint, its PEM and the line `const RELEASE_ROOT: Option<&str> = Some("…");`.
2. Put that line in `crates/oma/src/odk_release.rs`, in place of `None`, and change the test
   `this_build_has_no_release_root_until_the_owners_key_is_in`, which says there is none.
3. Publish the fingerprint and the PEM on the owner's own site, and the fingerprint in the repository's `README.md`.

The installer's test builds a tarball with the same script, so the layout cannot drift.

To check a download (ADR 0060, section 8), with the root's PEM saved as `odk-release-root.pem` from the owner's site:

```
openssl pkey -pubin -in odk-release-root.pem -outform DER | tail -c 32 | sha256sum
openssl pkeyutl -verify -pubin -inkey odk-release-root.pem -rawin -in install.sh -sigfile install.sh.sig
```

The first prints the fingerprint to compare with the one published out of band. The second must say `Signature
Verified Successfully`, and only then is `sh install.sh` run. The script names the tarball by length and SHA-256, so the
signature covers it too. `crates/oma/tests/release.rs` runs these two commands against a signed release.

Before the tag, `version` in `[workspace.package]` of the root `Cargo.toml` is set to the tag without its `v`, with
`Cargo.lock`, in a commit on `main`, and the built `odk --version` must say it (ADR 0064) before anything is uploaded.

`sh install.sh --system`, as root and never through `sudo`, installs the same release for every user under `/usr`
(ADR 0060, section 8; ADR 0086):

| Installed as | From |
|---|---|
| `/usr/bin/odk`, `/usr/lib/odk/odk-oma`, `oma-consent`, `odkd` | the programs |
| `/usr/lib/systemd/user/` | `systemd/user/`, with the programs named under `/usr/lib/odk` in place of `%h/.local/lib/odk`, enabled for every user with `systemctl --global` and started at each user's next login |
| `/usr/lib/binfmt.d/odk.conf` | `binfmt.d/odk.conf`, the `binfmt_misc` line that runs `/usr/lib/odk/odk-oma` with the path of a `.oma` file, registered by restarting `systemd-binfmt` |
| `/usr/share/mime/packages/oma.xml` | `mime/oma.xml`, the type `application/x-oma`, a sub-class of the raw disk image |
| `/usr/share/applications/oma.desktop` | `applications/oma.desktop`, the handler `Exec=/usr/bin/odk oma run %f` |
| `/usr/share/doc/odk/` | the licences |
| `/var/lib/oma/runtime.toml` | the release installed |

As root without `--system`, or with `--system` but not as root, it writes nothing. A per-user install puts down neither the
`binfmt_misc` line nor the handler, so the default for the type there is a disk image's, Disk Image Mounter, and
double-clicking a `.oma` file mounts it (ADR 0065). Both kinds keep the release's version in `runtime.toml`
(`~/.local/share/oma/runtime.toml` for one user) and refuse a release older than it, so an old signed script cannot take an
install back; the same release again repairs one. `ODK_DESTDIR` writes under another root, for the installer's tests,
which never run as root, never call `sudo` and never register `binfmt_misc` on this machine.
