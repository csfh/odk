---
status: accepted
---

# odk-oma keeps each pinned root's key set

ADR 0060 section 2 says a file already in hand is an upgrade even when the publisher's host does not resolve, if a release key the pinned root still authorizes signed it. To know that offline, `odk-oma` has to keep the set the root signed. The trust plan's question 14 left open whether it keeps the signed `oma-keys` body. On 2026-10-29 the owner agreed that it does, provided security is not weakened.

## Decision

- **What is stored.** When a fetched `oma-keys` is accepted for a root that is pinned, `odk-oma` keeps its body and the root's signature beside that root's records: `grants/<root key id>/oma-keys` and `oma-keys.sig`. Its version goes in `oma-keys.version`, which is ADR 0060 section 3's mark. A set older than the stored version is never stored.
- **Every use checks it again.** The stored copy is not trusted because it is stored. Each use verifies the signature under the pinned root key. A body whose version is not the stored version is refused, so an older set the root really did sign, swapped in, is refused. The release key must be in the set and inside its `not_after`.
- **When it is used.** Only when a fetch fails, and only for a file already in hand: an upgrade from `odk oma install`, `--register` or an inbox. A first contact still needs the fetch, and no root is pinned from a stored set. When the host does not resolve, section 2's line is written.

## What it does not change

- **The boundary.** The files sit in the state directory beside the pin, which sandboxed applications cannot write (ADR 0012). Anything that could replace the stored set could replace the pin.
- **Freshness.** Offline, a release key the root dropped after the last fetch still authorizes, until its `not_after`. ADR 0060 accepts this for a file in hand. Revocation (question 2) is what stops a compromised key sooner.
- **Secrecy.** Nothing secret is stored: the body and the signature are what the publisher serves to anyone.

## Consequences

- The trust plan's question 14 is answered for the key set. The grant file's field names stay as phase 4 wrote them.
