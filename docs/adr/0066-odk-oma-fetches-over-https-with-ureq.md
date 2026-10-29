---
status: amended by ADR 0077 and ADR 0086
---

# odk-oma fetches over HTTPS with ureq

ADR 0060 has `odk-oma` fetch two things. At first contact with a publisher it fetches `https://<publisher>/.well-known/oma-keys` and `oma-keys.sig` (section 3): HTTPS only, no redirect followed, a three-second timeout, a body of at most 16 KiB. A host that does not resolve and any other failure are two different lines (section 2). `odk oma self-upgrade` fetches a release from the install host (section 8). `odk-oma` has no HTTP client, and ADR 0060 does not name one. The owner chose `ureq` on 2026-10-27, on the condition that it is a good and trustworthy choice. The facts below are that check. Issues [#14](https://github.com/csfh/odk/issues/14) and [#15](https://github.com/csfh/odk/issues/15) add the dependency when a call uses it.

## Decision

`odk-oma` fetches with [`ureq` 3.5.0](https://docs.rs/ureq/3.5.0/ureq/), blocking, on the calling thread, with no async runtime. The dependency is `default-features = false` with the features `rustls` and `platform-verifier`. TLS is rustls with the `ring` provider. The roots are the system's (`RootCerts::PlatformVerifier`): on Omarchy, the `ca-certificates` package, which every TLS client on the machine trusts. The Mozilla bundle that `ureq` would otherwise compile in is not the trust store.

One agent is configured, and only these settings differ from the default:

| Setting | Value | ADR 0060 |
|---|---|---|
| `https_only` | `true` | HTTPS only |
| `max_redirects` | `0` | No redirect is followed. A `3xx` is returned as itself, and it is a failed fetch like any status but `200`. |
| `timeout_global` | 3 s for `oma-keys`; the release's own for `self-upgrade` | Section 3 |
| body `limit` | 16 KiB for `oma-keys` and for `oma-keys.sig` | Section 3 |
| `proxy` | `None` | The host the dialog names is the host connected to. The environment's `HTTPS_PROXY` is not read. |

`Error::HostNotFound` is the line that says the host does not resolve. Every other error, a status that is not `200`, and a body over the limit are the line that says it could not be reached. Neither changes the pin. Tests use `ureq`'s resolver and transport hooks, and no test reaches the network.

A fetched body is bytes until ADR 0060's checks pass. TLS says where the bytes came from. The root's signature says whether they count.

## Why ureq

Checked on 2026-10-27:

- **It is maintained.** It was first published in 2018, its last release was on 2026-10-21, and it has one long-standing maintainer ([algesten/ureq](https://github.com/algesten/ureq)). It is MIT or Apache-2.0.
- **It is widely used.** crates.io counts 244 million downloads, 76 million of them recent.
- **No `unsafe`.** `src/lib.rs` in 3.5.0 has `#![forbid(unsafe_code)]`.
- **No advisory.** The RustSec database has no entry for `ureq` (`crates/ureq` does not exist in `rustsec/advisory-db`). The advisories that searches turn up are for older `rustls` and `webpki` releases, which `ureq` depends on. The `audit` job checks the whole tree on every pull request and every Monday, and a new advisory fails it.
- **Little that is new.** `rustls` 0.23, `ring` 0.17 and `rustls-native-certs` are already in `Cargo.lock` through GPUI. The main additions are `ureq` itself, `ureq-proto` and `rustls-platform-verifier`. The full list is the lockfile diff of the pull request that adds it.
- **Nothing to install.** It needs no system package beyond `ca-certificates`, which a stock Omarchy has. The owner's rule of the same day is that ODK depends on nothing Omarchy does not already install.

## What was rejected

- **`reqwest`.** It brings tokio and hyper's client for two small blocking requests.
- **Running the system's `curl`.** It adds no crate, but it puts a second program in the trust path. Failures would have to be read from exit codes, and a curl built differently answers differently.
- **HTTP/1.1 written in `odk-oma` over `rustls`.** That is more code of ODK's own to keep correct than one well-used client.
- **`minreq` and `attohttpc`.** They do the same job with far fewer users.

## Consequences

- ADR 0060 sections 2, 3 and 8 are unchanged. This record names the client and how it is set up.
- Adding `ureq` runs `tools/check-deps` and updates `crates/deps/Cargo.toml`. It is not a `cargo-machete` exception.
- A proxy is not supported. Someone who needs one asks for it, and that is a new ADR.
