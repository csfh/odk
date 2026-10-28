# Implementing ADR 0060

Not normative. This is the order in which to build what [ADR 0060](adr/0060-a-bundles-publisher-is-the-pinned-root-key.md) decides. It decides nothing. If building shows a decision is wrong, write a new ADR. ADR 0060 wins over this file, over [`docs/plan.md`](plan.md), and over the two advisory reviews it cites. The Ed25519ctx crate is [ADR 0061](adr/0061-ed25519ctx-is-verified-with-crrl.md), which wins on that one point.

Checked first against the tree at `45310bd` on `cursor/adr-0060-bundle-trust-da7d`, and again against `main` at `5eee25c` (2026-10-27), which still installs a directory Bundle and verifies no signature. ADR 0056 is on `main` (pull request #17). There is no `impl/decisions.md` in the repository. Christoffer implements this. The phases are a suggested split of pull requests.

The two reviews of the superseded six-point model are [GPT-6.1-Sol](reviews/2026-10-25-signing-model-gpt-6.1-sol.md) and [Grok 4.7](reviews/2026-10-25-signing-model-grok-4.7.md), both 2026-10-25, both advisory.

The phases are GitHub issues [#10](https://github.com/csfh/odk/issues/10) to [#16](https://github.com/csfh/odk/issues/16). The issue is the canonical text for that phase. This file keeps the order, what the code does today, and the questions that are still open. The index is [below](#issues).

## What the code does today

| Area | What is in the tree |
|---|---|
| Crate | `oma` is one library and one binary (ADR 0031). `crates/oma/src/lib.rs` exposes `bundle`, `consent`, `daemon`, `dev`, `entry`, `flow`, `fmt`, `grants`, `install`, `layout`, `sandbox`, `userdirs`, `view`. `trust` (phase 1, issue #10) holds the header and prefix, canonical JSON, the signed manifest, `oma-keys`, key ids, Ed25519ctx, the sealed copy and the high-water rule; no command calls it yet. The binary is `odk-oma` (ADR 0056), and `odk` (`crates/odk-cli`) is the command on `PATH` that runs it. |
| Assembly | `bundle.rs::assemble` copies `manifest.toml`, the ELF, and an optional icon from beside the manifest into a staging directory, validates it with `fmt::Bundle::open`, and renames it to `<Name>.oma/`. It refuses to replace an existing target. There is no EROFS writer, header, signature, or monotonic release counter. |
| Format | `fmt/manifest.rs` parses TOML with `deny_unknown_fields`. `Manifest` has `identifier`, `name`, a semver `version`, `executable`, `permissions`, and optional `description`, `icon`, and `categories`. There is no `publisher` field. `docs/spec/manifest.md` already says `publisher` is required and that the code still accepts a Bundle without one. `fmt/bundle.rs::Bundle::open` canonicalizes a directory, so it follows a symlink to the directory, reads `manifest.toml`, and checks a contained executable (the executable bit and a minimal x86_64 ELF) and an optional png or svg icon. That opener is not a single-file verifier. `fmt::Identifier` and the permission vocabulary stay useful. Upgrade comparison in this code is still semver and Identifier. |
| Install | `install.rs::install` copies a directory tree and refuses an Identifier that is already installed (`AlreadyInstalled`). Grants are written only with `--accept-permissions` when permissions are missing. `installed` and `find` skip directories that do not open. There is no upgrade, no previous file, no `rollback`, and no `purge` under `crates/oma/src`. |
| Consent | Directory Bundles: `flow.rs` asks only when `grants::is_empty` is false, and `consent.rs::Request::new` reads the name, the semver and the Identifier from the Bundle. Signed files (phase 4, issue #13): `flow::admit` asks about a root not pinned for the app id even when nothing else would be. Run once writes nothing, Cancel stores a denial, a launch and `--register` do not ask again, `install` may, `--accept-permissions` never pins, and a host one edit from another root's is confirmed on its own first (`Button::Confirm`). `Request::new_root` and `Request::confusable` show the signed fields and the fetched URL, not the image's. The helper's answer is one button word. |
| Helper | `oma-consent` takes TOML, draws with GPUI, and prints one button word (ADR 0016, ADR 0033). `theme.rs` and `hyprland.rs` are appearance and placement. It holds no trust state. Tests stay headless. CLAUDE.md forbids opening this helper on the user's desktop. |
| Persistence | `grants.rs` stores permissions per Identifier in `grants/<identifier>.toml`: a staging file mode `0o600`, write, `sync_all`, rename. `layout.rs` builds `data/<identifier>`, `grants/<identifier>.toml`, and `inbox/<identifier>` from the Identifier alone, plus `~/Applications`. Those records are not pins. Pair-scoped paths come later; nothing here silently promotes an old grant file into a pin. Since phase 4, `grants::Pair` is a signed file's record at `grants/<root-key-id>/<app-id>.toml`: the pin, the host, consent, the high-water mark and a denial, and `Layout::pair_dir` is `apps/<root-key-id>/<app-id>/`. |
| Launch | `sandbox/launch.rs` resolves, preflights, and runs a directory Bundle. An intermediate process starts the init and exits (ADR 0034). The init executes `/app/<executable>` (ADR 0024). `sandbox/plan.rs` bind-mounts `bundle_root` read-only at `/app`. `sandbox/mounts.rs` bind-mounts with `MS_BIND\|MS_REC`, then `mount_setattr` sets `MOUNT_ATTR_NOSUID`, `MOUNT_ATTR_NODEV` unless the mount is a device mount, and `MOUNT_ATTR_RDONLY` when the mount is read-only. Landlock and seccomp stay the application sandbox. None of this verifies a signature or mounts EROFS. Keep this sandbox. ADR 0060 does not replace it. |
| CLI and packaging | `main.rs` implements `inspect`, `run`, `consent`, `install`, `dev`, `bundle` and `mount`, and it re-executes itself as `--init`. Every Bundle path `inspect`, `run`, `consent` and `install` open is a directory; `mount` (ADR 0065) shows a `.oma` file through udisks, read-only and `noexec`, and verifies nothing. `entry.rs` is the `binfmt_misc` entry: one regular-file argument is sealed, its envelope checked with phase 1, and refused either way (phase 2, issue #11). `packaging/` ships the ADR 0060 `binfmt_misc` line (`binfmt.d/odk.conf`), the MIME type, the desktop handler (`Exec=/usr/bin/odk oma run %f`) and `odkd`'s user units (ADR 0062); ADR 0044's units are gone. `install.sh` at the repository root is ADR 0063's: a GitHub release tarball checked by SHA-256, per user, with no `binfmt_misc` and no handler. Bringing it to section 8 belongs with phase 6. |

The order of a launch, once the pieces exist: the path, then a sealed copy, then the fixed header, the canonical manifest, the signature and the payload digest, then the pin and the version rule, then the image checks, then consent, then either a durable install or a mount, then the sandbox that already exists. `inspect`, registration, and "Show contents" use that same check of the sealed object.

## What ADR 0060 requires

| Decision | Work |
|---|---|
| 1 | An inert `OMA1` file. Root registers `binfmt_misc` as `:odk-oma:M:0:OMA1::/usr/lib/odk/odk-oma:` with an empty flags field, no `C` flag, and a fixed argument vector. No shell wrapper. A per-user install does not register `binfmt_misc`. The desktop handler calls `/usr/bin/odk oma run` (per-user, `~/.local/bin/odk oma run`). |
| 2 | The pin is the root public key. The fingerprint is SHA-256 of the 32 raw bytes, 64 lowercase hex digits. The key id is the first 8 bytes, 16 lowercase hex digits, recomputed and checked against the key. An application is the pair (root key id, app id). Data, the previous file, and the inbox are `~/.local/share/oma/apps/<root-key-id>/<app-id>/`. Consent, the pin, the high-water mark, and denials are `~/.local/share/oma/grants/<root-key-id>/<app-id>.toml`. A different root at the same URL is a new publisher and inherits nothing. NXDOMAIN on a stored host is a suspected name loss: installed applications keep running, and a fetch waits. Any other failed fetch is "could not be reached", with the same result for the pin. A file already in hand that verifies under a release key the pinned root still authorizes is an upgrade while the host does not resolve. |
| 3 | One offline root authorizes at most eight short-lived release keys. `oma-keys` is the exact, versioned, root-signed set at `https://<publisher>/.well-known/oma-keys`, with the raw 64-byte signature at `oma-keys.sig`. HTTPS, no redirects, 16 KiB, three-second timeout. An empty `keys` array is a failed fetch. A lower `version` is rejected. A higher `version` replaces the set. Keys are not unioned. Revocations persist across later key sets. A later revocation document from the same root can withdraw one. Rotation moves the named applications, or every application under that root when `app_ids` is `*`, onto the successor, and moves their data, permissions, previous file, inbox, and high-water mark with them. A new release key, a revocation, and a rotation are not the first-install prompt. |
| 4 | One canonical signed manifest of the twelve fields, in that order, and the 64-byte little-endian header in ADR 0060. Ed25519ctx with the context strings in that ADR. `header_sha256` is SHA-256 of bytes 0..64. `payload_sha256` is SHA-256 from offset 1024 to the end of the file. The file size is `1024 + payload length`. The manifest and its 64-byte signature fit in the prefix (`M + 128 <= 1024`). The high-water mark is the monotonic `u64`, per pair, in the grant file. A lower integer is refused. The same integer is accepted only when the payload digest is the one stored for that version. A higher integer is an upgrade when the rest of the ADR accepts the file. Uninstall from `~/Applications` does not clear the mark. `odk oma purge` does. `odk oma rollback` swaps the stored previous file and does not lower the mark. An expired manifest is refused at install and at upgrade. Launch of an installed application does not need the manifest to be inside its window and does not need the network. |
| 5 | Open `O_RDONLY\|O_NOFOLLOW`. The file must be a regular file. Copy into a memfd sealed with `F_SEAL_WRITE`, `F_SEAL_GROW`, `F_SEAL_SHRINK`, and `F_SEAL_SEAL`, or into a file in a directory mode `0700` that only the user running `odk-oma` can write, opened the same way and not writable after the copy. Verify that object. Mount and exec use that object for the life of the mount. The path is not opened again. No extended attribute and no stamp file is a verification result. |
| 6 | Parse and verify the header and the manifest before any EROFS code reads the image. The image is read by `odk-oma`'s own reader, from the sealed object, and only the layouts its writer emits (ADR 0067, amending this section): a device node, a fifo, a socket, a symlink, a setuid or setgid file, an extended attribute, compression, or a size past the header's payload length is a parser error. A launch unpacks the verified tree into a read-only, `nosuid`, `nodev` tmpfs at `/app` in the application's mount namespace. Nothing is extracted onto the host, and nothing is mounted from the image. |
| 7 | The dialog is built from signed fields and from the URL that was actually fetched. It shows the fingerprint in groups of eight, the app id, the monotonic version, and the publisher host in Unicode and in punycode. It does not read the image for a display name or an icon, and it does not mount the image to decide what to ask. Origin extended attributes are never a decision. The host of a fetched URL must be the signed `publisher`. A file from disk has no invented download host. First contact fetches `oma-keys` from that host. A public key inside the Bundle is not a trust anchor. A new root is Install, Run once, or Cancel. Run once stores neither the pin nor the set. Cancel stores a denial for that pair. A stored denial is not asked again by a launch, by `--register`, or by another pass over the same file. `odk oma install` of that file can ask again. `--accept-permissions` grants permissions for a root that is already the pin, and on a new root it exits non-zero and installs nothing. A confusable host (UTS #39, or edit distance 1 on the punycode form) needs its own confirmation before Install or Run once. `--register` holds a structurally valid file from an unpinned root at `~/.local/share/oma/held/<root-key-id>/<app-id>.oma` and does not launch it. A file that does not verify, a lower version, and an expired manifest are deleted, not held. |
| 8 | `install.sh` and `install.sh.sig` are downloaded to files, with no `-L`, and the script is run only after the person has checked the out-of-band release-root fingerprint and verified the signature. That check is plain Ed25519 via OpenSSL 3 `pkeyutl`, over the script bytes. It is not Ed25519ctx. The default install is per-user, at `~/.local/bin/odk`, `~/.local/lib/odk/odk-oma`, and `~/.local/lib/odk/oma-consent`. The script does not call `sudo`. Effective uid 0 without `--system` exits. `--system` without effective uid 0 exits. `odk oma self-upgrade` accepts a new binary only under the release root compiled into the running binary, with a monotonic version strictly above the receipt at `/var/lib/oma/runtime.toml` or `~/.local/share/oma/runtime.toml`. A different release root is not an upgrade. Files pacman owns are refused. ADR 0054's later handoff to Omarchy's repository stands. |
| 9 | Fail closed on an unknown algorithm, a missing pin where a pin is required, a digest mismatch, a parser error, a bad signature, a release key that is not in the pinned root's exact set on install or upgrade, and a download host that is not the signed publisher. The successful launch is the sealed object after it has verified under the pinned root, or, for Run once, under the root the dialog is showing. |

These stay as they are. One `oma` crate, no dependency on `odk-db` or on the ODK crates (ADR 0013, ADR 0031). The helper keeps using GPUI directly. The inner `manifest.toml` stays the Manifest of ADR 0006. The signed manifest is a different document. `fmt::Bundle` stays the directory opener until a later phase stops using it for the new file. Standard-error lines from `odk-oma` are prefixed `odk oma:`. Library errors do not carry that prefix.

ADR 0060 section 10 is not part of these phases: no timestamp role, no transparency log, no curated registry, and no new or removed sandbox grant.

A new fact that implementation measures, including how long hashing a representative Bundle takes, is recorded in `docs/checks.md`. The payload digest stays a whole-file digest until that measurement says it cannot, and a different construction would be a new ADR.

## Dependencies

ODK depends on nothing a stock Omarchy does not already install (the owner, 2026-10-27). No phase adds a system package: `erofs-utils` is not one of them (ADR 0067).

Add a crate when a production call uses it, then run `tools/check-deps` and update `crates/deps/Cargo.toml`. A trust dependency is used, not listed as a `cargo-machete` exception. Only `crates/deps` keeps the exceptions it already documents.

The workspace `rust-version` is `1.96`. No `forbid(unsafe_code)` attribute exists in the repository. `sandbox/mounts.rs` and the other sandbox modules already call `libc`. Any new `unsafe` stays in a small wrapper with a safety comment, next to `OwnedFd` or `File`.

What the lockfile holds today, and what `oma` does not yet depend on:

| Crate | Locked | Direct use |
|---|---|---|
| `sha2` | 0.10.9 and 0.11.1 | Neither version is a direct dependency of `oma`. A later change picks one version when a call needs it, and leaves the other alone. |
| `base64` | 0.22.1 | Locked, not a direct `oma` dependency. `oma-keys` and rotation use standard base64 of 32-byte keys. |
| `url` | 2.5.8 | Locked, not a direct `oma` dependency. |
| `serde_json` | 1.0.154 | Direct dependency of `oma-consent`, not of `oma`. |
| `libc` | 0.2.191 | Already a direct `oma` dependency. |
| `ureq` | absent | [ADR 0066](adr/0066-odk-oma-fetches-over-https-with-ureq.md). Version 3.5.0, `default-features = false`, features `rustls` and `platform-verifier`. Not in the lockfile until issue [#14](https://github.com/csfh/odk/issues/14) or [#15](https://github.com/csfh/odk/issues/15) has a call that uses it. |
| `crrl` | absent | [ADR 0061](adr/0061-ed25519ctx-is-verified-with-crrl.md). Version 0.9.0, `default-features = false`, features `std` and `ed25519`. Not in the lockfile until issue [#10](https://github.com/csfh/odk/issues/10) has a call that uses it. |
| `ed25519-dalek`, `openssl` | absent | Not the document verifier. `ed25519-dalek` 3.0.0's context API is Ed25519ph. The `openssl` crate's safe signer is plain Ed25519. `openssl-probe` appears transitively and is not a signature implementation. |

`install.sh` verification stays the OpenSSL 3 `pkeyutl` command in ADR 0060 section 8. That is plain Ed25519, and it is not `crrl`.

CI for every phase is the workflow in `.github/workflows/tests.yml`: `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo check --workspace --all-targets --locked` on the `rust-version` toolchain, `cargo audit` with no ignored vulnerability and with yanked crates denied, and `cargo machete`. This plan does not assert an audit result for a crate that has not been added. Headless tests only. Do not register `binfmt_misc` on the machine, and do not open a window on its desktop.

## Phases

Each phase leaves the commands it does not own behaving as they do today. A phase that would need an answer from [Open questions](#open-questions) stops at the ADR's text and does not pick the answer in code.

### 1. Trust core

**Issue.** [#10 oma trust: Trust core](https://github.com/csfh/odk/issues/10).

**Scope.** The header, the signed manifest, and `oma-keys`, as ADR 0060 specifies those bytes. Fingerprint and key id. Ed25519ctx sign and verify for the contexts the ADR names, with `crrl` as [ADR 0061](adr/0061-ed25519ctx-is-verified-with-crrl.md) decides. A sealed copy of a regular file by either of the two objects section 5 allows. Pure checks for the high-water rule in section 4. No network client, no mount, no GPUI, and no change to `inspect`, `run`, `install`, `consent`, or `bundle`.

Revocation bodies, rotation's version rule, an equal `oma-keys` version, a second signature, and the other open questions are not encoded as policy here. Constants for the four context strings can exist, because the ADR names them. A parser for a document whose schema is still open does not land in this phase.

**Modules.** A new `oma::trust` under `crates/oma/src/trust/`, declared from `lib.rs`. A split that fits is `container` (header and prefix), `manifest` (the twelve-field document), `keys` (`oma-keys` only), `signature`, and `sealed`. `fmt::Identifier` is reused. `SignedManifest` stays a different type from `fmt::Manifest`. This split is a layout, not a frozen API. No new crate.

**Tests.** Golden headers at the offsets in ADR 0060, including manifest length 896 and 897, a file whose size is not `1024 + payload length`, a non-zero reserved or padding byte, a truncated file, and the magic of an ELF and of a shebang. Manifest and `oma-keys` fixtures that are the canonical documents the ADR describes, plus duplicates, unknown fields, whitespace, a trailing byte, a bad key length, an empty `keys` array, more than eight release keys, a duplicate id, a key id that does not match the public key, a lower and a higher `oma-keys` version, and a signature under the wrong context string. The Ed25519ctx vectors in RFC 8032 section 7.2, through `crrl` as ADR 0061 requires, then fixtures for `odk-oma manifest v1` and `odk-oma keys v1`. Plain Ed25519 and a signature over a different context fail those two documents. Sealed-copy tests: symlink, directory, fifo, and socket are rejected; a write to the source after the copy does not change the sealed bytes; the memfd reports all four seals. A mode-`0700` store is also a successful sealed object, as section 5 allows. High-water: lower integer refused, equal integer only with the stored payload digest, higher integer accepted, semver string ignored. No test needs EROFS, FUSE, root, a window, or the network. Fixtures that would need a control character below U+0020 wait for question 5, so this phase does not pick a spelling.

**Acceptance.** `cargo test -p oma` covers the cases above. `odk oma`'s current commands are unchanged, because this phase does not wire them. The verifier does not claim the payload is a valid EROFS image or that `manifest.toml` matches. That is phase 3.

### 2. Private binary and inert entry

**Issue.** [#11 oma trust: Private binary and inert entry](https://github.com/csfh/odk/issues/11). Blocked by [#10](https://github.com/csfh/odk/issues/10).

**Scope.** The command names and paths of ADR 0056, together with the registration in ADR 0060 section 1. `odk` is the PATH command. `odk oma` is the front door. The private binary is `odk-oma` at `/usr/lib/odk/odk-oma` or `~/.local/lib/odk/odk-oma`. There is no `oma` command and no `/usr/lib/odk/binfmt` shell wrapper. A `binfmt_misc` invocation is `run` of the single path argument, with no shell. Until phases 3 and 4 can mount and ask, that entry verifies the envelope with phase 1 and then refuses to execute. Generating the drop-in and the desktop file is in scope. Registering `binfmt_misc` on the development machine is not.

ADR 0056's facade, the drop-in, the desktop handler and the refusing entry are on `main` already (pull request #17, issue #11's first part). What is left is the verification before the refusal, which needs phase 1. It does not redesign ADR 0056.

**Modules.** `crates/oma/src/main.rs`, the binary target in `crates/oma/Cargo.toml`, and the files under `packaging/` that ADR 0053 and ADR 0056 already name (MIME type, desktop entry, `binfmt` drop-in). `packaging/README.md` describes ADR 0063's install until phase 6.

**Tests.** The generated `binfmt` line is the line in ADR 0060 section 1. The desktop handler's command is `/usr/bin/odk oma run` or the per-user path, not a shell. A test invokes the private binary with one path argument and observes `run` of that path. Headless. No call to `systemd-binfmt` and no write under `/proc/sys/fs/binfmt_misc`.

**Acceptance.** The repository contains the drop-in text and the desktop command. Nothing in the test run registers an interpreter on the host. A `.oma` path does not execute payload bytes.

### 3. Image

**Issue.** [#12 oma trust: Image](https://github.com/csfh/odk/issues/12). Blocked by [#10](https://github.com/csfh/odk/issues/10).

**Scope.** After phase 1 has verified the sealed object, read the EROFS image with `odk-oma`'s own reader (ADR 0067), which accepts only the layouts phase 7's writer emits. `app_id`, `entry`, and `publisher` match `manifest.toml`. The executable check that `fmt::bundle` already does for a directory applies to the image's entry. Device nodes, fifos, sockets, symlinks, setuid or setgid files, extended attributes, compression, and a size past the payload length are parser errors. At launch the verified tree is written into a tmpfs at `/app` in the application's mount namespace, which is then made read-only, `nosuid` and `nodev`; the sealed object is closed after the copy. Nothing is mounted from the image, and there is no unmount helper. The view mount stays ADR 0065's. Permission lines for the dialog are question 12. This phase does not feed the dialog.

**Modules.** An EROFS reader (`oma::image`, or under `oma::trust`) and the tmpfs step beside `sandbox/mounts.rs`. The application sandbox in `plan.rs`, `landlock.rs`, `seccomp.rs`, and `init.rs` stays. `/app` is the tmpfs.

**Tests.** A fixture image that verifies, and fixtures that fail the `manifest.toml` match, the size cap, a device node, a fifo, a symlink, a setuid bit and a compressed file. Fixtures are built by hand in the tests until phase 7's writer exists. The bytes the reader parses are the sealed object's, not a second open of the source path. A launch's `/app` is read-only, `nosuid` and `nodev` and holds exactly the image's tree. No test mounts on the user's session.

**Acceptance.** An unverified object never reaches the EROFS reader. The tree at `/app` is the verified object's. The phase does not claim the consent dialog can list permissions.

### 4. Consent and pair-scoped state

**Issue.** [#13 oma trust: Consent and pair-scoped state](https://github.com/csfh/odk/issues/13). Blocked by [#10](https://github.com/csfh/odk/issues/10).

**Scope.** The paths in ADR 0060 section 2. The dialog fields in section 7. A new root is asked even when the permission set is empty, which is the change from `flow.rs` today. Run once writes neither the pin nor the key set. Cancel stores a denial. A stored denial is not re-asked by launch, `--register`, or another pass. `odk oma install` of that file may ask. `--accept-permissions` does not pin a new root. The confusable warning is a separate confirmation, and one answer cannot both dismiss it and install. Consent for a second app id under a root the user has already pinned names both ids. Old grant files keyed only by Identifier are not treated as pins.

How permission lines are read is question 12. Until an ADR answers it, the dialog shows the signed identity and the fetched host, and it does not invent a permission source.

**Modules.** `layout.rs`, `grants.rs`, `consent.rs`, `flow.rs`, and `crates/oma-consent`. The helper protocol gains the signed fields. It still returns one button word, plus the separate confusable confirmation.

**Tests.** Pair isolation: the same Identifier under two roots does not share a grant or a data directory. Empty permissions still produce the new-root dialog. Run once leaves no pin. A denial suppresses launch and `--register` and does not suppress `odk oma install`. `--accept-permissions` on a new root exits non-zero. A confusable host keeps Install and Run once unavailable until the warning is confirmed on its own. The helper is driven headlessly, as the existing consent tests are. No window on the user's desktop.

**Acceptance.** The dialog contents are the signed fields and the fetched URL. Name and icon are not read from the image. A grant file written by today's code is not a pin.

### 5. Install, fetch, run, registration

**Issue.** [#14 oma trust: Install, fetch, run, registration](https://github.com/csfh/odk/issues/14). Blocked by [#11](https://github.com/csfh/odk/issues/11), [#12](https://github.com/csfh/odk/issues/12), and [#13](https://github.com/csfh/odk/issues/13).

**Scope.** First contact fetches `oma-keys` and `oma-keys.sig` over HTTPS, without following redirects, with a three-second timeout and a 16 KiB cap. The host of that URL is the signed `publisher`, or the operation stops before consent. A file opened from disk does not consult `user.xdg.origin.url`. Install, upgrade, inbox, held files, rollback, and purge follow sections 2, 4, and 7. The high-water mark lives in the grant file. Rollback swaps the stored previous file and does not lower the mark. Purge removes the grant, the data, and the mark. An already-held file that verifies under an authorized release key installs while the host does not resolve. NXDOMAIN and any other failed fetch produce the two different standard-error lines in section 2, and neither changes the pin. Launch of an installed application re-checks the sealed object's signature, digests, and pin, including past `expires_at` and without a fresh `oma-keys`, and writes the one line section 4 requires when the manifest is past its window. `--register` does not open a dialog. It holds an unpinned root's valid file and deletes a file that does not verify, a lower version, and an expired manifest. Background registration does not delete arbitrary paths. A revoked or expired release key does not authorize a new install or upgrade, and it does not stop an application that is already installed.

**Modules.** `install.rs`, `flow.rs`, `main.rs` (the subcommands the CLI spec names for this work), and a fetch module under `oma::trust` or `oma`. `sandbox/launch.rs` runs the sealed object from phase 3 after phase 4's consent.

**Tests.** Redirect, timeout, non-200, and a body over 16 KiB are failed fetches. A host mismatch stops before the dialog. Disk open ignores origin attributes. High-water, equal-version digest match, held path, offline upgrade of a file already in hand, rollback, purge, and the installed-launch exceptions. A revocation that is absent from a later `oma-keys` is still stored. Withdrawal itself waits for question 2, because the document that says "withdrawn" has no schema yet. The test can store a revocation the phase-1 types can represent only if question 2 has been decided. Until then, the phase tests the rule the ADR states with a fixture whose body is supplied as already-verified bytes, and it does not define the JSON.

**Acceptance.** Directory copying is no longer how a new-format file is installed. Identifier-only paths are no longer how a new-format application is resolved. The sandbox still cannot write the grant and data paths (ADR 0012).

### 6. Bootstrap and self-upgrade

**Issue.** [#15 oma trust: Bootstrap and self-upgrade](https://github.com/csfh/odk/issues/15). Blocked by [#11](https://github.com/csfh/odk/issues/11).

**Scope.** ADR 0060 section 8, and the README in `packaging/` brought into line with it. The documented commands download `install.sh` and `install.sh.sig` to files. Verification is plain Ed25519, OpenSSL 3 `pkeyutl`, against the out-of-band fingerprint. The script defaults to the per-user paths, never calls `sudo`, and honours the uid rules for `--system`. `odk oma self-upgrade` checks the release root compiled into the running binary and a strictly higher monotonic version in the receipt outside the binary. A different release root is refused, with no flag that accepts one. Pacman-owned files are refused. The Omarchy repository handoff in ADR 0054 stays. The user units' `ExecStart` becomes the `odk` path this phase installs.

**Modules.** `packaging/` (script, units, README) and the self-upgrade command on the private binary. The release-root public key is compiled in. It is a different key from every publisher root.

**Tests.** A bad signature does not run the script. uid 0 without `--system`, and `--system` without uid 0, write nothing and do not invoke `sudo`. A lower or equal version, a different release root, an unreachable install host, and a pacman-owned path do not replace the binary. The receipt path is the one in section 8. Tests do not download from the network and do not install onto the machine.

**Acceptance.** The script text in the repository matches section 8. Self-upgrade is decided by the binary that is already running.

### 7. Producer and examples

**Issue.** [#16 oma trust: Producer and examples](https://github.com/csfh/odk/issues/16). The writer is blocked by [#10](https://github.com/csfh/odk/issues/10). An example that executes a Bundle is also blocked by [#14](https://github.com/csfh/odk/issues/14).

**Scope.** `odk oma bundle` writes one `.oma` file: an EROFS image from `odk-oma`'s own writer (ADR 0067: 4 KiB blocks, root-owned inodes with mtime 0, a zero UUID, the name as the label, directories and regular files only, uncompressed) and the phase-1 header, manifest, and signature. No program is run. `--sign` takes the path of a 32-byte ed25519 seed. The seed is not written into the image, the sandbox, or the consent record. There is no unsigned output. The manifest carries `publisher` and the monotonic `version`. `examples/hello` and `examples/todo`, and the notes that tell a reader how to build their Bundles, produce a real single-file Bundle once phase 5 can run one. Fixture keys stay fixtures. An offline root seed is not an example release.

This phase can start the writer after phase 1. An example that is meant to be executed waits until phase 5.

**Modules.** `crates/oma/src/bundle.rs`, `examples/hello`, `examples/todo`, and the example README.

**Tests.** The writer rejects a manifest that does not fit in the prefix, a missing seed, and an output that does not verify under phase 1. Round-trip: bytes the writer emits are bytes phase 1 and phase 3's reader accept, and the same inputs give the same bytes. CI mounts a written image with the kernel's EROFS driver, since the runner has root and a loop device. Example keys are marked as fixtures.

**Acceptance.** `bundle` no longer emits a directory as its success path. Existing directory-Bundle unit tests are either updated in this phase or kept, explicitly, as coverage of the old opener until nothing calls it.

## Open questions

Question 1 is decided by ADR 0061. The rest are not settled by ADR 0060. An implementation pull request does not pick an answer for those. Where a test would need one, the test waits, and the gap stays in this list. The numbers are stable: issues [#10](https://github.com/csfh/odk/issues/10) to [#16](https://github.com/csfh/odk/issues/16) cite them. Several were proposals in the planning note this file was checked against. Those proposals are not adopted.

1. **Ed25519ctx in process.** Decided by [ADR 0061](adr/0061-ed25519ctx-is-verified-with-crrl.md). `odk-oma` uses `crrl` 0.9.0, `sign_ctx` and `verify_ctx`, with `default-features = false` and features `std` and `ed25519`. `ed25519-dalek` 3.0.0's `with_context` is Ed25519ph, and the `openssl` crate's safe signer is plain Ed25519. `install.sh` stays plain Ed25519 via OpenSSL 3 `pkeyutl`. The number stays, because issues [#10](https://github.com/csfh/odk/issues/10) to [#16](https://github.com/csfh/odk/issues/16) cite these questions by number.

2. **Revocation JSON.** The ADR says the document names release key ids, that a verified revocation is kept even when a later `oma-keys` omits the id or lists the key again, and that withdrawal is a later document from the same root that names the id and says it is withdrawn. It does not give the fields, their order, a version counter, a size limit, or the spelling of "withdrawn". A planning note proposed `algorithm`, `version`, and `keys` entries of `id` plus a `withdrawn` boolean, at most eight, sorted, with equal versions required to be the same bytes. Not adopted.

3. **Rotation encoding and `version`.** The fields are `algorithm`, `version`, `expires_at`, `successor`, and `app_ids` (an array of Identifiers, or `*`). The document verifies under the current root and is rejected past `expires_at`. Unlike the signed manifest and `oma-keys`, the ADR does not say this field list is exclusive and ordered, does not say `version` is a high-water mark, and does not say a named array is sorted or free of duplicates. It does not say whether a rotation that has already been applied is still history after `expires_at`. A planning note proposed those extra rules. Not adopted.

4. **The same `oma-keys` version.** A lower version is rejected. A higher version replaces the exact set. The same version is not specified.

5. **Control-escape spelling.** Canonical strings escape the quotation mark, the reverse solidus, and code points below U+0020. The ADR does not choose among the JSON spellings of a control. A planning note recommended lowercase `\u00xx` for U+0000..U+001F and literal UTF-8 for every other non-ASCII character. Not adopted. Phase 1 fixtures avoid code points below U+0020 so they do not depend on a spelling.

6. **Algorithm policy and a second signature.** The context string `odk-oma algorithm v1` is named. During a transition a file carries both signatures over the same manifest bytes, each with its own context, and a binary that does not know one algorithm rejects the file. The v1 header holds one 64-byte signature. The policy document's fields, and where the second signature is stored, are not specified. These phases do not implement a second algorithm.

7. **Time boundaries and the clock.** `expires_at` is later than `issued_at`. An expired manifest is refused at install and upgrade. A release key used for install or upgrade is not past its `not_after`. The ADR does not say whether an `issued_at` in the future is refused, whether the instant equal to `expires_at` or `not_after` is already past, or which clock supplies "now". A planning note added `NotYetValid` and treated equality as expired. Not adopted.

8. **A zero payload.** The file size is `1024 + payload length`. A payload length of zero is not discussed. A planning note required `P > 0`. Not adopted.

9. **A file-size limit beyond the header.** The image size cap is the authenticated payload length. A planning note proposed a default 1 GiB limit before hashing. That number is not in the ADR.

10. **Duplicate public keys.** Duplicate key ids in `oma-keys` are a failed fetch. Two entries with different ids and the same 32 public-key bytes are not mentioned.

11. **Key-id collision.** Paths use the 16-hex-digit key id. The fingerprint is the identity. Two different 32-byte keys with the same key id have no rule. A planning note treated that as an error and refused to share a directory. Not adopted.

12. **Permission lines.** The dialog shows the fingerprint, the app id, the monotonic version, and the host in Unicode and punycode. It does not read a display name or an icon from the image, and it does not mount the image to decide what to ask. Install still records consent for the permissions in the Manifest, and those permissions live in `manifest.toml` inside the image (ADR 0006). The ADR does not say how the lines are obtained. A planning note suggested a bounded read of `manifest.toml` from the authenticated image, without mounting, and only for permissions. Not adopted. Phase 4 shows the signed identity without inventing that read.

13. **The mode-0700 store.** Section 5 allows a sealed memfd or a file in a directory mode `0700` that only the user running `odk-oma` can write, not writable after the copy. It does not say what "not writable after the copy" requires of that file, or whether a memfd that cannot be sealed must fail or must use the store. A planning note made a seal failure fatal and allowed no other file. That is narrower than the ADR and is not adopted. Phase 1 accepts either object the ADR names.

14. **Grant-file schema.** The path and the facts are fixed: consent, the pin, the high-water mark, denials. The TOML fields are not. Whether the grant file must keep the signed `oma-keys` body from install time, so a later launch can show why that release key was accepted, is not said. Launch still re-checks the signature, the digests, and the pin on the sealed object. It does not treat a rewritten grant file as a reason to skip those checks.

15. **UTS #39.** The warning applies when the publisher host is a Unicode confusable (UTS #39) of a host stored for a different pinned root, or within edit distance 1 of that host's punycode form. The ADR does not name a Unicode version, a confusable skeleton, or a restriction level.

16. **Left open by ADR 0060 section 10.** A timestamp role and a transparency log. Any change to app confinement. A curated registry as an optional extra root. The reviews argued for an append-only log and, in places, for a threshold of roots. Section 3 uses one root. These phases do not build section 10.

17. **Raised by the reviews and not mentioned in ADR 0060.** Requiring Certificate Transparency SCTs on the `oma-keys` fetch. Sending `Cache-Control: no-store` and ignoring HTTP cache freshness. Publisher-side DNSSEC, CAA, and a registrar lock, which are not client behaviour. These phases do not add them.


## Issues

The issue is the canonical text. This table is the index.

| Phase | Issue | Blocked by |
|---|---|---|
| 1. Trust core | [#10 oma trust: Trust core](https://github.com/csfh/odk/issues/10) | nothing |
| 2. Private binary and inert entry | [#11 oma trust: Private binary and inert entry](https://github.com/csfh/odk/issues/11) | #10, and [PR #6](https://github.com/csfh/odk/pull/6) for the command names |
| 3. Image | [#12 oma trust: Image](https://github.com/csfh/odk/issues/12) | #10 |
| 4. Consent and pair-scoped state | [#13 oma trust: Consent and pair-scoped state](https://github.com/csfh/odk/issues/13) | #10 |
| 5. Install, fetch, run, registration | [#14 oma trust: Install, fetch, run, registration](https://github.com/csfh/odk/issues/14) | #11, #12, #13 |
| 6. Bootstrap and self-upgrade | [#15 oma trust: Bootstrap and self-upgrade](https://github.com/csfh/odk/issues/15) | #11 |
| 7. Producer and examples | [#16 oma trust: Producer and examples](https://github.com/csfh/odk/issues/16) | #10 for the writer; #14 for an example that executes a Bundle |
