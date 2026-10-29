---
status: accepted
---

# odk is released signed by its release root

ADR 0060, section 8, says how `odk` is installed and replaced. A person checks a signed `install.sh` against a release root they compare out of band. `odk oma self-upgrade` accepts a release only under the root compiled into the running binary, and only above the version in a receipt. Section 8 leaves four things open:
- what a release is;
- how a signed script vouches for the tarball it downloads;
- what the receipt holds;
- what self-upgrade runs.

ADR 0063's script still downloads from GitHub Releases and checks a `.sha256` file published beside the tarball. On 2026-10-27 the owner chose `odk.csfh.dev` on Cloudflare Pages as the host, and to keep the root key encrypted to their YubiKey. They asked for the work to be built with a test key, with their public key swapped in at the end (issue #15).

## Decision

- **The release root.** One Ed25519 key, distinct from every publisher root, held by the owner.
  - Its 32-byte public key is compiled into `odk-oma` as one constant.
  - No environment variable, Cargo feature or flag changes it. Any of those would be the flag section 8 forbids.
  - Until the owner's key is in, the constant holds none, and `self-upgrade` refuses because this build has no release root.
  - Tests give the library a fixture root through its arguments. The binary never takes one.
  - Releases are signed by the root itself. A release key would need a root-signed key set, and the root has to sign `install.sh` for every release anyway (section 8).
- **What a release is.** `https://odk.csfh.dev/` serves the current release as five files:
  - `install.sh` and `install.sh.sig`;
  - `release` and `release.sig`;
  - `odk-x86_64-linux.tar.gz`.

  The tarball was 12.7 MiB on 2026-10-29, against Cloudflare Pages' 25 MiB limit for one file. Its host is only transport: both signed documents name its digest and length.
  
  Only the current release is served, so the script has no option to install a named release, as ADR 0063's `--version TAG` did.
- **The script carries the tarball.** `tools/package-release` writes the release's version and the tarball's name, SHA-256 and length into `install.sh`.
  - The root signs the script with plain Ed25519 (`install.sh.sig`, 64 raw bytes), which a person verifies with OpenSSL 3 `pkeyutl` (section 8). `packaging/README.md` gives the commands, including the public key as PEM.
  - The script checks the tarball's length and SHA-256 against what it carries. There is no `.sha256` file.
  - It downloads with `curl --proto '=https' --tlsv1.2 -fsS`, with no `-L`.
- **The release document.** `release` is canonical JSON, at most 16 KiB, as `oma-keys` is (ADR 0060, section 3).
  - It carries `algorithm`, `root_key_id`, `version`, `issued_at`, and the tarball's name, `sha256` and `length`, and `install.sh`'s `sha256` and `length`.
  - `release.sig` is the root's Ed25519ctx signature over it, in the context `odk-oma release v1`. The script's signature does not stand in for it (section 8).
- **The version.** A release's version is a `u64`: the Unix seconds when it was packaged. `odk --version` still prints the workspace's semver (ADR 0064).
- **The receipt.** `runtime.toml`, at `~/.local/share/oma/runtime.toml` or `/var/lib/oma/runtime.toml`, holds the installed release's version and the root's key id. The script writes it.
  - The script refuses a version below the receipt's, so an old signed script cannot downgrade an install. The same version again is a repair.
- **`--system`.** The script installs for one user by default.
  - `--system` installs under `/usr`, as section 8 says: the programs, the user units with the system path, the type, the handler and the `binfmt_misc` line.
  - Effective uid 0 without `--system` writes nothing. `--system` without uid 0 writes nothing, and `sudo` is never called.
  - `ODK_DESTDIR` puts every file under another root, for tests. It moves where files go, and nothing about what is trusted.
- **`odk oma self-upgrade`.** The running binary decides everything.
  1. It refuses when pacman owns its own path.
  2. It fetches `release` and `release.sig` under ADR 0066's rules, verifies them under the compiled root, and requires a version above the receipt's. A document naming another root is refused.
  3. It fetches `install.sh` and the tarball. Each is capped at the length the document gives, over HTTPS with no redirect followed, within 30 seconds without progress. It checks both digests.
  4. It runs that `install.sh`, verified by digest, against a local copy of the tarball. The script is the one place that knows the file list, so there is one installer.
  
  A host that cannot be reached, a lower or equal version, and another root each leave the installed files and the receipt as they were.
- **Signing.** `odk-oma sign-release <dir>` reads the root's 32-byte seed on standard input, so `gpg --decrypt root.seed.gpg | target/release/odk-oma sign-release <dir>` keeps it off the disk. While this build has no release root, or another one, it prints the seed's public key, its fingerprint and the line that compiles it in, which is how the owner's key goes in.
  - It signs `release` and `install.sh` in `<dir>`.
  - It refuses to sign when the `odk-oma` in the tarball is not the binary running, or when that binary's compiled root is not the seed's public key. A release can then only ship a binary that trusts the key that signed it.

## Consequences

- ADR 0063 is amended. Its host, its `.sha256` file, its `--version TAG` and its refusal to run as root give way to this record. Its per-user paths, units and `--uninstall` stay.
- ADR 0066 is amended. A tarball and a script are fetched within the length their signed document gives, with a 30-second wait for progress in place of three seconds in all.
- ADR 0054's handoff to Omarchy's repository stands. Once the package exists, the script does not download a tarball.
- Until the owner's public key is compiled in, no release can be signed for real, and `self-upgrade` refuses on every install.
