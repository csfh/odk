# Implementing ADR 0060

Not normative. This is the order in which to build what [ADR 0060](adr/0060-a-bundles-publisher-is-the-pinned-root-key.md) decides. It decides nothing. If building shows a decision is wrong, write a new ADR. ADR 0060 wins over this file, over [`docs/plan.md`](plan.md), and over the two advisory reviews it cites.

Checked against the tree at `45310bd` on `cursor/adr-0060-bundle-trust-da7d`, which still installs a directory Bundle and verifies no signature. [ADR 0056](https://github.com/csfh/odk/pull/6) is pull request #6 and is not in this branch. There is no `impl/decisions.md` in the repository. Christoffer implements this. The phases are a suggested split of pull requests.

The two reviews of the superseded six-point model are [GPT-6.1-Sol](reviews/2026-10-25-signing-model-gpt-6.1-sol.md) and [Grok 4.7](reviews/2026-10-25-signing-model-grok-4.7.md), both 2026-10-25, both advisory.

The seven phases were not filed as GitHub issues. `csfh/odk` had no issues, and this agent can read the repository but cannot create issues. The texts under [Issue texts](#issue-texts) are the issues that would have been opened, one per phase, using the existing `enhancement` label, with no assignee and no milestone. Each phase links to its text. They are not issue numbers. File them by hand if they should exist on GitHub.

## What the code does today

| Area | What is in the tree |
|---|---|
| Crate | `oma` is one library and one binary (ADR 0031). `crates/oma/src/lib.rs` exposes `bundle`, `consent`, `flow`, `fmt`, `grants`, `hyprland`, `install`, `layout`, `omarchy`, `sandbox`, `userdirs`. There is no `trust` module. The binary name is the package name, `oma`. ADR 0056's `odk` facade and private `odk-oma` target are not on this branch. |
| Assembly | `bundle.rs::assemble` copies `manifest.toml`, the ELF, and an optional icon from beside the manifest into a staging directory, validates it with `fmt::Bundle::open`, and renames it to `<Name>.oma/`. It refuses to replace an existing target. There is no EROFS writer, header, signature, or monotonic release counter. |
| Format | `fmt/manifest.rs` parses TOML with `deny_unknown_fields`. `Manifest` has `identifier`, `name`, a semver `version`, `executable`, `permissions`, and optional `description`, `icon`, and `categories`. There is no `publisher` field. `docs/spec/manifest.md` already says `publisher` is required and that the code still accepts a Bundle without one. `fmt/bundle.rs::Bundle::open` canonicalizes a directory, so it follows a symlink to the directory, reads `manifest.toml`, and checks a contained executable (the executable bit and a minimal x86_64 ELF) and an optional png or svg icon. That opener is not a single-file verifier. `fmt::Identifier` and the permission vocabulary stay useful. Upgrade comparison in this code is still semver and Identifier. |
| Install | `install.rs::install` copies a directory tree and refuses an Identifier that is already installed (`AlreadyInstalled`). Grants are written only with `--accept-permissions` when permissions are missing. `installed` and `find` skip directories that do not open. There is no upgrade, no previous file, no `rollback`, and no `purge` under `crates/oma/src`. |
| Consent | `flow.rs` asks only when `grants::is_empty` is false. An empty permission request skips the dialog on run, on install, and on `oma consent` (`consent` then returns `Ok(false)`). `consent.rs::Request::new` reads the name, the semver, and the Identifier from the directory Bundle. The helper's answer is one button word. There is no fingerprint, monotonic version, publisher host, fetched-host check, stored denial, or separate confusable confirmation. |
| Helper | `oma-consent` takes TOML, draws with GPUI, and prints one button word (ADR 0016, ADR 0033). `theme.rs` and `hyprland.rs` are appearance and placement. It holds no trust state. Tests stay headless. CLAUDE.md forbids opening this helper on the user's desktop. |
| Persistence | `grants.rs` stores permissions per Identifier in `grants/<identifier>.toml`: a staging file mode `0o600`, write, `sync_all`, rename. `layout.rs` builds `data/<identifier>`, `grants/<identifier>.toml`, and `inbox/<identifier>` from the Identifier alone, plus `~/Applications`. Those records are not pins. Pair-scoped paths come later; nothing here silently promotes an old grant file into a pin. |
| Launch | `sandbox/launch.rs` resolves, preflights, and runs a directory Bundle. An intermediate process starts the init and exits (ADR 0034). The init executes `/app/<executable>` (ADR 0024). `sandbox/plan.rs` bind-mounts `bundle_root` read-only at `/app`. `sandbox/mounts.rs` bind-mounts with `MS_BIND\|MS_REC`, then `mount_setattr` sets `MOUNT_ATTR_NOSUID`, `MOUNT_ATTR_NODEV` unless the mount is a device mount, and `MOUNT_ATTR_RDONLY` when the mount is read-only. Landlock and seccomp stay the application sandbox. None of this verifies a signature or mounts EROFS. Keep this sandbox. ADR 0060 does not replace it. |
| CLI and packaging | `main.rs` implements `inspect`, `run`, `consent`, `install`, `omarchy sync`, and `bundle`, and it re-executes itself as `--init`. The module comment that says only `inspect` and `bundle` exist is stale. Every Bundle path those commands open is a directory. `packaging/` ships the two Omarchy user units from ADR 0044 and a README. The README still describes ADR 0054's domain script and units that run `/usr/bin/oma`. It does not ship a MIME type, a desktop file, a `binfmt_misc` drop-in, or the ADR 0060 install script. Updating that README belongs with phase 6. |

The order of a launch, once the pieces exist: the path, then a sealed copy, then the fixed header, the canonical manifest, the signature and the payload digest, then the pin and the version rule, then the image checks, then consent, then either a durable install or a mount, then the sandbox that already exists. `inspect`, registration, and "Show contents" use that same check of the sealed object.

## What ADR 0060 requires

| Decision | Work |
|---|---|
| 1 | An inert `OMA1` file. Root registers `binfmt_misc` as `:odk-oma:M:0:OMA1::/usr/lib/odk/odk-oma:` with an empty flags field, no `C` flag, and a fixed argument vector. No shell wrapper. A per-user install does not register `binfmt_misc`. The desktop handler calls `/usr/bin/odk oma run` (per-user, `~/.local/bin/odk oma run`). |
| 2 | The pin is the root public key. The fingerprint is SHA-256 of the 32 raw bytes, 64 lowercase hex digits. The key id is the first 8 bytes, 16 lowercase hex digits, recomputed and checked against the key. An application is the pair (root key id, app id). Data, the previous file, and the inbox are `~/.local/share/oma/apps/<root-key-id>/<app-id>/`. Consent, the pin, the high-water mark, and denials are `~/.local/share/oma/grants/<root-key-id>/<app-id>.toml`. A different root at the same URL is a new publisher and inherits nothing. NXDOMAIN on a stored host is a suspected name loss: installed applications keep running, and a fetch waits. Any other failed fetch is "could not be reached", with the same result for the pin. A file already in hand that verifies under a release key the pinned root still authorizes is an upgrade while the host does not resolve. |
| 3 | One offline root authorizes at most eight short-lived release keys. `oma-keys` is the exact, versioned, root-signed set at `https://<publisher>/.well-known/oma-keys`, with the raw 64-byte signature at `oma-keys.sig`. HTTPS, no redirects, 16 KiB, three-second timeout. An empty `keys` array is a failed fetch. A lower `version` is rejected. A higher `version` replaces the set. Keys are not unioned. Revocations persist across later key sets. A later revocation document from the same root can withdraw one. Rotation moves the named applications, or every application under that root when `app_ids` is `*`, onto the successor, and moves their data, permissions, previous file, inbox, and high-water mark with them. A new release key, a revocation, and a rotation are not the first-install prompt. |
| 4 | One canonical signed manifest of the twelve fields, in that order, and the 64-byte little-endian header in ADR 0060. Ed25519ctx with the context strings in that ADR. `header_sha256` is SHA-256 of bytes 0..64. `payload_sha256` is SHA-256 from offset 1024 to the end of the file. The file size is `1024 + payload length`. The manifest and its 64-byte signature fit in the prefix (`M + 128 <= 1024`). The high-water mark is the monotonic `u64`, per pair, in the grant file. A lower integer is refused. The same integer is accepted only when the payload digest is the one stored for that version. A higher integer is an upgrade when the rest of the ADR accepts the file. Uninstall from `~/Applications` does not clear the mark. `odk oma purge` does. `odk oma rollback` swaps the stored previous file and does not lower the mark. An expired manifest is refused at install and at upgrade. Launch of an installed application does not need the manifest to be inside its window and does not need the network. |
| 5 | Open `O_RDONLY\|O_NOFOLLOW`. The file must be a regular file. Copy into a memfd sealed with `F_SEAL_WRITE`, `F_SEAL_GROW`, `F_SEAL_SHRINK`, and `F_SEAL_SEAL`, or into a file in a directory mode `0700` that only the user running `odk-oma` can write, opened the same way and not writable after the copy. Verify that object. Mount and exec use that object for the life of the mount. The path is not opened again. No extended attribute and no stamp file is a verification result. |
| 6 | Parse and verify the header and the manifest before any EROFS code reads the image. `erofsfuse` runs as the user, `ro,nosuid,nodev`, with the header's payload length as the size cap, with no setuid helper. Before exec, refuse a device node, a fifo, or a setuid or setgid file. Extract nothing. The per-launch mount, the view mount, and the unmount helper stay as ADR 0052 and ADR 0053 describe them, pointed at the sealed object. |
| 7 | The dialog is built from signed fields and from the URL that was actually fetched. It shows the fingerprint in groups of eight, the app id, the monotonic version, and the publisher host in Unicode and in punycode. It does not read the image for a display name or an icon, and it does not mount the image to decide what to ask. Origin extended attributes are never a decision. The host of a fetched URL must be the signed `publisher`. A file from disk has no invented download host. First contact fetches `oma-keys` from that host. A public key inside the Bundle is not a trust anchor. A new root is Install, Run once, or Cancel. Run once stores neither the pin nor the set. Cancel stores a denial for that pair. A stored denial is not asked again by a launch, by `--register`, or by another pass over the same file. `odk oma install` of that file can ask again. `--accept-permissions` grants permissions for a root that is already the pin, and on a new root it exits non-zero and installs nothing. A confusable host (UTS #39, or edit distance 1 on the punycode form) needs its own confirmation before Install or Run once. `--register` holds a structurally valid file from an unpinned root at `~/.local/share/oma/held/<root-key-id>/<app-id>.oma` and does not launch it. A file that does not verify, a lower version, and an expired manifest are deleted, not held. |
| 8 | `install.sh` and `install.sh.sig` are downloaded to files, with no `-L`, and the script is run only after the person has checked the out-of-band release-root fingerprint and verified the signature. That check is plain Ed25519 via OpenSSL 3 `pkeyutl`, over the script bytes. It is not Ed25519ctx. The default install is per-user, at `~/.local/bin/odk`, `~/.local/lib/odk/odk-oma`, and `~/.local/lib/odk/oma-consent`. The script does not call `sudo`. Effective uid 0 without `--system` exits. `--system` without effective uid 0 exits. `odk oma self-upgrade` accepts a new binary only under the release root compiled into the running binary, with a monotonic version strictly above the receipt at `/var/lib/oma/runtime.toml` or `~/.local/share/oma/runtime.toml`. A different release root is not an upgrade. Files pacman owns are refused. ADR 0054's later handoff to Omarchy's repository stands. |
| 9 | Fail closed on an unknown algorithm, a missing pin where a pin is required, a digest mismatch, a parser error, a bad signature, a release key that is not in the pinned root's exact set on install or upgrade, and a download host that is not the signed publisher. The successful launch is the sealed object after it has verified under the pinned root, or, for Run once, under the root the dialog is showing. |

These stay as they are. One `oma` crate, no dependency on `odk-db` or on the ODK crates (ADR 0013, ADR 0031). The helper keeps using GPUI directly. The inner `manifest.toml` stays the Manifest of ADR 0006. The signed manifest is a different document. `fmt::Bundle` stays the directory opener until a later phase stops using it for the new file. Standard-error lines from `odk-oma` are prefixed `odk oma:`. Library errors do not carry that prefix.

ADR 0060 section 10 is not part of these phases: no timestamp role, no transparency log, no curated registry, and no new or removed sandbox grant.

A new fact that implementation measures, including how long hashing a representative Bundle takes, is recorded in `docs/checks.md`. The payload digest stays a whole-file digest until that measurement says it cannot, and a different construction would be a new ADR.

## Dependencies

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
| `ed25519-dalek`, `openssl` | absent | Not in the workspace lockfile. `openssl-probe` appears transitively and is not a signature implementation. |

The in-process Ed25519ctx backend is [open question 1](#open-questions). This plan does not add one. `install.sh` verification is the OpenSSL 3 command in ADR 0060 section 8, which is separate from that crate. A planning note said `ed25519-dalek` 2.2.0's public context API is Ed25519ph rather than Ed25519ctx. That claim was not re-checked against crates.io for this file, and it is not a choice of backend.

CI for every phase is the workflow in `.github/workflows/tests.yml`: `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo check --workspace --all-targets --locked` on the `rust-version` toolchain, `cargo audit` with no ignored vulnerability and with yanked crates denied, and `cargo machete`. This plan does not assert an audit result for a crate that has not been added. Headless tests only. Do not register `binfmt_misc` on the machine, and do not open a window on its desktop.

## Phases

Each phase leaves the commands it does not own behaving as they do today. A phase that would need an answer from [Open questions](#open-questions) stops at the ADR's text and does not pick the answer in code.

### 1. Trust core

**Issue text.** [oma trust: Trust core](#issue-oma-trust-trust-core). Not filed.

**Scope.** The header, the signed manifest, and `oma-keys`, as ADR 0060 specifies those bytes. Fingerprint and key id. Ed25519ctx sign and verify for the contexts the ADR names, once question 1 has an ADR. A sealed copy of a regular file by either of the two objects section 5 allows. Pure checks for the high-water rule in section 4. No network client, no mount, no GPUI, and no change to `inspect`, `run`, `install`, `consent`, or `bundle`.

Revocation bodies, rotation's version rule, an equal `oma-keys` version, a second signature, and the other open questions are not encoded as policy here. Constants for the four context strings can exist, because the ADR names them. A parser for a document whose schema is still open does not land in this phase.

**Modules.** A new `oma::trust` under `crates/oma/src/trust/`, declared from `lib.rs`. A split that fits is `container` (header and prefix), `manifest` (the twelve-field document), `keys` (`oma-keys` only), `signature`, and `sealed`. `fmt::Identifier` is reused. `SignedManifest` stays a different type from `fmt::Manifest`. This split is a layout, not a frozen API. No new crate.

**Tests.** Golden headers at the offsets in ADR 0060, including manifest length 896 and 897, a file whose size is not `1024 + payload length`, a non-zero reserved or padding byte, a truncated file, and the magic of an ELF and of a shebang. Manifest and `oma-keys` fixtures that are the canonical documents the ADR describes, plus duplicates, unknown fields, whitespace, a trailing byte, a bad key length, an empty `keys` array, more than eight release keys, a duplicate id, a key id that does not match the public key, a lower and a higher `oma-keys` version, and a signature under the wrong context string. RFC 8032 Ed25519ctx vectors once a backend exists, then fixtures for `odk-oma manifest v1` and `odk-oma keys v1`. Plain Ed25519 and a signature over a different context fail those two documents. Sealed-copy tests: symlink, directory, fifo, and socket are rejected; a write to the source after the copy does not change the sealed bytes; the memfd reports all four seals. A mode-`0700` store is also a successful sealed object, as section 5 allows. High-water: lower integer refused, equal integer only with the stored payload digest, higher integer accepted, semver string ignored. No test needs EROFS, FUSE, root, a window, or the network. Fixtures that would need a control character below U+0020 wait for question 5, so this phase does not pick a spelling.

**Acceptance.** `cargo test -p oma` covers the cases above. `odk oma`'s current commands are unchanged, because this phase does not wire them. The verifier does not claim the payload is a valid EROFS image or that `manifest.toml` matches. That is phase 3.

### 2. Private binary and inert entry

**Issue text.** [oma trust: Private binary and inert entry](#issue-oma-trust-private-binary-and-inert-entry). Not filed.

**Scope.** The command names and paths of ADR 0056, together with the registration in ADR 0060 section 1. `odk` is the PATH command. `odk oma` is the front door. The private binary is `odk-oma` at `/usr/lib/odk/odk-oma` or `~/.local/lib/odk/odk-oma`. There is no `oma` command and no `/usr/lib/odk/binfmt` shell wrapper. A `binfmt_misc` invocation is `run` of the single path argument, with no shell. Until phases 3 and 4 can mount and ask, that entry verifies the envelope with phase 1 and then refuses to execute. Generating the drop-in and the desktop file is in scope. Registering `binfmt_misc` on the development machine is not.

If pull request #6 has not landed on this branch, this phase includes that facade or stacks on it. It does not redesign ADR 0056.

**Modules.** `crates/oma/src/main.rs`, the binary target in `crates/oma/Cargo.toml`, and the files under `packaging/` that ADR 0053 and ADR 0056 already name (MIME type, desktop entry, `binfmt` drop-in). The Omarchy user units stay on `/usr/bin/oma` until phase 6. `packaging/README.md` still describes the ADR 0054 script until that phase.

**Tests.** The generated `binfmt` line is the line in ADR 0060 section 1. The desktop handler's command is `/usr/bin/odk oma run` or the per-user path, not a shell. A test invokes the private binary with one path argument and observes `run` of that path. Headless. No call to `systemd-binfmt` and no write under `/proc/sys/fs/binfmt_misc`.

**Acceptance.** The repository contains the drop-in text and the desktop command. Nothing in the test run registers an interpreter on the host. A `.oma` path does not execute payload bytes.

### 3. Image

**Issue text.** [oma trust: Image](#issue-oma-trust-image). Not filed.

**Scope.** After phase 1 has verified the sealed object, check the EROFS image. `app_id`, `entry`, and `publisher` match `manifest.toml`. The executable check that `fmt::bundle` already does for a directory applies to the image's entry. `erofsfuse` runs as the user, `ro,nosuid,nodev`, capped at the payload length, with no setuid helper. Device nodes, fifos, and setuid or setgid files are refused before exec. Nothing is extracted. The mount and the exec use the sealed object. The per-launch mount, the view mount, and the unmount helper follow ADR 0052 and ADR 0053. Permission lines for the dialog are question 12. This phase does not feed the dialog.

**Modules.** A mount path beside `sandbox/mounts.rs`. The application sandbox in `plan.rs`, `landlock.rs`, `seccomp.rs`, and `init.rs` stays. The image is mounted from the sealed object, and `/app` is that mount.

**Tests.** A fixture image that verifies, and fixtures that fail the `manifest.toml` match, the size cap, a device node, a fifo, and a setuid bit. The file descriptor `erofsfuse` receives is the sealed object's, not a second open of the source path. Orphaned-mount behaviour stays the one ADR 0052 describes. No test mounts on the user's session. A FUSE test that the environment cannot run is recorded in `docs/checks.md` rather than skipped silently.

**Acceptance.** An unverified object never reaches `erofsfuse`. A verified object is the one that is mounted. The phase does not claim the consent dialog can list permissions.

### 4. Consent and pair-scoped state

**Issue text.** [oma trust: Consent and pair-scoped state](#issue-oma-trust-consent-and-pair-scoped-state). Not filed.

**Scope.** The paths in ADR 0060 section 2. The dialog fields in section 7. A new root is asked even when the permission set is empty, which is the change from `flow.rs` today. Run once writes neither the pin nor the key set. Cancel stores a denial. A stored denial is not re-asked by launch, `--register`, or another pass. `odk oma install` of that file may ask. `--accept-permissions` does not pin a new root. The confusable warning is a separate confirmation, and one answer cannot both dismiss it and install. Consent for a second app id under a root the user has already pinned names both ids. Old grant files keyed only by Identifier are not treated as pins.

How permission lines are read is question 12. Until an ADR answers it, the dialog shows the signed identity and the fetched host, and it does not invent a permission source.

**Modules.** `layout.rs`, `grants.rs`, `consent.rs`, `flow.rs`, and `crates/oma-consent`. The helper protocol gains the signed fields. It still returns one button word, plus the separate confusable confirmation.

**Tests.** Pair isolation: the same Identifier under two roots does not share a grant or a data directory. Empty permissions still produce the new-root dialog. Run once leaves no pin. A denial suppresses launch and `--register` and does not suppress `odk oma install`. `--accept-permissions` on a new root exits non-zero. A confusable host keeps Install and Run once unavailable until the warning is confirmed on its own. The helper is driven headlessly, as the existing consent tests are. No window on the user's desktop.

**Acceptance.** The dialog contents are the signed fields and the fetched URL. Name and icon are not read from the image. A grant file written by today's code is not a pin.

### 5. Install, fetch, run, registration

**Issue text.** [oma trust: Install, fetch, run, registration](#issue-oma-trust-install-fetch-run-registration). Not filed.

**Scope.** First contact fetches `oma-keys` and `oma-keys.sig` over HTTPS, without following redirects, with a three-second timeout and a 16 KiB cap. The host of that URL is the signed `publisher`, or the operation stops before consent. A file opened from disk does not consult `user.xdg.origin.url`. Install, upgrade, inbox, held files, rollback, and purge follow sections 2, 4, and 7. The high-water mark lives in the grant file. Rollback swaps the stored previous file and does not lower the mark. Purge removes the grant, the data, and the mark. An already-held file that verifies under an authorized release key installs while the host does not resolve. NXDOMAIN and any other failed fetch produce the two different standard-error lines in section 2, and neither changes the pin. Launch of an installed application re-checks the sealed object's signature, digests, and pin, including past `expires_at` and without a fresh `oma-keys`, and writes the one line section 4 requires when the manifest is past its window. `--register` does not open a dialog. It holds an unpinned root's valid file and deletes a file that does not verify, a lower version, and an expired manifest. Background registration does not delete arbitrary paths. A revoked or expired release key does not authorize a new install or upgrade, and it does not stop an application that is already installed.

**Modules.** `install.rs`, `flow.rs`, `main.rs` (the subcommands the CLI spec names for this work), and a fetch module under `oma::trust` or `oma`. `sandbox/launch.rs` runs the sealed object from phase 3 after phase 4's consent.

**Tests.** Redirect, timeout, non-200, and a body over 16 KiB are failed fetches. A host mismatch stops before the dialog. Disk open ignores origin attributes. High-water, equal-version digest match, held path, offline upgrade of a file already in hand, rollback, purge, and the installed-launch exceptions. A revocation that is absent from a later `oma-keys` is still stored. Withdrawal itself waits for question 2, because the document that says "withdrawn" has no schema yet. The test can store a revocation the phase-1 types can represent only if question 2 has been decided. Until then, the phase tests the rule the ADR states with a fixture whose body is supplied as already-verified bytes, and it does not define the JSON.

**Acceptance.** Directory copying is no longer how a new-format file is installed. Identifier-only paths are no longer how a new-format application is resolved. The sandbox still cannot write the grant and data paths (ADR 0012).

### 6. Bootstrap and self-upgrade

**Issue text.** [oma trust: Bootstrap and self-upgrade](#issue-oma-trust-bootstrap-and-self-upgrade). Not filed.

**Scope.** ADR 0060 section 8, and the README in `packaging/` brought into line with it. The documented commands download `install.sh` and `install.sh.sig` to files. Verification is plain Ed25519, OpenSSL 3 `pkeyutl`, against the out-of-band fingerprint. The script defaults to the per-user paths, never calls `sudo`, and honours the uid rules for `--system`. `odk oma self-upgrade` checks the release root compiled into the running binary and a strictly higher monotonic version in the receipt outside the binary. A different release root is refused, with no flag that accepts one. Pacman-owned files are refused. The Omarchy repository handoff in ADR 0054 stays. The user units' `ExecStart` becomes the `odk` path this phase installs.

**Modules.** `packaging/` (script, units, README) and the self-upgrade command on the private binary. The release-root public key is compiled in. It is a different key from every publisher root.

**Tests.** A bad signature does not run the script. uid 0 without `--system`, and `--system` without uid 0, write nothing and do not invoke `sudo`. A lower or equal version, a different release root, an unreachable install host, and a pacman-owned path do not replace the binary. The receipt path is the one in section 8. Tests do not download from the network and do not install onto the machine.

**Acceptance.** The script text in the repository matches section 8. Self-upgrade is decided by the binary that is already running.

### 7. Producer and examples

**Issue text.** [oma trust: Producer and examples](#issue-oma-trust-producer-and-examples). Not filed.

**Scope.** `odk oma bundle` writes one `.oma` file: ADR 0052's EROFS image (`mkfs.erofs`, the flags that ADR names) and the phase-1 header, manifest, and signature. `--sign` takes the path of a 32-byte ed25519 seed. The seed is not written into the image, the sandbox, or the consent record. There is no unsigned output. The manifest carries `publisher` and the monotonic `version`. `examples/hello` and `examples/todo`, and the notes that tell a reader how to build their Bundles, produce a real single-file Bundle once phase 5 can run one. Fixture keys stay fixtures. An offline root seed is not an example release.

This phase can start the writer after phase 1. An example that is meant to be executed waits until phase 5.

**Modules.** `crates/oma/src/bundle.rs`, `examples/hello`, `examples/todo`, and the example README.

**Tests.** The writer rejects a manifest that does not fit in the prefix, a missing seed, and an output that does not verify under phase 1. Round-trip: bytes the writer emits are bytes phase 1 accepts. Example keys are marked as fixtures.

**Acceptance.** `bundle` no longer emits a directory as its success path. Existing directory-Bundle unit tests are either updated in this phase or kept, explicitly, as coverage of the old opener until nothing calls it.

## Open questions

ADR 0060 does not settle these. An implementation pull request does not pick an answer. Where a test would need one, the test waits, and the gap stays in this list. Several of them were proposals in the planning note this file was checked against. Those proposals are not adopted.

1. **Ed25519ctx in process.** The four document contexts are Ed25519ctx (RFC 8032), verified in process, not by a shell. `install.sh` is plain Ed25519 via OpenSSL 3 `pkeyutl`, which is decided and is not this question. The workspace has no signature crate. Which crate implements Ed25519ctx, and how CI and packaging supply it, needs its own ADR before phase 1 links one. A planning note recommended OpenSSL's EVP Ed25519ctx through Rust bindings, and said `ed25519-dalek` 2.2.0's public context API is Ed25519ph. The dalek claim was not re-checked against crates.io for this file.

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

## Issue texts

Not filed. `gh issue list` on `csfh/odk` returned no issues. This agent has no tool that creates issues, and `gh` is read-only here, so nothing below has a number. The label to apply when filing by hand is the existing `enhancement` label. Do not create a label. Do not assign anyone. Do not set a milestone.

The spec is not on `main` (`main` ends at ADR 0045). Implement on `cursor/adr-0060-bundle-trust-da7d`, which is [PR #9](https://github.com/csfh/odk/pull/9), based on `cursor/single-file-oma-adrs-60ba` ([PR #3](https://github.com/csfh/odk/pull/3)). ADR 0060 wins over this plan. An open question in this file is not decided inside an implementation pull request. A new fact goes in `docs/checks.md`. CI for every phase is `.github/workflows/tests.yml`: `tests`, `msrv`, `audit`, and `machete`. Tests are headless. Do not register `binfmt_misc` on the machine, and do not open a window on its desktop.

Shared links, on that branch:

- [ADR 0060](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/adr/0060-a-bundles-publisher-is-the-pinned-root-key.md)
- [This plan](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/oma-trust-plan.md)
- [PR #9](https://github.com/csfh/odk/pull/9)

When these are filed, replace each "Blocked by" line with the GitHub issue number of the named text.

### Issue: oma trust: Trust core

**Title.** oma trust: Trust core

**Goal.** Verify an `OMA1` envelope in `oma` without changing install or run. The header, the signed manifest, and `oma-keys` match ADR 0060. A regular file is copied into a sealed object, and that object is what gets checked.

**Links.** [ADR 0060](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/adr/0060-a-bundles-publisher-is-the-pinned-root-key.md) sections 3, 4, 5, and 9. Plan section [1. Trust core](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/oma-trust-plan.md#1-trust-core). [PR #9](https://github.com/csfh/odk/pull/9). [Open questions](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/oma-trust-plan.md#open-questions).

**Blocked by.** Nothing.

**Scope.**

- Little-endian 64-byte header: magic `OMA1`, header version 1, header length 64, algorithm `u16` 1, reserved zeros, manifest length, signature length 64, payload length, 32-byte release public key. Manifest at offset 64, raw 64-byte signature immediately after it, zeros through offset 1024, payload from 1024 to EOF. File size is `1024 + payload length`. Manifest plus signature fits in the prefix (`M + 128 <= 1024`).
- Signed manifest: the twelve fields in ADR 0060 section 4, in that order, canonical UTF-8 JSON as that section states. `header_sha256` is SHA-256 of bytes 0..64. `payload_sha256` is SHA-256 from offset 1024 to EOF.
- `oma-keys`: the exact field list and order in section 3, at most eight release keys, standard base64 of 32-byte keys, key id recomputed from the public key. Empty `keys`, a duplicate id, a lower `version`, and any byte the ADR rejects are a failed document. A higher `version` replaces the set. Keys are not unioned.
- Fingerprint is SHA-256 of the 32 raw key bytes, 64 lowercase hex digits. Key id is the first 8 bytes, 16 lowercase hex digits.
- Ed25519ctx (RFC 8032) for contexts `odk-oma manifest v1` and `odk-oma keys v1`, in process, not via a shell. The other two context strings may be constants. Do not parse a revocation or rotation body.
- Sealed copy: `O_RDONLY|O_NOFOLLOW`, regular file only. Destination is a memfd with `F_SEAL_WRITE`, `F_SEAL_GROW`, `F_SEAL_SHRINK`, and `F_SEAL_SEAL`, or a file in a directory mode `0700` that only the user running the verifier can write. Both are success. No xattr and no stamp file.
- High-water comparison as a pure function: lower `u64` refused, equal `u64` only when the payload digest matches the stored one, higher `u64` accepted. The semver string is not an input.

**Non-goals.** No network, mount, GPUI, CLI change, or new crate. Do not claim the payload is a valid EROFS image. Do not pick a signature crate: open question 1 needs its own ADR before this phase links one. Parser and sealed-copy work can land first. Do not encode open questions 2 through 11 or 13 as policy. Fixtures avoid code points below U+0020 so question 5 stays open. `install.sh` stays plain Ed25519 and is not this phase.

**Crates and modules.** `crates/oma` only (ADR 0031). New `oma::trust` under `crates/oma/src/trust/`, declared from `lib.rs`. A split that fits, and is not a frozen API: `container`, `manifest`, `keys`, `signature`, `sealed`. Reuse `fmt::Identifier`. `SignedManifest` is a different type from `fmt::Manifest`. No dependency on `odk-db` or the ODK crates. Add a crate only when a call uses it, then `tools/check-deps` and `crates/deps/Cargo.toml`. Do not machete-ignore a trust dependency.

**Required tests.** Golden headers at ADR 0060's offsets, including manifest length 896 and 897, a file whose size is not `1024 + payload length`, a non-zero reserved or padding byte, truncation, and ELF and shebang magics. Manifest and `oma-keys` fixtures for the canonical documents, plus duplicates, unknown fields, whitespace, a trailing byte, a bad key length, an empty `keys` array, more than eight keys, a duplicate id, a key id that does not match the key, a lower and a higher `oma-keys` version, and a signature under the wrong context. Once a backend ADR exists: RFC 8032 Ed25519ctx vectors, then fixtures for the two contexts above. Plain Ed25519 and a crossed context fail. Sealed copy: symlink, directory, fifo, and socket rejected; a later write to the source does not change the sealed bytes; the memfd reports all four seals; the mode-`0700` store also succeeds. High-water cases above. No EROFS, FUSE, root, window, or network.

**Acceptance.**

- [ ] `cargo test -p oma` covers the cases above.
- [ ] `inspect`, `run`, `install`, `consent`, and `bundle` behave as they do today.
- [ ] The verifier does not treat the payload as a checked image.
- [ ] No signature crate is added unless an ADR has named it.
- [ ] `tests`, `msrv`, `audit`, and `machete` are green.

### Issue: oma trust: Private binary and inert entry

**Title.** oma trust: Private binary and inert entry

**Goal.** Make `odk` the PATH command and `odk-oma` the private binary, and make a `.oma` file inert. The kernel registration names that binary directly. A `.oma` path does not execute payload bytes.

**Links.** [ADR 0060](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/adr/0060-a-bundles-publisher-is-the-pinned-root-key.md) section 1. [ADR 0056 / PR #6](https://github.com/csfh/odk/pull/6) for the command names and paths. ADR 0056 is not on this branch. Plan section [2. Private binary and inert entry](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/oma-trust-plan.md#2-private-binary-and-inert-entry). [PR #9](https://github.com/csfh/odk/pull/9).

**Blocked by.** [oma trust: Trust core](#issue-oma-trust-trust-core). Also include or stack on PR #6. Do not redesign ADR 0056.

**Scope.**

- `odk` is the only PATH command. `odk oma` is the front door. The private binary is `/usr/lib/odk/odk-oma` or `~/.local/lib/odk/odk-oma`. There is no `oma` command.
- `binfmt_misc` line, exactly: `:odk-oma:M:0:OMA1::/usr/lib/odk/odk-oma:`
- Empty flags field. No `C` flag. No `/usr/lib/odk/binfmt` shell wrapper. The kernel passes the Bundle path as the one argument, and `odk-oma` treats that as `run` of that path. No shell and no command read from the file.
- Desktop handler calls `/usr/bin/odk oma run`, or `~/.local/bin/odk oma run` for a per-user install (ADR 0053's handler, this interpreter).
- Until the image and consent phases exist, that entry verifies the envelope with the trust core and then refuses to execute.
- Generate the drop-in and the desktop file in the repository. A per-user install does not register `binfmt_misc`.

**Non-goals.** Do not register `binfmt_misc` on the development machine, and do not call `systemd-binfmt` or write `/proc/sys/fs/binfmt_misc`. Do not mount an image or open the consent dialog. Do not change the Omarchy user units: their `ExecStart` stays `/usr/bin/oma` until the bootstrap phase. Do not rewrite `packaging/README.md` yet. It still describes the ADR 0054 script. No timestamp role, transparency log, registry, or sandbox change (ADR 0060 section 10).

**Crates and modules.** `crates/oma/src/main.rs`, the binary target in `crates/oma/Cargo.toml`, and the packaging files ADR 0053 and ADR 0056 name (MIME type, desktop entry, `binfmt` drop-in). The `oma` library stays one crate.

**Required tests.** The generated `binfmt` line is the line above. The desktop command is `odk oma run` at the system or per-user path, not a shell. Invoking the private binary with one path argument is `run` of that path. The entry does not execute payload bytes. Headless. The test process does not register an interpreter.

**Acceptance.**

- [ ] The repository contains the drop-in text and the desktop command from ADR 0060 section 1.
- [ ] There is no shell between the kernel and `odk-oma`.
- [ ] Nothing in the test run registers `binfmt_misc` on the host.
- [ ] A `.oma` path does not execute payload bytes.
- [ ] `tests`, `msrv`, `audit`, and `machete` are green.

### Issue: oma trust: Image

**Title.** oma trust: Image

**Goal.** After the envelope verifies, check the EROFS image on the sealed object and mount that object. An object that has not verified never reaches `erofsfuse`.

**Links.** [ADR 0060](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/adr/0060-a-bundles-publisher-is-the-pinned-root-key.md) section 6. [ADR 0052](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/adr/0052-a-bundle-is-one-erofs-file.md) for the image, the mount lifetime, and the unmount helper. Plan section [3. Image](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/oma-trust-plan.md#3-image). [PR #9](https://github.com/csfh/odk/pull/9). Open question 12 (permission lines) stays open.

**Blocked by.** [oma trust: Trust core](#issue-oma-trust-trust-core).

**Scope.**

- Parse and verify the header and the manifest before any EROFS code reads the image.
- `app_id`, `entry`, and `publisher` in the signed manifest match `manifest.toml` inside the image. The executable check `fmt::bundle` already applies to a directory applies to that entry.
- `erofsfuse` runs as the user, with no extra privileges, `ro,nosuid,nodev`, size cap equal to the header's payload length, no setuid helper. Before exec, refuse a device node, a fifo, or a setuid or setgid file. Extract nothing.
- Mount and exec use the sealed object for the life of the mount. The source path is not opened again. `/app` is that mount. The per-launch mount, the view mount, and the unmount helper stay as ADR 0052 and ADR 0053 describe them, pointed at the sealed object.
- The application sandbox in `plan.rs`, `landlock.rs`, `seccomp.rs`, and `init.rs` stays. ADR 0024's init does not gain policy. ADR 0034's intermediate process stays.

**Non-goals.** Do not build the consent dialog, and do not invent a way to read permission lines (open question 12). Do not replace the sandbox or add a grant (ADR 0060 section 10). Do not mount on the user's session. A FUSE test the environment cannot run is recorded in `docs/checks.md`, not skipped without a note. Stamping a real image and mounting it is still the unchecked item ADR 0052 names in `docs/checks.md`. If `erofsfuse` rejects a non-zero prefix, stop and write an ADR. Do not change the format in this phase.

**Crates and modules.** A mount path beside `crates/oma/src/sandbox/mounts.rs`. The sealed object comes from `oma::trust`. `fmt` supplies the inner Manifest checks. No new crate.

**Required tests.** A fixture image that verifies. Fixtures that fail the `manifest.toml` match, the size cap, a device node, a fifo, and a setuid bit. The descriptor `erofsfuse` receives is the sealed object's, not a second open of the source path. An unverified object is never handed to `erofsfuse`. Orphaned-mount behaviour matches ADR 0052.

**Acceptance.**

- [ ] Verification of the prefix completes before any EROFS parser runs.
- [ ] The mounted bytes are the sealed object.
- [ ] Device nodes, fifos, and setuid or setgid files are refused before exec.
- [ ] Nothing is extracted onto the host.
- [ ] The phase does not claim the consent dialog can list permissions.
- [ ] `tests`, `msrv`, `audit`, and `machete` are green.

### Issue: oma trust: Consent and pair-scoped state

**Title.** oma trust: Consent and pair-scoped state

**Goal.** Ask for a new publisher root, and store consent, the pin, the high-water mark, and denials under the pair (root key id, app id). The dialog is built from signed fields and from the URL that was fetched.

**Links.** [ADR 0060](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/adr/0060-a-bundles-publisher-is-the-pinned-root-key.md) sections 2 and 7. [Consent dialog spec](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/spec/consent-dialog.md) on the same branch. Plan section [4. Consent and pair-scoped state](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/oma-trust-plan.md#4-consent-and-pair-scoped-state). [PR #9](https://github.com/csfh/odk/pull/9). Open questions 12, 14, and 15 stay open.

**Blocked by.** [oma trust: Trust core](#issue-oma-trust-trust-core). Not blocked by the image phase or the private-binary phase.

**Scope.**

- Paths: data, previous file, and inbox at `~/.local/share/oma/apps/<root-key-id>/<app-id>/`. Grant file at `~/.local/share/oma/grants/<root-key-id>/<app-id>.toml`. `<root-key-id>` is 16 lowercase hex digits. `<app-id>` is the Identifier. Two roots do not share a directory.
- The dialog shows the fingerprint in groups of eight, the app id, the monotonic version, and the publisher host in Unicode and in punycode. It does not read a display name or an icon from the image, and it does not mount the image to decide what to ask.
- A new root is Install, Run once, or Cancel, including when the permission set is empty. Today `flow.rs` skips the dialog when `grants::is_empty` is true. That skip is the bug this phase removes for a new root.
- Run once verifies under the set the dialog shows, uses a private data directory, and stores neither the pin nor the set. Cancel stores a denial for that pair. A stored denial is not asked again by a launch, by `--register`, or by another pass over the same file. `odk oma install` of that file can ask again.
- `--accept-permissions` grants permissions for a root that is already the pin. On a new root it exits non-zero and installs nothing.
- A second app id under a root the user has already pinned is a new consent for that pair. It names both ids. A release key rotating under the same root is not a consent prompt.
- A confusable host (UTS #39, or edit distance 1 of the stored host's punycode) names the existing host. Install and Run once stay unavailable until that warning is confirmed on its own. One answer cannot both dismiss the warning and install. Which UTS #39 profile applies is open question 15. Do not pick a profile in code. The edit-distance rule can be tested without inventing the profile.
- Old grant files keyed only by Identifier are not pins.
- The helper still uses GPUI directly (ADR 0016, ADR 0033). It gains the signed fields and the separate confirmation. Tests drive it headlessly.

**Non-goals.** Do not invent a reader for permission lines (open question 12). Until an ADR answers it, the dialog shows the signed identity and the fetched host and does not add a permission source. Do not design the grant-file TOML beyond the facts ADR 0060 names (open question 14). Do not treat `user.xdg.origin.url` as a decision. Do not open `oma-consent` on the user's desktop. No sandbox change.

**Crates and modules.** `crates/oma/src/layout.rs`, `grants.rs`, `consent.rs`, `flow.rs`, and `crates/oma-consent`. Signed fields come from `oma::trust`.

**Required tests.** The same Identifier under two roots does not share a grant or a data directory. Empty permissions still produce the new-root dialog. Run once leaves no pin. A denial suppresses launch and `--register` and does not suppress install. `--accept-permissions` on a new root exits non-zero. Edit distance 1 keeps Install and Run once unavailable until the warning is confirmed alone. A today's Identifier-only grant file is not a pin. Headless helper tests. No window.

**Acceptance.**

- [ ] Dialog contents are the signed fields and the fetched URL.
- [ ] Name and icon are not read from the image.
- [ ] Pair paths match ADR 0060 section 2.
- [ ] Run once and Cancel persist what section 7 says and nothing more.
- [ ] `tests`, `msrv`, `audit`, and `machete` are green.

### Issue: oma trust: Install, fetch, run, registration

**Title.** oma trust: Install, fetch, run, registration

**Goal.** Install, upgrade, roll back, purge, and launch a single-file Bundle from the sealed object, after consent. First contact fetches the root-signed key set. A file from disk has no invented download host.

**Links.** [ADR 0060](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/adr/0060-a-bundles-publisher-is-the-pinned-root-key.md) sections 2, 3, 4, 7, and 9. [CLI spec](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/spec/oma-cli.md) on the same branch. Plan section [5. Install, fetch, run, registration](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/oma-trust-plan.md#5-install-fetch-run-registration). [PR #9](https://github.com/csfh/odk/pull/9). Open question 2 (revocation JSON) stays open.

**Blocked by.** [oma trust: Private binary and inert entry](#issue-oma-trust-private-binary-and-inert-entry), [oma trust: Image](#issue-oma-trust-image), and [oma trust: Consent and pair-scoped state](#issue-oma-trust-consent-and-pair-scoped-state).

**Scope.**

- First contact fetches `https://<publisher>/.well-known/oma-keys` and `oma-keys.sig`. HTTPS only, no redirects, three-second timeout, body at most 16 KiB. The signature is the raw 64 bytes and verifies under `root_public_key`. After a root is pinned, the signature must verify under that key. A body signed by any other root is a different publisher and is not applied.
- The host of a URL that was fetched must be the signed `publisher`. A mismatch stops before consent. A public key inside the Bundle is not a trust anchor. If the first-contact fetch fails, there is no pin and nothing is installed. Run once is not offered for a root whose fetch failed.
- A file opened from disk was not fetched. Do not invent a download host. Do not read `user.xdg.origin.url` or `user.xdg.referrer.url` as a decision.
- NXDOMAIN on a stored host is one standard-error line: the host does not resolve, the name may have been lost, the app keeps running, and fetched updates wait. A timeout, TLS failure, non-200, or any other failed fetch is a different line: the host could not be reached, the app keeps running, and a fetched update waits. Neither changes the pin.
- A file already in hand that verifies under a release key the pinned root still authorizes is an upgrade while the host does not resolve.
- High-water mark lives in the grant file, outside the Bundle. A lower integer is refused, including when the semver string looks newer. The same integer is accepted only when `payload_sha256` is the stored digest. Uninstall from `~/Applications` does not clear the mark. `odk oma purge` removes the grant, the data, and the mark. `odk oma rollback` swaps the stored previous file and does not lower the mark.
- An expired manifest is refused at install and upgrade. Launch of an installed application does not need the manifest inside its window and does not need the network. Past `expires_at` it writes: `odk oma: <app id> is past its signature window and keeps running.` Launch re-checks the sealed object's signature, digests, and pin. It does not require the release key to be inside `not_after` or a fresh `oma-keys`.
- A revoked or expired release key does not authorize a new install or upgrade. It does not stop an application already installed. Revocations persist when a later `oma-keys` omits the id or lists the key again. Do not define revocation JSON (open question 2). Test that rule with a fixture whose body is already-verified bytes.
- `--register` does not open a dialog. A structurally valid file signed by a root that is not the pin is held at `~/.local/share/oma/held/<root-key-id>/<app-id>.oma` and is not launched. One standard-error line names the app id and says the file is signed by a different publisher and is held. A file that does not verify, a lower version, and an expired manifest are deleted, not held. Registration deletes only those rejects it owns. It does not delete arbitrary user files.
- Fail closed on the list in ADR 0060 section 9. The successful launch executes the sealed object after it has verified under the pinned root, or, for Run once, under the root the dialog is showing.
- The sandbox still cannot write these paths (ADR 0012).

**Non-goals.** Do not decide open question 2, 4, or 14. Do not add SCTs, `Cache-Control` policy, a transparency log, or a registry. Do not replace directory-Bundle tests that still cover the old opener until the producer phase says they are gone. Standard-error lines from the private binary are prefixed `odk oma:`. Library errors do not carry that prefix.

**Crates and modules.** `crates/oma/src/install.rs`, `flow.rs`, `main.rs`, `sandbox/launch.rs`, and a fetch module under `oma::trust` or `oma`. Consent and paths come from the consent phase. The mount comes from the image phase.

**Required tests.** Redirect, timeout, non-200, and a body over 16 KiB are failed fetches. A host mismatch stops before the dialog. A disk open ignores origin attributes. High-water, equal-version digest match, held path, offline upgrade of a file already in hand, rollback, purge, and the installed-launch exceptions, including the past-window line. A revocation absent from a later `oma-keys` is still stored. NXDOMAIN and another failed fetch produce the two different lines and leave the pin. No test performs a real network fetch. No window and no `binfmt_misc` registration.

**Acceptance.**

- [ ] A new-format file is installed from the sealed object, not by copying a directory tree.
- [ ] A new-format application is resolved by (root key id, app id), not by Identifier alone.
- [ ] First contact that cannot fetch `oma-keys` installs nothing and runs nothing.
- [ ] Rollback does not lower the high-water mark. Purge removes it.
- [ ] The sandbox cannot write the grant and data paths.
- [ ] `tests`, `msrv`, `audit`, and `machete` are green.

### Issue: oma trust: Bootstrap and self-upgrade

**Title.** oma trust: Bootstrap and self-upgrade

**Goal.** Install `odk` from a file that was verified before it ran, and replace `odk-oma` only under the release root compiled into the binary that is already running.

**Links.** [ADR 0060](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/adr/0060-a-bundles-publisher-is-the-pinned-root-key.md) section 8. [ADR 0054](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/adr/0054-oma-is-installed-from-its-domain-then-omarchys-repository.md) for the later repository handoff, which still stands. Plan section [6. Bootstrap and self-upgrade](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/oma-trust-plan.md#6-bootstrap-and-self-upgrade). [PR #9](https://github.com/csfh/odk/pull/9).

**Blocked by.** [oma trust: Private binary and inert entry](#issue-oma-trust-private-binary-and-inert-entry). Not blocked by the image, consent, or install phases.

**Scope.**

- Documented install, with no `-L` and no pipe into a shell:

  ```
  curl --proto '=https' --tlsv1.2 -fsS -o install.sh https://<odk-domain>/install.sh
  curl --proto '=https' --tlsv1.2 -fsS -o install.sh.sig https://<odk-domain>/install.sh.sig
  ```

- The person checks the out-of-band release-root fingerprint, then verifies `install.sh.sig` with OpenSSL 3 `pkeyutl`, plain Ed25519, over the exact bytes of `install.sh`. That check is not Ed25519ctx. A mismatch stops the install. The script runs only after that check: `sh install.sh`. The fingerprint is not in the script and not at a URL the script fetches.
- The odk release root is a different key from every publisher root. Its public key is compiled into `odk-oma`.
- Default install is per-user: `~/.local/bin/odk`, `~/.local/lib/odk/odk-oma`, `~/.local/lib/odk/oma-consent`. The script does not call `sudo`. Effective uid 0 without `--system` exits and writes nothing. `--system` without effective uid 0 exits and does not call `sudo`. `--system` is the root install under `/usr`, including the `binfmt_misc` registration. The per-user install does not register `binfmt_misc`.
- `odk oma self-upgrade` accepts a new binary only when the signature verifies under the release root compiled into the running binary, and the monotonic version is strictly above the receipt at `/var/lib/oma/runtime.toml` or `~/.local/share/oma/runtime.toml`. The running binary performs the check. A different release root is not an upgrade. There is no flag that accepts one. A lower or equal version does not replace the binary. A dark install host does not replace it. When pacman owns the files, self-upgrade refuses them.
- ADR 0054's later handoff to Omarchy's repository stands. The script does not download a tarball once the package is there.
- Bring `packaging/README.md` and the user units' `ExecStart` into line with these paths. Today the README describes ADR 0054's domain script and `/usr/bin/oma`.

**Non-goals.** Do not choose the in-process Ed25519ctx crate (open question 1). This phase's script check is the OpenSSL 3 command above. Do not add a recovery key that replaces the compiled-in release root. Do not stream the script into a shell. Tests do not download from the network and do not install onto the machine. Do not register `binfmt_misc` during tests.

**Crates and modules.** `packaging/` (script, units, README) and the self-upgrade command on the private binary. The release-root public key is compiled into that binary.

**Required tests.** A bad signature does not run the script. uid 0 without `--system`, and `--system` without uid 0, write nothing and do not invoke `sudo`. A lower or equal version, a different release root, an unreachable install host, and a pacman-owned path do not replace the binary. The receipt path is one of the two paths in section 8. The running binary is what accepts the new file.

**Acceptance.**

- [ ] The script text in the repository matches ADR 0060 section 8.
- [ ] Self-upgrade is decided by the binary that is already running.
- [ ] A different release root is refused, with no accepting flag.
- [ ] `packaging/README.md` no longer tells the old install story.
- [ ] `tests`, `msrv`, `audit`, and `machete` are green.

### Issue: oma trust: Producer and examples

**Title.** oma trust: Producer and examples

**Goal.** `odk oma bundle` writes one signed `.oma` file. Hello and Todo become single-file Bundles once a verified run path exists. There is no unsigned output.

**Links.** [ADR 0060](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/adr/0060-a-bundles-publisher-is-the-pinned-root-key.md) section 4 (`odk oma bundle --sign`). [ADR 0052](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/adr/0052-a-bundle-is-one-erofs-file.md) for the image and for `mkfs.erofs`. [ADR 0032](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/adr/0032-oma-bundle-assembles-a-bundle-from-a-built-executable.md) for "compiles nothing". Plan section [7. Producer and examples](https://github.com/csfh/odk/blob/cursor/adr-0060-bundle-trust-da7d/docs/oma-trust-plan.md#7-producer-and-examples). [PR #9](https://github.com/csfh/odk/pull/9).

**Blocked by.** [oma trust: Trust core](#issue-oma-trust-trust-core) for the writer. Example workflows that execute a Bundle are also blocked by [oma trust: Install, fetch, run, registration](#issue-oma-trust-install-fetch-run-registration). The writer can proceed before that run path exists.

**Scope.**

- `odk oma bundle` stages the tree, runs `mkfs.erofs`, stamps the ADR 0060 header, manifest, and signature, and renames into place only if phase 1's verifier accepts the file. The `mkfs.erofs` invocation ADR 0052 names is `mkfs.erofs -z zstd,9 -T 0 --all-root -U clear <image> <staged-tree>`. Repeated `--include <file>[=<path>]` copies extra resources under the same containment rules as the executable. The command still compiles nothing and still refuses to replace an existing target.
- `--sign <secret-key>` is the path of a 32-byte ed25519 seed of the release key. The seed is never written into the image, the sandbox, or the consent record. There is no unsigned output. ADR 0052's sentence that an unsigned file carries a zero signature does not survive ADR 0060.
- The signed manifest carries `publisher` and the monotonic `version`. A manifest that does not fit in the prefix is rejected. Missing `mkfs.erofs` fails with an error that names `erofs-utils` and does not write a Bundle.
- `examples/hello` and `examples/todo`, and the notes that say how to build their Bundles, produce a real single-file Bundle once the install and run phase can run one. Fixture keys are marked as fixtures. An offline root seed is not an example release.
- Two runs over the same inputs and the same signing key produce the same file, as ADR 0052 requires of the image.

**Non-goals.** Do not pick the Ed25519ctx crate (open question 1). Do not emit code points below U+0020 in signed documents until question 5 is decided. Do not change the image format if a prefixed image will not mount. That is the ADR 0052 check in `docs/checks.md`, and a failure is a new ADR. Do not register `binfmt_misc` or open a window to prove an example. A headless run waits until the install and run phase exists.

**Crates and modules.** `crates/oma/src/bundle.rs`, `examples/hello`, `examples/todo`, and the example README. The stamp uses `oma::trust`.

**Required tests.** The writer rejects a manifest that does not fit in the prefix, a missing seed, and an output that does not verify under the trust core. Round-trip: bytes the writer emits are bytes the trust core accepts. Example keys are identifiable as fixtures. Directory-Bundle unit tests are either updated here or kept, explicitly, as coverage of the old opener until nothing calls it.

**Acceptance.**

- [ ] `bundle`'s success path is one `.oma` file, not a directory.
- [ ] There is no unsigned output.
- [ ] The seed does not appear in the image, the sandbox, or the consent record.
- [ ] Hello and Todo's notes describe a single-file Bundle, and an executable example is wired only after the install and run phase can run it.
- [ ] `tests`, `msrv`, `audit`, and `machete` are green.
