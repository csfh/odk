---
status: accepted
---

# Ed25519ctx is verified with crrl

ADR 0060 requires every signed document to be Ed25519ctx (RFC 8032), checked in process. It names the context strings and says `odk-oma` does not shell out. It does not name a crate. This ADR names it. Checked on 2026-10-26 against the published crates, not against a copy already in this repository. Nothing here is linked yet. Issue [#10](https://github.com/csfh/odk/issues/10) adds the dependency when a call uses it.

## Decision

`odk-oma` signs and verifies the document contexts with [`crrl` 0.9.0](https://docs.rs/crrl/0.9.0/crrl/ed25519/struct.PrivateKey.html). The dependency is `default-features = false` and features `std` and `ed25519`. The `omnes` feature stays off. It enables every curve in the crate, and this binary needs one.

Signing is `PrivateKey::sign_ctx`. Verification is `PublicKey::verify_ctx`. Both take the context bytes and the message bytes. In 0.9.0, `sign_ctx` calls `sign_inner(true, 0, ctx, m)`. The `0` is the RFC 8032 prehash flag, so the hash is `dom2` with F = 0 over the message itself. `sign_ph` is the same function with flag `1`. The `dom2` prefix in that source is the 32 bytes of `SigEd25519 no Ed25519 collisions`. The crate's own comments spell the mode `Ed25519cx`. The flag and the prefix are Ed25519ctx.

The context is one of the strings ADR 0060 names (`odk-oma manifest v1`, `odk-oma keys v1`, `odk-oma rotation v1`, `odk-oma revocation v1`, and, when that document exists, `odk-oma algorithm v1`). Each is non-empty and at most 255 bytes, which is what `sign_ctx` accepts. `sign_raw`, `sign_ph`, `verify_raw`, `verify_ph`, and every `verify_trunc_*` function are not used for these documents.

`from_seed` panics unless the slice is 32 bytes. The caller checks the length and returns the parser error. It does not pass a short seed through.

`install.sh` stays plain Ed25519, OpenSSL 3 `pkeyutl`, over the script bytes (ADR 0060 section 8). That check is not this crate, and a signature from one does not verify as the other.

Phase 1 runs the Ed25519ctx vectors in RFC 8032 section 7.2 through `sign_ctx` and `verify_ctx`, then the same message under plain Ed25519 and under Ed25519ph. Those two must fail. A vector that needs no code point below U+0020 is enough for this crate. The canonical-JSON spelling question stays open.

## What was compared

**`ed25519-dalek` 3.0.0**, published 2026-07-06, `rust-version` 1.85. [`SigningKey::with_context`](https://docs.rs/ed25519-dalek/3.0.0/ed25519_dalek/struct.SigningKey.html#method.with_context) is documented as Ed25519ph and returns a [`Context`](https://docs.rs/ed25519-dalek/3.0.0/ed25519_dalek/struct.Context.html) that implements `DigestSigner` and `DigestVerifier`. The digest is SHA-512 of the message, which is prehash, flag F = 1. [`hazmat::raw_sign`](https://docs.rs/ed25519-dalek/3.0.0/ed25519_dalek/hazmat/fn.raw_sign.html) is ordinary Ed25519 and takes no context. [`hazmat::raw_sign_prehashed`](https://docs.rs/ed25519-dalek/3.0.0/ed25519_dalek/hazmat/fn.raw_sign_prehashed.html) is Ed25519ph. There is no Ed25519ctx function. Passing a context to `with_context` and calling it ctx would accept a ph signature as a manifest signature.

Pure Ed25519 over a prefixed message is not a substitute. Pure hashing is `H(prefix || M)`. Ed25519ctx hashing is `H(dom2(0, C) || prefix || M)`. The order differs, so a wrapper around `raw_sign` does not implement the RFC.

**`openssl` 0.10.81.** OpenSSL 3 the C library signs Ed25519ctx: the EVP instance is the string `Ed25519ctx` and the parameter is `context-string` ([EVP_SIGNATURE-ED25519](https://docs.openssl.org/master/man7/EVP_SIGNATURE-ED25519/)). The Rust crate's [`Signer::new_without_digest`](https://docs.rs/openssl/0.10.81/openssl/sign/struct.Signer.html#method.new_without_digest) is plain Ed25519. [`PkeyCtx::set_context_string`](https://docs.rs/openssl/0.10.81/openssl/pkey_ctx/struct.PkeyCtx.html#method.set_context_string) is the ML-DSA context, and it requires OpenSSL 3.5. Using this crate for ADR 0060 would mean new FFI to `EVP_DigestSignInit_ex`, and `libcrypto` inside `odk-oma`. The install script already calls the OpenSSL 3 binary for a different algorithm. That does not put the library in the verifier.

**`aws-lc-rs` 1.18.2.** [`Ed25519KeyPair::sign`](https://docs.rs/aws-lc-rs/1.18.2/aws_lc_rs/signature/struct.Ed25519KeyPair.html#method.sign) takes the message and no context. `ED25519ctx_sign` exists on `aws-lc-sys` as an unsafe C function. That is the same shape as the OpenSSL gap: a C library and FFI, for one call `crrl` already exposes.

## What was rejected

- **`ed25519-dalek`'s context API.** It is Ed25519ph. ADR 0060's documents are Ed25519ctx.
- **A local Ed25519ctx on top of dalek's pure signer.** The hash input is not the pure one with a prefix glued on.
- **The `openssl` crate or `aws-lc-rs`.** The safe Rust APIs do not sign Ed25519ctx. The C entry points do. `odk-oma` does not take a C cryptographic library for this.
- **`crrl`'s other modes.** `sign_raw` is pure Ed25519. `sign_ph` is Ed25519ph. `verify_trunc_*` rebuilds a shortened signature. None of those is a document signature.

## Limits that are accepted

`crrl` 0.9.0 was published on 2024-06-03. Its description is a library for cryptographic research. It declares no `rust-version`. The edition is 2018, so the workspace's 1.96 compiler accepts it. `PublicKey::verify_ctx` is documented as not constant-time, on the grounds that the public key and the signature are public. The bytes ADR 0060 verifies are public. That is accepted. A different crate later is a new ADR, after the RFC 8032 vectors still pass.

## Consequences

- ADR 0060's sentence that `odk-oma` verifies Ed25519ctx itself means this crate, linked as above. The algorithm, the context strings, and "no shell" are unchanged.
- `docs/oma-trust-plan.md` question 1 is this decision. The question number stays, because issues [#10](https://github.com/csfh/odk/issues/10) to [#16](https://github.com/csfh/odk/issues/16) cite the plan's questions by number.
- The crate is not in `Cargo.toml` yet. Adding it runs `tools/check-deps` and updates `crates/deps/Cargo.toml`. It is not a `cargo-machete` exception.
