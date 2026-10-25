---
status: accepted
---

# A Bundle's trust is its publisher's domain, then its key, then its download origin

A Bundle is signed with ed25519. The signature lives in the reserved 1024-byte prefix (ADR 0052). Trust is three layers, in this order, and a later layer never overrides an earlier one:

1. **The publisher's domain** is the root of trust. The Manifest names a domain. That domain publishes its current signing keys at `https://<domain>/.well-known/oma-keys`. oma fetches that document over HTTPS.
2. **The signature** is checked at install, at upgrade and at launch. The public key in the prefix must verify the file, and that key is pinned per Identifier the first time the user installs it. A file signed by any other key is not installed, upgraded or launched until the user accepts that key through the first-install consent flow (Install, Run once, Cancel). Listing the new key on the domain does not accept it. A domain that was dark and comes back with a different key is a new publisher, and that is the same flow.
3. **The download origin** is a hint, read from the file's `user.xdg.origin.url` and `user.xdg.referrer.url` extended attributes when the filesystem has them. It is shown when it disagrees with the publisher. It never blocks, and it never lets a key in.

This amends ADR 0007. ADR 0007 put a future signature in a file inside the Bundle directory, over per-file hashes, which was the shape of a directory (ADR 0022), and it chose no trust model. A sealed image has nowhere inside the tree that is not part of the content, and ADR 0008 already required that a plain reader see the same directory with or without a signature. The prefix is outside the filesystem, so a mount shows the Bundle and no signature file. The trust model is the three layers above. Unsigned Bundles stay installable when no key is pinned for that Identifier. Once a key is pinned, an unsigned file does not clear it.

Nothing here is built. The code still treats every Bundle as unsigned.

There is no way for an old key to appoint a new one. The prefix has no field for that, and oma keeps no such statement. Any new key is the first-install consent flow. That includes a key the domain has newly listed, a key from a domain that went dark and came back, and any other case.

## The prefix

Little-endian, inside the first 120 bytes of the 1024. Bytes 120 through 1023 stay zero. Version 1 rejects any other header length.

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | `OMA1` |
| 4 | 2 | header version, `u16` = 1 |
| 6 | 2 | header length, `u16` = 120 |
| 8 | 4 | flags, `u32`. Bit 0 means a signature is present. |
| 12 | 2 | algorithm, `u16`. 0 is none, 1 is ed25519. |
| 14 | 2 | reserved, `u16` = 0 |
| 16 | 8 | key id. Zero if unsigned. |
| 24 | 32 | ed25519 public key. Zero if unsigned. |
| 56 | 64 | signature. Zero if unsigned. |

The key id is the first 8 bytes of SHA-256 over the 32-byte public key, stored raw and shown as 16 lowercase hex digits. oma recomputes it. A header whose key id does not match its public key fails verification.

The signed bytes are the 1024-byte preamble with the 64 signature bytes set to zero, then every byte from offset 1024 to the end of the file. The image is signed, and so are the magic, the version, the flags, the algorithm, the key id and the public key. Clearing the flag, swapping the algorithm or replacing the public key does not produce a file that still verifies. The signature field itself is the part left out. There is no second file and no trailer.

`oma bundle --sign <secret-key>` fills the field. The file holds the 32-byte ed25519 seed. It is a path the author passes, and it is never written into the image, the sandbox or the consent record. Without `--sign` the flag is clear, the algorithm is 0, and the key id, public key and signature are zero. A signed header with a signature that does not verify is invalid and is not treated as unsigned.

oma verifies ed25519 itself (RFC 8032) over those bytes. It does not shell out to `minisign`. A minisign text signature was weighed and rejected: the trust root is the domain's key document, and a second signature encoding would be a second thing to check.

## What oma keeps per Identifier

Next to the consent permissions, in `~/.local/share/oma/grants/<identifier>.toml` (ADR 0023), which an application cannot write (ADR 0012):

| Field | Meaning |
|---|---|
| `domain` | The publisher hostname this Identifier was pinned under. Absent if the Manifest named none. |
| `key_id` | The pinned key id, 16 lowercase hex digits. Absent if no key is pinned. |
| `public_key` | The pinned public key, standard base64 of the 32 bytes. |
| `pinned_unconfirmed` | True when the pin was taken while the domain could not be checked. Absent or false otherwise. |
| `last_keys` | The key ids from the last well-known document that parsed. Empty if that document listed no keys. |
| `last_reachable` | RFC 3339 UTC time of the last fetch that returned a parseable version-1 document, including one that listed no keys. |
| `dark_since` | RFC 3339 UTC time the domain was first seen dark after a pin existed. Absent when the domain is not dark. |

A grant written before these fields still means "no key pinned", and loads. `oma purge` removes the grant and the held file below.

These fields are how oma tells a dark domain that came back from a domain that stayed reachable. Nothing else is stored for that decision. The old key is not kept once the user accepts a new one.

## Layer 1: the domain

`publisher` in the Manifest (see `docs/spec/manifest.md`) is a hostname: lowercase letters, digits, hyphens and dots, no scheme, no userinfo, no port, no path, no IP address, at most 253 characters. It is optional. A signature is confirmed by the domain only when this field is present and the document below lists the key. A signed Bundle with no `publisher` can still be checked against the public key in its prefix, and the consent prompt says the publisher domain is missing, so the install is unconfirmed. The pin, once the user installs, still rejects a later file signed by a different key.

The document is `https://<publisher>/.well-known/oma-keys` and nothing else. HTTP is not tried. Redirects are not followed: a redirect, a TLS failure, a timeout, or a status other than 200 is a failed fetch. The timeout is three seconds, so a launch does not wait on a dead domain. The body is JSON, at most 16 KiB:

```json
{"version": 1, "keys": [{"id": "0123456789abcdef", "public_key": "<standard base64 of 32 bytes>"}]}
```

`version` must be 1. Unknown fields are ignored. `keys` has at most eight entries. Each `id` must be the key id of that `public_key`, lowercase hex. A duplicate id, a bad id, or a public key that is not 32 bytes makes the fetch a failure rather than a document that listed nothing. An empty `keys` array is a successful parse that lists no keys: the domain has stopped serving keys.

## Layer 2: the signature, the pin, and a new key

Verification runs when a Bundle is installed, when an inbox file is considered, and on every launch. `~/Applications` is writable by the user, so a hash cached beside the file is writable by the same person who could replace the file. A cache there does not help against the tampering the check is for. The installed `oma` is not writable by that user; the check it performs is (ADR 0054).

The private key stays with the author. An application with `updates` can write a file into its inbox. It cannot sign one unless the author signed it first. That is the hole a pin closes. A pin does not make the first install safe. The first install is the user reading the prompt, which is ADR 0007's gate, plus a key that will be required next time.

**The same key.** A file whose signature verifies under the pinned public key, and whose key id matches, is the same publisher's file. When the domain is reachable and lists that key, an upgrade is the rule ADR 0015 already has: Identifier, a higher version, a valid image and Manifest, and a consent prompt only when the new file requests a permission the record lacks. While the domain is dark, that file is held and not applied, as the next section says. The domain listing extra keys beside the pin changes nothing for a file signed by the pin.

**Any other key.** A file signed by a key that is not the pin is not applied and not launched in place of the installed one. `--register` does not open a dialog. It moves a structurally valid higher version to `~/.local/share/oma/held/<identifier>.oma` and writes one line to standard error: `oma: an update for <identifier> is signed by a different key and is held until you accept it.` A signature that does not verify is deleted, not held. The user re-verifies through the first-install consent flow: the buttons are Install, Run once and Cancel, and the permission list is the whole request, not an upgrade's collapsed difference (`docs/spec/consent-dialog.md`). This is the same flow for every new key:

- a key the domain has newly listed, whether or not the pinned key is still listed;
- a key from a domain that was dark and has come back;
- a key the domain does not list, offered while the domain is reachable;
- a key on a Bundle whose Manifest names no publisher.

Install replaces the pin with that key, records consent for the permissions in that Manifest, and installs the file. The previous pin does not remain as a second trusted key. Run once runs the file with a private data directory, records nothing and pins nothing (ADR 0018). Cancel installs nothing and launches nothing. Non-interactive mode fails closed, as a missing permission does (ADR 0016). `oma install --accept-permissions` does not accept a different key; it grants permissions for a file whose key is already the pin, or for a first install the user has pointed it at when no key is pinned yet. Pointed at a different key, it exits non-zero and installs nothing.

A domain listing is not acceptance. The well-known document says which keys the domain is willing to name. It does not move a pin.

**A domain that comes back with a different key is a new publisher.** Suppose the domain becomes unreachable or stops serving keys, and later answers again with a document that does not include the pinned key. oma treats that as a new publisher, not as the one already installed. Neither the old pin nor the new listing lets the new key in. The user goes through the first-install consent flow again, and the prompt says so:

> This domain was unreachable and now signs with a different key. This is a new publisher, and it needs your consent as on a first install.

The threat this closes: an attacker who takes over a lapsed or hijacked registration, waits out the outage, and publishes a different key. Coming back is not evidence that the new key belongs to the publisher the user already trusted. The outage is the gap in which that swap is cheap, so the new key starts over.

oma tells that case apart from a key change while the domain stayed up with `dark_since`, `last_keys` and `last_reachable`:

- The domain is **dark** when the fetch fails, or when the document parses and `keys` is empty. If a pin exists, oma sets `dark_since` to now when it was absent. It does not change the pin. `last_reachable` stays at the previous success.
- The domain is **reachable** when the document parses and `keys` is non-empty. oma sets `last_keys` to those ids and `last_reachable` to now.
- Reachable, and the pinned key is in `last_keys`: the same publisher. oma clears `dark_since` and, if the pin was unconfirmed, clears `pinned_unconfirmed`. A file signed by some other listed key is still a new key, and it takes the first-install flow. The prompt for that one is: "This file is signed by a different key from the one pinned for this application. Installing it asks you to accept that key, as on a first install." It does not say the domain was a new publisher, because the pinned key is still listed and the domain did not go dark.
- Reachable, the pinned key is not in `last_keys`, and `dark_since` is set: a new publisher, as above. `dark_since` stays set and the pin stays until the user installs.
- Reachable, the pinned key is not in `last_keys`, and `dark_since` is absent: the domain stayed reachable and stopped listing the pinned key. That is still the first-install flow for a file signed by another key, with the "different key" prompt, not the new-publisher prompt. The pin does not move on its own. `dark_since` is not set, because the fetch succeeded.

A file signed by the pinned key still belongs to the pin the user accepted, even when the domain has stopped listing it. Launch of that installed file is the section below. A different key never inherits it.

**Unsigned.** An Identifier with no pinned key accepts an unsigned Bundle on Identifier, version and Manifest, and the prompt says the bundle is unsigned. Once a key is pinned, an unsigned upgrade is deleted. It is not held and not offered as a consent choice: accepting it would drop the pin without a new key to show.

**Run once** of a signed file verifies the signature against the public key in the prefix and pins nothing. A signature that does not verify does not run. The domain check still decides whether the prompt says the key is confirmed, unconfirmed, or a new publisher.

## While the domain is dark

Already-installed applications keep running on their pinned key. Launch verifies the installed file against that key and does not need the domain to answer.

oma says that updates cannot be confirmed until the domain answers. The line is written to standard error, which is the user journal when it comes from `oma --register` (the login oneshot and the path units run that command and do not open a window):

> oma: `<domain>` could not be reached; holding the update for `<identifier>` until the domain answers.

`oma inspect` prints the same fact: the domain, `dark_since`, and the path of a held file. When a consent prompt is shown for that Identifier while the domain is dark, the dialog carries the note "The publisher's domain could not be reached, so this signing key is not confirmed."

**An update that arrives while the domain is dark is held, not applied and not deleted.** It is moved to `~/.local/share/oma/held/<identifier>.oma`, one file, replaced only by a structurally valid higher version. Holding is the decision because applying it would install bytes the domain cannot confirm, which is the outage the attacker is waiting on, and deleting it throws away the only copy the application staged. The held file is not executed. When the domain is reachable again:

- the pinned key is listed and the held file verifies under it: apply it as a normal upgrade (ADR 0015), including that upgrade's consent dialog if it requests a new permission;
- the held file is signed by any other key: leave it held and do not apply it; the first-install flow above is how it gets in;
- the held file does not verify, or it is unsigned while a key is pinned: delete it.

A fresh install from a dark domain, or of a signed file whose domain fetch fails before any pin exists, uses the unconfirmed consent path. The signature is checked against the public key in the prefix. The dialog is the first-install flow, with the note that the domain could not be reached and the key is not confirmed. Install pins the key, sets `pinned_unconfirmed` and `dark_since`, and records the domain. Run once pins nothing. Cancel installs nothing. An unsigned fresh install is the unsigned path, with the note that the bundle is unsigned. When that domain later answers with the same key, oma clears `pinned_unconfirmed` and `dark_since`. When it answers with a different key, the new-publisher rule applies: the unconfirmed pin does not let the new key in.

## Launch

Launch of an installed Bundle, after the consent record's permission check:

1. No key pinned, and the file is unsigned or its signature verifies against the public key in its own prefix: launch, subject to consent. A signature that does not verify does not launch.
2. A key is pinned and the file's signature does not verify under it, or the file is unsigned: do not launch. Standard error says verification failed. This is a replaced file, not a dark domain.
3. A key is pinned and the signature verifies under it. Try the well-known fetch.
   - The domain is reachable and still lists the pin: clear `dark_since`. If a held file verifies under the same pin, apply it as an upgrade before launching (the same check `--register` does). If that upgrade requests a new permission, the upgrade dialog comes first. If a held file is signed by a different key, step 4 asks before this launch. Otherwise launch the installed file.
   - The domain is reachable and does not list the pin: if a held file is signed by a different key, step 4 asks before this launch. Otherwise launch this file. It is the key the user pinned. Standard error says the domain no longer lists that key, that this copy still runs, and that a file signed by another key needs consent. Do not change the pin.
   - The domain is dark: launch. Set `dark_since` if it was absent. One line on standard error: `oma: <domain> could not be reached; launching on the pinned key. Updates are held until the domain answers.`
4. A held file signed by a different key is not launched by step 3. On an interactive `oma run` of that Identifier, once the domain is reachable, oma shows the first-install consent for the held file before launching anything. Install pins the new key and launches the new file. Run once launches the held file and leaves the pin and the installed file. Cancel deletes the held file and launches nothing; the installed file remains for a later run. While the domain is still dark, `oma run` of the Identifier launches the installed file (step 3) and does not prompt for the held file. `--register` never opens a dialog and never applies a different key.

A Bundle that fails verification does not start. An upgrade that fails its structural checks, or whose signature does not verify, is deleted, as ADR 0015 already deletes an upgrade that fails its other checks. The cost of hashing the whole file on each launch is unmeasured (`docs/checks.md`). The check stays a whole-file signature. A Merkle tree would be a new ADR if that measurement says the launch check cannot stay.

This does not give `oma` an update client. ADR 0010 already says `oma` verifies the Bundle it was given. The author, or the application shipping bytes the author signed, produces the file. `oma` does not fetch it.

## Layer 3: the download origin

When oma is about to show consent for a file, it reads `user.xdg.origin.url` and `user.xdg.referrer.url` on that file if the filesystem returns them. These are the extended attributes a desktop may set on a download. They are not signed, and the Bundle's publisher can be claimed by anyone who can write the file, so they are not a root of trust.

- No attribute, or an attribute that is not an `https` URL: say nothing about origin. Copying a file, or a browser that does not set the attribute, is normal. A missing origin never blocks.
- The origin URL's host is the publisher domain: say nothing extra. Agreement is the quiet case.
- The host differs: the consent note is "The download origin (`<host>`) is not the publisher's domain (`<domain>`)." The user still decides. A mismatch does not reject a file whose key the user accepts, and a match does not accept a key the user has not.

oma does not invent these attributes and does not copy them onto the installed file as evidence. The pin is the record.

## What was rejected

- **Trust on first use alone**, with no domain. The first prompt would be the only gate forever, and a replaced domain would look like the same publisher. The domain document is the root; the pin is what stops a later file from moving the key quietly.
- **A registry** that maps an Identifier to a key. `oma` would need an operator. ADR 0010 gives it none. The publisher's own domain is the list, fetched when oma already has a reason to look.
- **Accepting a new key because the domain lists it.** That is a quiet key change, and it is the same opening as a hijacked domain that simply edits the document. Listing is evidence the prompt can show. It is not consent.
- **A new key appointed by the pinned key.** That path is not in the format and not in the grant file. Every new key is the first-install flow, so there is nothing for a stolen key to sign that oma would honour as a handover.
- **dm-verity.** A Merkle tree checked by the kernel on every read. It needs device-mapper, which needs `CAP_SYS_ADMIN` in the initial user namespace. The sandbox does not have that (ADR 0052), and neither does an unprivileged install.
- **fs-verity.** Enforcement against a replaced file needs the measurement pinned somewhere the writer of `~/Applications` cannot change, or a key in the kernel keyring. Userspace would still compare that measurement to the pinned key, which is the same check as a whole-file signature, with a filesystem dependency.
- **Per-file hashes inside the directory.** ADR 0007's reserved shape. The mount would show the hash file, or the hash file would sit outside the signed set. The prefix covers the image in one signature.

## Consequences

- ADR 0007 is amended. Unsigned remains allowed only while no key is pinned for that Identifier. The signature lives in the preamble. The trust model is the three layers in this ADR.
- ADR 0015 is amended. An inbox file that the domain cannot yet confirm is held, not deleted and not installed. A file signed by a key other than the pin is not an upgrade until the user takes the first-install flow.
- ADR 0023 is amended. The grant file gains the fields in the table above.
- The consent dialog gains the notes in this ADR (`docs/spec/consent-dialog.md`): unsigned, unconfirmed domain, a different key, a new publisher after an outage, and a download origin that does not match.
- A failed signature does not run. A dark domain does not stop an installed application whose file still verifies under the pin.

## Open questions

1. How long hashing a representative Bundle takes on the development machine (`docs/checks.md`). The launch check stays a whole-file signature until a measurement says it cannot, and that change would be a new ADR.
2. Whether a browser on Omarchy sets `user.xdg.origin.url` on a downloaded `.oma` (`docs/checks.md`). The rule does not depend on it: a missing origin never blocks.
