---
status: accepted
---

# odk-oma looks a publisher's name up itself

ADR 0066 has `odk-oma` fetch with `ureq`, and says `Error::HostNotFound` is the line that says the host does not resolve. Building the fetch (issue #14) showed that `ureq` 3.5.0's own resolver never gives that for a name that does not exist.
- That resolver looks names up through the standard library.
- The standard library reports `getaddrinfo`'s `EAI_NONAME` as an error that has only a message: `failed to lookup address information: Name or service not known`, of kind `Uncategorized`, with no code. This was checked on 2026-10-29.
- The resolver passes that on as `Error::Io`.
- `HostNotFound` comes only from a lookup that succeeded and left no address to use.

So with ADR 0066's setup, a publisher whose name was lost would get the line for a host that could not be reached. ADR 0060 section 2's two lines would be one.

## Decision

`odk-oma` gives `ureq` a resolver of its own, `oma::fetch::Names`. It is the only part of the agent that is not `ureq`'s.
- It calls `getaddrinfo` for the host. It runs on a thread of its own, as `ureq`'s resolver does, so the fetch's three seconds still bound it.
- `EAI_NONAME` and `EAI_NODATA` are `Error::HostNotFound`, the does-not-resolve line. They mean a name that does not exist, or one that exists with no address.
- Every other failure is the could-not-be-reached line. This includes `EAI_AGAIN`, when no server answered: a resolver that did not answer has not said the name is gone.

The connector stays `ureq`'s, and ADR 0066's settings are unchanged.

## Consequences

- ADR 0066 is amended. Its agent has this resolver, and its sentence about `Error::HostNotFound` holds because of it.
- The tests check the mapping of `getaddrinfo`'s codes, and resolve `localhost` from the hosts file. No test looks a name up on the network. That a lost name gives `EAI_NONAME` was seen once by hand on the development machine (`docs/checks.md`).
- `getaddrinfo` reads the system's resolver configuration, as every program does: `/etc/nsswitch.conf`, and `systemd-resolved` on Omarchy. Nothing else is configured.
