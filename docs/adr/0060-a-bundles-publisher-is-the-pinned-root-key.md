---
status: accepted
---

# A Bundle's publisher is the pinned root key, and the file does not run itself

ADR 0055 made six trust rules. The publisher's domain was the root of trust, through `https://<domain>/.well-known/oma-keys`. The signature was checked at install, at upgrade and at every launch. `user.xdg.origin.url` was a hint that never blocked. A domain that went dark left installed applications running and held updates. A domain that came back with a different key was a new publisher. Any new key, including a key the domain had just listed, was the first-install consent flow, and there was no rotation.

This ADR replaces those six rules. It supersedes ADR 0055. The 1024-byte prefix ADR 0052 left in front of the EROFS image is still where the trust metadata lives, and the bytes inside it are the header and the signed manifest below, not ADR 0055's 120-byte preamble. Nothing here is built. The code still treats every Bundle as unsigned.

The command a person types is `odk oma`. There is no `oma` command. The private binary that verifies, mounts and starts a Bundle is `odk-oma`, at `/usr/lib/odk/odk-oma` or, for a per-user install, `~/.local/lib/odk/odk-oma` (ADR 0056). Standard-error lines from that binary are prefixed `odk oma:`.

Two advisory reviews dated 2026-10-25 read ADR 0055's six rules: [GPT-6.1-Sol, through Codex CLI](../reviews/2026-10-25-signing-model-gpt-6.1-sol.md), and [Grok 4.7, through Grok CLI](../reviews/2026-10-25-signing-model-grok-4.7.md). They are advisory. This record is the decision. The reviews are not reproduced here. The order in which to build this ADR is [docs/oma-trust-plan.md](../oma-trust-plan.md). That plan decides nothing. The in-process Ed25519ctx crate is [ADR 0061](0061-ed25519ctx-is-verified-with-crrl.md). The questions still open are listed in the plan, along with the hashing measurement below.

## 1. The file is inert

A `.oma` file begins with the four bytes `OMA1`. That magic is not ELF (`\x7fELF`) and not a shebang (`#!`). The kernel's ELF and script handlers run before `binfmt_misc` and are not consulted for this magic, so the kernel never executes bytes from the Bundle. There is no preamble that is a program.

`binfmt_misc` is registered by root, for this magic only, and the interpreter is the private binary. A system install writes `/usr/lib/binfmt.d/odk.conf`:

```
:odk-oma:M:0:OMA1::/usr/lib/odk/odk-oma:
```

The flags field is empty. The `C` credentials flag is not set, so the kernel does not take credentials from the Bundle. The argument vector is fixed: the kernel invokes `/usr/lib/odk/odk-oma` with the Bundle path, and `odk-oma` treats that invocation as `run` of that path. It does not read a command or arguments out of the file, and it does not pass the path through a shell. `systemd-binfmt` loads the registration. The executable bit is still required for that path; a double-click uses the desktop handler either way (ADR 0053), and that handler calls `/usr/bin/odk oma run` (per-user, `~/.local/bin/odk oma run`).

**Tradeoff.** A Bundle does not run on a machine where odk is not installed. `./App.oma` from a shell works only where this registration is installed, which is the system install below. A per-user install cannot register `binfmt_misc`, because that needs root, and on that install the file runs through the desktop handler. This is accepted. A file that can start itself has to contain the code that starts it, and that code is the part a signature check never gets to see.

ADR 0053 already rejected a shebang and an ELF stub for that reason. No accepted ADR on this branch made the preamble self-running. What this ADR supersedes is the interpreter ADR 0053 installed: a root-owned shell script whose job was `exec` of a public `oma`. ADR 0056 carries the same wrapper forward as `/usr/lib/odk/binfmt`. The registration names `/usr/lib/odk/odk-oma` directly. There is no shell between the kernel and the verifier.

## 2. The pin is the root key

The installed publisher is the pinned root public key. Its identity is the fingerprint: SHA-256 over the 32 raw bytes, stored and shown as 64 lowercase hex digits. The key id is the first 8 bytes of that digest, stored raw and shown as 16 lowercase hex digits. `odk-oma` recomputes both. A document whose key id does not match its public key fails verification.

The domain is display text, and it is the host used the first time that root is discovered. It is not the root of trust. Controlling the domain does not move update rights. A different root public key served at the same URL is a new publisher: it has no installed applications, and it inherits no data, no permissions, no previous version and no high-water mark. The applications already pinned to the old root keep that root.

An application is the pair (root key id, app id). Data, the previous file, the inbox, the consent record and the high-water mark are stored under that pair:

| What | Path |
|---|---|
| Data, previous file, inbox | `~/.local/share/oma/apps/<root-key-id>/<app-id>/` |
| Consent, pin, high-water mark, denials | `~/.local/share/oma/grants/<root-key-id>/<app-id>.toml` |

`<root-key-id>` is 16 lowercase hex digits. `<app-id>` is the Identifier. Two Bundles with the same Identifier and different roots are different applications and do not share a directory. The sandbox still cannot write these paths (ADR 0012).

**NXDOMAIN** on a host already stored for a pinned root is shown as a suspected loss of the name. Installed applications keep running. `odk oma inspect` prints the host, that the name does not resolve, and the path of any held file. One line on standard error:

> odk oma: `<host>` does not resolve; the name may have been lost. `<app id>` keeps running. Updates that have to be fetched from this host are held.

A timeout, a TLS failure, a non-200 status or any other failed fetch is not called a name loss. The line says the host could not be reached, the installed application keeps running, and a fetched update waits. Neither case changes the pin.

A file already in hand whose signature verifies under a release key the pinned root still authorizes is an upgrade even when the host does not resolve. The domain is not asked to approve bytes the root has signed. A fetch of a new key set, or of anything else that is not already in hand, waits.

## 3. Two keys, and a way to rotate them

This replaces ADR 0055's rule that any new key is the first-install flow and that an old key cannot appoint a new one.

The user pins one offline publisher root. That root authorizes short-lived release-signing keys. A release key has a `not_after` instant. It signs Bundles. It does not appoint a root, and it does not edit the key set. The root is what stays offline. One root key is the pin. A threshold of roots is not required.

Three root-signed documents, plus the manifest, are what the root signs in this version. Each is a separate document. Each signature is Ed25519ctx (RFC 8032) with the context string in the table, so a signature over one document does not verify as another. The algorithm field of every document is the string `ed25519` for this version. An unknown algorithm is a hard failure (section 9). A later change of algorithm is a rotation under the same root: the root signs an algorithm policy (context `odk-oma algorithm v1`) naming the new algorithm, and during the transition a file carries both signatures over the same manifest bytes, each with its own context. That is the same publisher. It is not a first-install prompt. A binary that does not know one of the two algorithms rejects the file.

| Document | Context string |
|---|---|
| Signed manifest | `odk-oma manifest v1` |
| `oma-keys` | `odk-oma keys v1` |
| Rotation | `odk-oma rotation v1` |
| Revocation | `odk-oma revocation v1` |

**`oma-keys`** is `https://<publisher>/.well-known/oma-keys`. HTTPS only. Redirects are not followed. A redirect, a TLS failure, a timeout of three seconds, or a status other than 200 is a failed fetch. The body is at most 16 KiB. It is a canonical document (the same encoding rules as the manifest) of exactly these fields, in this order:

| Field | Rule |
|---|---|
| `algorithm` | `ed25519` |
| `root_public_key` | standard base64 of the 32-byte root |
| `root_key_id` | key id of that root |
| `version` | monotonic integer, no leading zeros |
| `keys` | the exact release-key set, at most eight entries, sorted by key id |

Each entry of `keys` is an object with exactly `id`, `not_after` (RFC 3339 UTC) and `public_key` (standard base64 of 32 bytes), in that order. `id` must be the key id of `public_key`. Duplicate ids, a key that is not 32 bytes, an empty `keys` array, a field this table does not name, a duplicate field, and any byte after the one JSON value are a failed fetch, not a document that listed nothing.

The signature is the file `https://<publisher>/.well-known/oma-keys.sig`, the raw 64 bytes, fetched with the same rules. It verifies under `root_public_key` over the exact body bytes.

On first contact the user is about to pin `root_public_key`. After a root is pinned, the signature must verify under that pinned key. A body signed by any other root is a different publisher and is not applied to the pinned one.

`version` is a high-water mark for this root's key set. A body whose version is lower than the stored version is rejected. A body whose version is higher replaces the stored set. The set is exact: a release key that is not in it is not authorized for install or upgrade, and keys are not unioned with a previous body, with a key found in a Bundle, or with an extra key a mirror added. Unknown schema versions are rejected.

**Rotation** is a root-signed document with `algorithm`, `version`, `expires_at`, `successor` (standard base64 of the new root's 32 bytes) and `app_ids`. `app_ids` is either a JSON array of Identifiers or the string `*`. It verifies under the current root. Past `expires_at` it is rejected. On success, each named application (or every application under that root, when `app_ids` is `*`) is pinned to the successor. Data, permissions, the previous file, the inbox and the high-water mark move with the application. The successor inherits nothing else. The domain is not consulted for this.

**Revocation** is a root-signed document naming release key ids. Once a revocation verifies, `odk-oma` stores it and does not drop it because a later `oma-keys` omits it or lists the key again. Removing a revocation takes a later revocation document, signed by the same root, that names the id and says it is withdrawn. A revoked release key cannot authorize an install or an upgrade. An application already installed under that root still launches.

A first-install prompt is only for a root the user has not pinned and that no rotation document has appointed. A new release key inside a higher `oma-keys`, a revocation, and a rotation are not that prompt.

## 4. One signed manifest

One manifest covers every byte that affects execution. It is canonical UTF-8 JSON: one object, keys in the order below and no others, no whitespace, no duplicate keys, integers in decimal without a sign and without leading zeros, hex in lowercase, timestamps as `YYYY-MM-DDTHH:MM:SSZ`, strings escaped only for quotation mark, reverse solidus and code points below U+0020. Any other encoding, a duplicate field, an unknown field, or a trailing byte is a parser error.

| Field | Meaning |
|---|---|
| `algorithm` | `ed25519` |
| `app_id` | The Identifier. It matches `identifier` in `manifest.toml`. |
| `version` | Monotonic `u64`. This is the version the high-water mark stores. It is not the semver string in `manifest.toml`. |
| `entry` | The executable path. It matches `executable` in `manifest.toml`. |
| `payload_sha256` | SHA-256 of every byte from offset 1024 to the end of the file. |
| `header_sha256` | SHA-256 of the 64-byte header. |
| `key_id` | Key id of the release public key in the header. |
| `root_key_id` | Key id of the publisher root. Consent and the high-water mark use this, not the release key. |
| `issued_at` | RFC 3339 UTC. |
| `expires_at` | RFC 3339 UTC, later than `issued_at`. |
| `update_url` | `https` URL whose host is `publisher`, with no userinfo and no port. |
| `publisher` | Hostname: lowercase letters, digits, hyphens and dots, no scheme, no userinfo, no port, no path, not an IP address, at most 253 characters. An internationalized name is the punycode form (`xn--`). |

The semver in `manifest.toml` is the version people read. The payload digest covers it. Upgrade and rollback comparisons use `version`, the integer. A file whose integer is lower than the stored high-water mark for that (root key id, app id) is refused, including when the semver string looks newer. The same integer is accepted only when `payload_sha256` is the digest already stored for that version. A higher integer is an upgrade when the rest of this ADR accepts the file.

`odk-oma` stores that high-water mark in the grant file, which is outside the Bundle. `odk oma purge` removes the grant, the data and the mark. Uninstalling the file from `~/Applications` does not remove the mark, so installing an older file afterwards is still refused. `odk oma rollback` (ADR 0011) swaps in the previous file already stored and does not lower the mark. That previous file is an installed copy. It is not a new file with a lower version.

An expired manifest is refused at install and at upgrade. Launch of an application already installed does not need the manifest to be inside its window, and it does not need the network. A launch past `expires_at` writes one line: `odk oma: <app id> is past its signature window and keeps running.`

The image's `manifest.toml` is still the Manifest of ADR 0006. `app_id`, `entry` and `publisher` must match it. A mismatch is a failed verification.

### The header

Little-endian. The signed manifest begins at offset 64. The 64-byte signature begins at offset `64 + manifest length`. Bytes after the signature and before offset 1024 are zero. The EROFS image begins at offset 1024 and runs for `payload length` bytes, which is the end of the file. A file whose size is not `1024 + payload length` is rejected. The manifest must fit in the prefix with its signature: `64 + manifest length + 64` is at most 1024.

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | `OMA1` |
| 4 | 2 | header version, `u16` = 1 |
| 6 | 2 | header length, `u16` = 64 |
| 8 | 2 | algorithm, `u16` = 1 (ed25519). Any other value fails closed. |
| 10 | 2 | reserved, `u16` = 0 |
| 12 | 4 | manifest length, `u32` |
| 16 | 4 | signature length, `u32` = 64 |
| 20 | 8 | payload length, `u64` |
| 28 | 4 | reserved, `u32` = 0 |
| 32 | 32 | release public key |

The release public key's key id must equal `key_id`. The signature verifies under that key over the canonical manifest bytes and no others. The release public key must be a member of the pinned root's current exact set, not revoked, and not past its `not_after`, before an install or an upgrade. Launch of an already-installed file uses the signature, the digests and the pin. It does not require the release key to be inside its `not_after`, and it does not require a fresh `oma-keys`.

`odk oma bundle --sign <secret-key>` fills the signature. The file holds the 32-byte ed25519 seed of the release key. It is a path the author passes, and it is never written into the image, the sandbox or the consent record. There is no unsigned output. A header whose signature does not verify is invalid.

`odk-oma` verifies Ed25519ctx itself (RFC 8032), with `crrl` as [ADR 0061](0061-ed25519ctx-is-verified-with-crrl.md) decides. It does not shell out.

## 5. The bytes that verify are the bytes that run

`odk-oma` opens the path with `O_RDONLY|O_NOFOLLOW`. The file must be a regular file. A symlink, a directory, or any other file type is rejected. It copies those bytes into a new object and seals it before it trusts them. The object is a memfd sealed with `F_SEAL_WRITE`, `F_SEAL_GROW`, `F_SEAL_SHRINK` and `F_SEAL_SEAL`, or a file in a directory mode `0700` that only the user running `odk-oma` can write, opened the same way and not writable after the copy. Verification runs on that object. The mount and the exec use that object for as long as the mount exists. The path is not opened a second time.

No extended attribute, and no stamp file beside the Bundle, is a verification result. `odk-oma` does not write one and does not read one. A `verified` attribute is ignored the same way an absent one is.

## 6. The image is parsed after the manifest verifies

The 64-byte header and the manifest are parsed and verified before any EROFS code reads the image. `erofsfuse` is not started on a file that has not verified. It runs as the user, with no extra privileges, with `ro,nosuid,nodev`. The payload length in the header is the size cap. There is no setuid helper. Before the application is executed, `odk-oma` refuses the mount if it contains a device node, a fifo, or a setuid or setgid file. Nothing is extracted onto the host. The install is the sealed object and the pin.

The per-launch mount, the view mount and the unmount helper stay as ADR 0052 and ADR 0053 describe them, pointed at the sealed object.

## 7. Consent is the root, the application and the URL that was fetched

The consent dialog is built from signed fields and from the URL `odk-oma` actually fetched. It shows the root fingerprint (64 hex digits, in groups of eight), the app id, the monotonic version, and the publisher host in Unicode and in punycode. It does not read the image for a display name or an icon, and it does not mount the image to decide what to ask.

`user.xdg.origin.url` and `user.xdg.referrer.url` are never an input to a decision. They are not a publisher, not a fetch URL, and not a match against `publisher`. If one is shown, the line says it is an unverified leftover of the download. A missing attribute changes nothing.

The host of a URL `odk-oma` fetched must be the signed `publisher`. A mismatch is a hard stop, before consent. A file opened from disk was not fetched. No download host is invented for it, and the attributes above are not that host. First contact with a new root still fetches `oma-keys` from `https://<publisher>/.well-known/oma-keys` and nowhere else. If that fetch fails, there is no pin and nothing is installed. A public key inside the Bundle is not a trust anchor.

A new root is the first-install buttons: Install, Run once, Cancel (ADR 0018). The release key must be in the `oma-keys` set signed by the root the dialog shows. Install pins that root for that app id, records consent for the permissions in the Manifest, and installs the file. Run once verifies under that same set, runs with a private data directory, and stores neither the pin nor the set. A root that is not already pinned, and whose `oma-keys` fetch fails, is not run. Cancel installs nothing and launches nothing, and the denial is stored for that (root key id, app id). A stored denial is not asked again by a launch, by `--register`, or by another pass over the same file. `odk oma install` of that file can ask again, because the person is asking. Non-interactive mode fails closed (ADR 0016). `odk oma install --accept-permissions` grants permissions for a root that is already the pin. Pointed at a new root, it exits non-zero and installs nothing.

Consent is scoped to (root key id, app id). A second app id under a root the user has already pinned is a new consent for that pair. It names both ids. It is not a repeat of the first app's prompt. A release key rotating under the same root is not a consent prompt.

When the publisher host is a Unicode confusable (UTS #39) of a host already stored for a different pinned root, or within edit distance 1 of that host's punycode form, the dialog says so and names the existing host. Install and Run once stay disabled until the person confirms that warning on its own. One confirmation cannot both dismiss the warning and install.

`--register` does not open a dialog. A structurally valid file signed by a root that is not the pin is held at `~/.local/share/oma/held/<root-key-id>/<app-id>.oma` and is not launched. One line on standard error names the app id and says the file is signed by a different publisher and is held. A file that does not verify, a file whose version is lower than the high-water mark, and a file whose manifest has expired are deleted, not held.

## 8. Installing odk, and replacing it

`odk-oma` has to be on the machine before any Bundle can be trusted, because `odk-oma` is the program that checks the signature. That install is its own trust decision. The odk release root is a distinct key from every publisher root. Its public key is compiled into `odk-oma`. Its fingerprint, SHA-256 of those 32 bytes as 64 lowercase hex digits, is published out of band: not in `install.sh`, and not at a URL the script fetches.

The documented install downloads to a file and verifies before anything runs. It does not stream the script into a shell.

```
curl --proto '=https' --tlsv1.2 -fsS -o install.sh https://<odk-domain>/install.sh
curl --proto '=https' --tlsv1.2 -fsS -o install.sh.sig https://<odk-domain>/install.sh.sig
```

There is no `-L`. The person checks the out-of-band fingerprint against the release-root public key they obtained from that same out-of-band channel, then verifies `install.sh.sig` under that key (OpenSSL 3, `pkeyutl`, Ed25519) over the exact bytes of `install.sh`. That check is plain Ed25519, which is what OpenSSL 3 verifies. It is not Ed25519ctx. The script is not a manifest, so the two signatures do not stand in for each other. A mismatch stops the install. The script is run only after that check: `sh install.sh`. A signature fetched from the install host is not a second root. The fingerprint the person compared is the root.

The script's default is per-user, under `~/.local`, at the paths ADR 0056 names (`~/.local/bin/odk`, `~/.local/lib/odk/odk-oma`, `~/.local/lib/odk/oma-consent`). It does not call `sudo`. If the effective uid is 0 and `--system` was not passed, it exits and writes nothing. `--system` is the explicit flag for a root install under `/usr`, including the `binfmt_misc` registration. If `--system` is passed and the effective uid is not 0, it exits without calling `sudo`. The per-user install does not register `binfmt_misc`.

`odk oma self-upgrade` accepts a new `odk-oma` only when the signature verifies under the release root compiled into the running binary (a release key only when that root's signed key set authorizes it) and the monotonic version is strictly above the high-water mark. The mark is stored in the receipt, `/var/lib/oma/runtime.toml` or `~/.local/share/oma/runtime.toml`, which is not inside the binary being replaced. The running binary performs the check. The binary just written does not get to decide that it was acceptable. A different release root is not an upgrade. There is no flag that accepts one. Replacing the release root is a new install.

A dark install host does not replace the binary. A lower or equal version does not replace it. When pacman owns the files, `odk oma self-upgrade` refuses them, and upgrades come from the package (ADR 0054). ADR 0054's later handoff to Omarchy's repository stands: that repository's signing key is the trust anchor for a packaged install, and the script does not download a tarball once the package is there.

## 9. Fail closed

These are hard failures. Nothing is installed, upgraded or launched:

- an algorithm the binary does not know
- a missing pin, where a pin is required (launch or upgrade of an application, or a first install whose `oma-keys` fetch did not produce a root)
- a digest that does not match
- a parser error, including a bad length, a duplicate field, an unknown field, a trailing byte, a non-canonical document, a symlink, or a file that is not a regular file
- a signature that does not verify
- a release key that is not in the pinned root's exact set, on install or upgrade
- a download host that is not the signed publisher

The only successful launch is the one that executes the sealed object after that object has verified under the pinned root, or, for Run once, under the root the dialog is showing. A log line is not a success.

## 10. Not decided here

These are follow-ups. This ADR does not choose them.

- **A timestamp role, and a transparency log.** A short-lived signed statement of the current version, and an append-only log of key sets and releases, would make a split view detectable. Freshness beyond `expires_at` and the high-water mark is not specified here.
- **App confinement.** A signature says who published the bytes. What the process may touch is the sandbox `odk-oma` already builds (ADR 0005, ADR 0012, `docs/spec/sandbox-baseline.md`). [PR #4](https://github.com/csfh/odk/pull/4) names a sandboxed process with `OMA_SANDBOX=1` (ADR 0048): that process reads oma's copy of the text size, the font and the reduced-motion answer, and it does not reach Hyprland. This ADR does not add or remove a grant.
- **A curated registry.** An optional extra root, for people who want a store, is not specified. It would not replace the per-publisher pin. ADR 0010 still gives the Bundle program no operator of its own.

## What was rejected

- **The domain as the root of trust.** That is ADR 0055's first rule. Whoever takes the name takes the publisher. The pin is the root key. The name is how a person finds it the first time, and the text the dialog shows.
- **No rotation.** ADR 0055's sixth rule. Every honest key change then looks like a takeover, and a stolen release key cannot be retired. The root appoints release keys, revokes them and names a successor.
- **Unioning keys from an unsigned `oma-keys`.** A body that still contains the old key and adds another one would widen the set under a reading of "a different key is a new publisher". The stored set is replaced only by a higher root-signed version, and it is exact.
- **A self-running preamble.** A shebang or an ELF stub runs before `odk-oma`. ADR 0053 rejected both. The magic stays `OMA1`, which is neither.
- **A shell script as the `binfmt_misc` interpreter.** The script is a second program, and it is not the binary whose release root was checked.
- **Trusting `user.xdg.origin.url`.** It is writable by anyone who can write the file, and it disappears on a filesystem without extended attributes. It is not a decision. The fetched host is.
- **A `verified` attribute or a stamp file.** The same user who can replace the Bundle can write the stamp.
- **Streaming `install.sh` into a shell.** The shell runs bytes before the download has finished, and the script cannot check its own signature. The file is verified against the out-of-band release-root fingerprint, then run.
- **A checksum of the script published on the same host.** It catches a mirror that drifted. It does not catch the host serving a new pair. ADR 0054 weighed this and did not use it. The out-of-band fingerprint is the check this ADR adds.
- **Unsigned Bundles.** ADR 0007 left them unsigned and chose no trust model. ADR 0055 still allowed an unsigned file while no key was pinned. A missing pin on a launch, and a file that does not verify, are hard failures.
- **dm-verity and fs-verity.** Unchanged from ADR 0055. dm-verity needs `CAP_SYS_ADMIN` in the initial user namespace, which the sandbox does not have. fs-verity still needs a measurement the writer of `~/Applications` cannot change, which is the same check as the signature over the sealed object.
- **The first-install prompt as the rotation mechanism.** Rotation is a signed document. The prompt is for a publisher the user has not pinned.

## Consequences

- ADR 0055 is superseded. Its three layers, its 120-byte preamble, its unsigned path, its grant fields and its rule that a new key cannot be appointed do not apply.
- ADR 0007 is amended. Unsigned is not the trust model. The signature lives in the prefix, over the manifest, and the pin is the root key.
- ADR 0015 is amended. The version that must be strictly higher is the signed monotonic integer. A file that verifies under the pinned root is an upgrade when that integer is higher and the manifest has not expired, including while the publisher host is unreachable. A different root is not an upgrade.
- ADR 0023 is amended. Consent is stored per (root key id, app id), not per Identifier alone.
- ADR 0052 is amended in the bytes of the prefix. The 1024-byte reservation, the EROFS image at offset 1024, and the mount rules stand. The header and the manifest in this ADR replace the header ADR 0055 defined.
- ADR 0053 is amended in the `binfmt_misc` registration. The MIME type, the desktop handler and "Show contents" stand. The interpreter is `/usr/lib/odk/odk-oma`, with no `C` flag and a fixed argument vector.
- ADR 0054 is amended in the bootstrap and in `self-upgrade`. The script is downloaded to a file and verified against the out-of-band release-root fingerprint before it runs. The default is per-user, and root requires `--system`. `odk oma self-upgrade` checks the compiled-in release root and a high-water mark outside the binary. The later Omarchy package, and the refusal to replace files pacman owns, stand.
- ADR 0056's paths are the ones this ADR uses. Its `/usr/lib/odk/binfmt` shell wrapper is superseded by the registration in section 1. ADR 0056 is not in this branch; it is pull request #6.
- `docs/spec/manifest.md`, `docs/spec/consent-dialog.md` and `docs/spec/oma-cli.md` follow this ADR where they used to state ADR 0055.
- The code is unchanged. It still installs a directory and still verifies no signature. The build order is `docs/oma-trust-plan.md`. The Ed25519ctx implementation is ADR 0061.

## Open questions

1. How long hashing a representative Bundle takes on the development machine (`docs/checks.md`). The check stays a whole-file payload digest until a measurement says it cannot, and a different construction would be a new ADR.
