---
status: proposed
---

# A Bundle signature is an ed25519 preamble, and the trust model is open

**Recommended, and not decided.** Sign the EROFS image with ed25519, store the signature in the reserved 1024-byte prefix (ADR 0050), and pin the public key per Identifier the first time a signed Bundle is installed, showing the fingerprint in the consent prompt. Unsigned Bundles stay installable. A registry is not recommended. ADR 0007 stays the rule until the user picks: v1 verifies no signature, and the consent prompt is the only gate. This ADR amends ADR 0007 only on where a signature would live. ADR 0007 put it in a file inside the Bundle directory, over per-file hashes, which was the shape of a directory (ADR 0022). A sealed image has nowhere inside the tree that is not part of the content, and ADR 0008 already required that a plain reader see the same directory with or without a signature. The prefix is outside the filesystem, so a mount shows the Bundle and no signature file.

Nothing here is built. No trust model is accepted by this record.

## Where it sits

The prefix ADR 0050 stamps, little-endian, all inside the first 88 bytes of the 1024:

| Offset | Field |
|---|---|
| 0 | `OMA1` |
| 4 | header version, `u16` = 1 |
| 6 | header length, `u16` |
| 8 | flags, `u32`. Bit 0 means a signature is present. |
| 12 | algorithm, `u16`. 0 is none, 1 is ed25519. |
| 14 | reserved, `u16` = 0 |
| 16 | key id, 8 bytes, zero if unsigned |
| 24 | signature, 64 bytes, zero if unsigned |

The signed bytes are the header with the 64 signature bytes set to zero, then every byte from offset 1024 to the end of the file. The image is signed, and so are the magic, the version, the flags, the algorithm and the key id, so clearing the flag or swapping the algorithm does not produce a file that still verifies. The signature field itself is the part left out. There is no second file and no trailer. `oma bundle --sign <secret-key>` fills the field. Without `--sign` the flag is clear and the signature bytes are zero. The secret key is a path the author passes. It is never written into the image, the sandbox or the consent record.

The construction is minisign's: ed25519, with minisign's 8-byte key id, so a verifier can rebuild a `.minisig` for the `minisign` tool from the prefix and the image. `oma` verifies the bytes itself. Running or installing a Bundle does not depend on the `minisign` binary.

## Whole file, not a hash tree

A whole-file signature is one hash of the signed bytes and one ed25519 check. It fits a file `oma` already has to read the header of, and it needs no kernel feature.

Rejected for this recommendation:

- **dm-verity.** A Merkle tree checked by the kernel on every read. It needs device-mapper, which needs `CAP_SYS_ADMIN` in the initial user namespace. The sandbox does not have that (ADR 0050), and neither does an unprivileged install.
- **fs-verity.** A Merkle tree on a file the filesystem stores. It needs a filesystem that supports it, and enforcement against a *replaced* file needs the measurement pinned somewhere the writer of `~/Applications` cannot change, or a key in the kernel keyring, which is root. A user who can replace `Foo.oma` can write a new file with a new measurement. Userspace still has to compare that measurement to a pinned key, which is the same check as a whole-file signature, with a filesystem dependency. One kernel that was inspected (Linux 6.12.94, not an Omarchy kernel) has `CONFIG_FS_VERITY` unset. Omarchy's config was not checked.
- **A Merkle tree inside the image, checked by `oma`.** The same trust decision as a whole-file signature, and a reader that checks blocks as they are read. Worth it if hashing the whole file at launch is too slow. That cost has not been measured (`docs/checks.md`). It is the follow-up, not the first design.
- **Per-file hashes inside the directory.** ADR 0007's reserved shape. The mount would show the hash file, or the hash file would be outside the signed set and ADR 0008's problem returns. The prefix covers the image in one signature.

## Trust models

Three were weighed. The recommendation is the first, with the fingerprint from the second shown in the prompt. The third is rejected for v1. The user chooses, including staying with ADR 0007 unchanged.

1. **Trust on first use, per Identifier.** The first time a signed Bundle is installed, `oma` stores the public key and the key id next to the consent record, in the grant file for that Identifier (`~/.local/share/oma/grants/<identifier>.toml`, ADR 0023), which an application cannot write (ADR 0012). A later upgrade must verify under that key. A different key, or an unsigned file where a key is pinned, is not applied silently: it is a consent prompt, and declining offers "Keep previous version" and rolls back (ADR 0015). The private key stays with the author. An application with `updates` can write a file into its inbox; it cannot sign one unless the author signed it first. That is the hole a pin closes. A pin does not make the first install safe. The first install is still the user reading the prompt, which is ADR 0007's gate, plus a key that will be required next time.
2. **The developer key in the consent prompt.** Show the key id as a fingerprint next to the permissions, or the word "unsigned" when the flag is clear. Most people will not compare a fingerprint to anything. It is still the right thing to show, because the prompt is the only moment a person sees the key, and it is how model 1 is explained. It is not a separate trust model. A prompt that shows a fingerprint and then does not pin it would let the next upgrade change keys quietly.
3. **A registry** that maps an Identifier to a key. `oma` would need a network and an operator. ADR 0010 gives it neither. A registry can be its own ADR later if someone is willing to run it. It is not recommended now.

Unsigned stays allowed under the recommendation. An Identifier with no pinned key is today's Bundle: the prompt is the gate, and an upgrade is accepted on Identifier, a higher version and a valid Manifest. Once a key is pinned, an unsigned upgrade does not clear it. Stripping the signature and the flag produces a file that fails the pin, which is the point of signing the flags.

Verification runs when a Bundle is installed, when an inbox file is accepted, and on every launch of a pinned Identifier. `~/Applications` is writable by the user, so a hash cached beside the file is writable by the same person who could replace the file. A cache there does not help against the tampering the check is for. The installed `oma` in `/usr/bin` is not writable by that user; the check it performs is (ADR 0052). A Bundle that fails verification does not start, and an upgrade that fails is deleted as ADR 0015 already deletes an upgrade that fails its other checks. The cost of hashing the whole file on each launch is the measurement in `docs/checks.md`. If it is too high, the follow-up is the Merkle tree, not dropping the launch check.

This does not give `oma` an update client. ADR 0010 already says `oma` verifies the Bundle it was given. The author, or the application shipping bytes the author signed, produces the file. `oma` does not fetch it.

`oma bundle` without `--sign` remains the normal command. Signing is optional while unsigned Bundles are allowed. A signed Bundle with no pinned key yet is how a key becomes pinned, at the install that first shows it.

## Consequences

- ADR 0007 is amended on storage only. "No trust model is chosen yet" remains true, and unsigned remains the implemented behaviour, until the user answers the questions below.
- The consent dialog gains a line for the fingerprint or for "unsigned" when this is accepted (`docs/spec/consent-dialog.md`). It does not gain it before.
- The grant file gains a key and a key id. A grant written before that still means "no key pinned", and loads.
- A Run-once launch (ADR 0018) of a signed file verifies the signature and pins nothing. There is no installed identity to pin to. A failed signature does not run.
- Key rotation is not designed. A new key is a prompt, under the recommendation. A successor key signed by the current one would avoid the prompt, and it is not part of the recommendation.

## Open questions

1. Which model: trust on first use per Identifier with the fingerprint in the prompt and unsigned still allowed, signatures required for every Bundle, a registry, or no signature at all (ADR 0007 unchanged)?
2. If a key can be pinned, may an unsigned Bundle be installed at all?
3. Verify on every launch, or only when installing and when accepting an upgrade?
4. The binary prefix above, or minisign's text signature stored the same way?
5. How does a developer change keys: a consent prompt each time, or a new key signed by the old one?
