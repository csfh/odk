# Review record

| | |
|---|---|
| Model | Grok 4.7 |
| Tool | Grok CLI |
| Date | 2026-10-25 |
| Subject | The six-point trust model in ADR 0055, which [ADR 0060](../adr/0060-a-bundles-publisher-is-the-pinned-root-key.md) supersedes |
| Status | Advisory. ADR 0060 is the decision. This file is the review as it was written, not a specification. |

---

Model ID: Grok 4.7

# oma trust-model review

The six rules fetch an unsigned key list over WebPKI, pin whatever the user clicks through, and treat a valid ed25519 signature as proof that those bytes may run. They never define the signed span, a package identity, a version, an expiry, or a rotation ceremony. Update authority and the domain name are easy to conflate. The file format is a shell script or an ELF with an EROFS appended, so the kernel can execute attacker bytes before oma's verifier exists in the path.

## Domain hijack and expiry

**Attack.** The registrant loses `publisher.example` (expiry, registrar compromise, or a forced transfer). The new registrant serves `https://publisher.example/.well-known/oma-keys` with a key they control and serves signed bundles for the same package names. Two outcomes depend on an ambiguity in points 1, 5, and 6. If a consent click rebinds the domain to the new key, the click hands over the existing install base. If the new key is only a new publisher, the old installs stay pinned to a key this domain no longer publishes, and they never update again. A partial takeover is enough: a writable path on the live site, a dangling subdomain, or a CMS that will store `/.well-known/oma-keys`.

**Preconditions.** The attacker can publish HTTPS on a name the user, or an already-pinned install, treats as the publisher. For the rebind case, one consent affirmation.

**Impact.** Rebind: silent malicious updates after a single prompt that looks like maintenance. No rebind: permanent freeze of every install from that publisher, plus a fresh publisher sitting on the name users already know. Either reading is a design bug because the spec does not say which one is required.

**Fix.** Normative rule: the installed publisher is the pinned public key, not the domain. Domain control never moves update rights. A new key on that URL starts a publisher with zero installed packages. The only rebind is a rotation document signed by the current key (or by a threshold offline root) naming the successor key, the package ids it covers, and an expiry. After a domain loss the old key still signs updates hosted anywhere. Users who still have the old key keep receiving updates. The successor domain is display text.

## TLS and CA compromise

**Attack.** A mis-issued certificate for the publisher name (compromised or coerced CA, short-lived DNS or HTTP hijack that passes ACME domain validation, or a local trust anchor pushed by malware or an enterprise proxy) lets the attacker serve a different `oma-keys` during the fetch. On first install the user consents to the real name and pins the attacker's key. After install, the same substitution works if the client treats the fetched file as a live root and adds or replaces keys. A targeted client can be given a different file than the rest of the world. Nothing in the six points detects that split.

**Preconditions.** WebPKI acceptance of an attacker certificate for that name, and a client that will pin or union whatever the URL returns. First install is enough. Existing installs are exposed when the key set is re-read as authority.

**Impact.** A signer the publisher never created. Every package name under that pin becomes the attacker's. Interception by a TLS-inspecting proxy does the same to every oma install on that network, including `install.sh`.

**Fix.** WebPKI delivers the key list once, at first consent. Persist the key id. Later fetches may add a key only when a rotation signature from the current root verifies. Require SCTs and reject certificates the CT logs do not show. Refuse redirects off the publisher origin. Send `Cache-Control: no-store` and ignore cache freshness for this URL. Cap the body (a few kilobytes), require one schema, reject duplicate fields. Record the key set and each release in an append-only log and require an inclusion proof so a split view is a public event. A locally added root is a hard stop in the installer unless the user turns interception on for that run.

## DNS attacks

**Attack.** DNS alone does not break a correct TLS stack. It becomes a signing oracle where domain-validated issuance treats DNS as proof of control: cache poisoning or a rogue resolver long enough for ACME, a BGP announcement in front of the publisher's authoritative servers, or a lookalike NS delegation after a registrar account is phished. Split-horizon DNS plus a captive portal also produces the "domain is dark" branch (below) and holds updates. IDN and punycode are a DNS-layer lookalike channel.

**Preconditions.** A publisher name that is not locked down at the registrar, or a resolver a victim will use, or a moment of DNS control sufficient for a DV certificate.

**Impact.** Same as serving a forged `oma-keys`. A resolver that only blackholes the name gets a freeze rather than code execution.

**Fix.** Registrar lock, DNSSEC on the publisher zone, and CAA limited to CAs the publisher actually uses. The client still pins the application key after first use, so a later DNS win does not move trust. Treat NXDOMAIN, SERVFAIL, and a timeout as three different states. NXDOMAIN on a previously pinned publisher is a suspected name loss: keep running the installed app, refuse updates, and show that the name no longer resolves. Do not display a punycode name only in Unicode form.

## Publisher key theft

**Attack.** The ed25519 private key leaks from a laptop, a backup, or CI. `oma-keys` does not change, so points 5 and 6 never fire. There is no expiry and no revocation. The attacker signs a new manifest-compatible bundle. Any delivery path works, because verification is on the signature, not the socket: a mirror, a poisoned CDN object, a file sent to the user, or a later write on the real download URL.

**Preconditions.** A copy of the long-lived signing key. Write access to the publisher origin is not required.

**Impact.** Every install pinned to that key takes the attacker's build as a normal upgrade. The theft stays valid for the life of the install base. Point 6 makes this worse for honest publishers: they cannot retire the key without sending every user through a first-install prompt, so they will keep using a key they no longer trust.

**Fix.** Two roles, as in TUF or APK Signature Scheme v3. An offline root, ideally threshold-signed, lives in `oma-keys` and is what the user pins. An online signing key signs releases, carries `notAfter` (days or weeks), and is authorized by a root signature. Ship a revocation list signed by the root. Clients that are online refuse revoked or expired signing keys for upgrades. Already-installed apps keep launching. Publish the root ceremony out of band (a short fingerprint on a site the project already has, and in the transparency log). A CI leak then expires on its own and can be revoked immediately. Sigstore-style short-lived certificates bound to a workflow identity are a workable substitute for the online key, with Fulcio's root pinned in the client and a Rekor-style proof stored next to the bundle.

## Rollback and downgrade

**Attack.** The signer has produced many valid bundles over time, including ones with known vulnerabilities. Nothing in the six points names a version or a high-water mark. A mirror, a cache, or a stolen key serves an older signed file. `oma` checks the signature at upgrade time, accepts it, and replaces the newer install. The same pattern on `oma self-upgrade` is worse: an older client with a verifier bug becomes the program that checks every future bundle.

**Preconditions.** At least one older signed artifact the attacker can obtain. For a stolen-key downgrade, the ability to sign a forged older version. No privilege on the victim machine.

**Impact.** Known-vulnerable code, reintroduced as an official upgrade. A downgraded oma client can disable checks for every publisher.

**Fix.** Put a monotonic integer in the signed manifest. The client stores the highest accepted version per `(key id, package id)` and rejects anything lower. Store oma's own high-water mark in a root-owned or user-owned state file outside the binary being replaced, and refuse to exec a client version below it after the outer launcher has checked the baked-in root signature. Equal-version replacement is allowed only when the payload digest matches the one already installed.

## Replay of old signed bundles

**Attack.** Rollback is the upgrade path. Replay is every other path. A file signed years ago still verifies at launch, because "verify the signature" is not "verify this signature is current". An attacker redistributes that file as a fresh download, or a cache replays it to a machine that is installing for the first time. Launch-time verification rechecks the same old signature and succeeds. A bundle with no `expires` field is a credential that never dies.

**Preconditions.** A byte-for-byte copy of any previously signed bundle from a key the victim will pin or has pinned.

**Impact.** First-time installs of vulnerable releases. Side-loads that skip the publisher's current tree. Combined with a freeze, the victim never walks forward to the fixed release.

**Fix.** The manifest carries `issued-at` and `expires-at` covered by the signature. Clients reject expired manifests for install and upgrade. Launch of an already-installed copy stays allowed when the machine is offline, so expiry does not brick an air-gapped app, and the UI shows that the running version is past its signature window. A short-lived timestamp role (TUF's timestamp/snapshot pattern) signs "package P is at version N as of time T, and T is only valid for a few days." When that stamp is fresh, any other version is a replay. The client also stores the newest valid `issued-at` it has seen and refuses older stamps.

## TOCTOU between verify and mount or exec

**Attack.** Verification opens a path, hashes it, then erofsfuse or `exec` opens the path again. A writable directory, a symlink swap, or a second hard link lets the bytes change between the two opens. A `verified` xattr or a sibling stamp file is forgeable, so a later launch that trusts the stamp skips the hash. The format makes a second, larger hole: a file whose first bytes are a valid ELF is executed by the kernel as a native binary, and a file whose first bytes are `#!` is executed as a script. `binfmt_misc` is not consulted for either case. The preamble runs as the user who launched the file, and oma's signature check is not on that path. An unsigned preamble glued to a legitimately signed EROFS has the same effect if the signature does not cover the preamble.

**Preconditions.** For the race: the bundle path is replaceable by the attacker between check and use (download directory, world-writable cache, or another user on the machine). For the preamble: the user or a file manager executes the `.oma` file, which the design invites by making it a shell or ELF program.

**Impact.** Arbitrary code at launch, with a valid signature on a different byte string than the one that ran. A stolen preamble swap needs no signing key at all.

**Fix.** The container format starts with a magic that is not ELF and not a shebang, so the kernel will not run it. The only interpreter is the already-installed `oma` binary, registered for that magic. Verify on a file descriptor opened `O_RDONLY|O_NOFOLLOW`, require a regular file, hash and check the signature on that fd, copy into a sealed memfd or `O_TMPFILE`, and mount and exec only from the sealed fd. Cover the preamble bytes in the signed manifest. Do not store trust in an xattr. `binfmt_misc` is registered by root, points at a path owned by root and covered by the client's own signature, uses a fixed argument vector, and does not set the credentials flag.

## Offline behaviour

**Attack.** Point 4 collapses every reachability failure into "keep running, hold updates": NXDOMAIN, TCP blackhole, TLS failure, HTTP 5xx, a captive portal, and an attacker dropping packets to the publisher. A network adversary can freeze security updates for as long as they sit on the path. The client cannot tell "the publisher has not shipped anything" from "I am being prevented from looking". There is no revocation list to consult, so offline is also when a stolen key keeps working. First install has no defined offline behavior. If the client falls back to a public key embedded in the bundle, the bundle is choosing its own root of trust.

**Preconditions.** Ability to make the well-known URL fail for the victim, or a user who installs from a USB copy while offline.

**Impact.** Indefinite exposure to fixed bugs, with the UI looking like the specified graceful path. Offline first install that trusts an embedded key is a trust-root bypass.

**Fix.** Launch from the local pin with no network. Refuse install and upgrade when the timestamp role cannot be fetched fresh. Show staleness once the last good timestamp is older than a threshold the manifest names (a default of 30 days is a reasonable policy). Keep the three failure classes separate: name gone, transport failure, and "stamp says you are behind". Offline first install is refused. The embedded key id is a lookup hint into the local pin store and is never a trust anchor.

## Preamble and EROFS parsing

**Attack.** The consent UI, a file manager, or the installer mounts or walks the image to read an icon, a title, and a version before the signature is checked. erofsfuse then parses attacker-controlled bytes in a process that is part of the host, not part of the app. A crafted image targets that parser: integer overflow, a decompression bomb, a symlink or device node if the mount allows it, a setuid bit if a privileged helper performs a real mount. A publisher the user later trusts can still ship an image whose purpose is to exploit the host parser, which matters the moment the app itself is sandboxed and the parser is not. Zip-slip style extraction, if any install path unpacks instead of mounting, writes outside the destination directory.

**Preconditions.** A parse or mount of bytes whose signature has not yet verified, or a mount helper with more privilege than the user running the app.

**Impact.** Code execution as the mount helper, including root if the helper is privileged, before any consent. Denial of service via decompression. Host breakout that a future app sandbox does not contain.

**Fix.** A fixed-size, simple-to-parse header in front of the image holds the manifest. Verify that header before any filesystem code runs. Mount only after consent and only from the sealed fd. Use unprivileged erofsfuse with `nosuid,nodev`, a size cap taken from the signed manifest, and a seccomp profile around the daemon. Do not ship a setuid mount helper. If the product later sandboxes apps, sandbox the parser with at least the same strength. Refuse device nodes, fifos, and setuid files in the image. Extract nothing: the install is the sealed blob plus the pin.

## Consent fatigue

**Attack.** Points 5 and 6 route every legitimate key change through the first-install prompt, because rotation does not exist. Publishers who rotate, and publishers who lose a domain, generate the same dialog as an attacker. Users learn that the dialog is a step in updating. A second bug feeds the dialog from inside the image: the display name and icon are attacker-chosen, and the real key fingerprint is absent or visually secondary. "Trust example.com?" with a familiar icon is a different decision from "pin `a3:9f:…` for package `editor`". A single affirmation that means "this domain, all packages, forever" turns one install into a standing grant.

**Preconditions.** A prompt the user has seen before for benign reasons, or a UI that renders unverified name and icon bytes.

**Impact.** The only first-install control becomes a click-through. After that click, background upgrades need no further consent, which is what a stolen or substituted key wants.

**Fix.** Make honest rotation a signed ceremony so the first-install prompt is rare. When it appears, build it only from signed manifest fields and from the retrieval URL the client actually used. Show the key fingerprint, the package id, the version, the domain in Unicode and punycode, and the retrieval host. Scope the stored consent to `(key id, package id)`. A second package from the same key is a new, smaller prompt that names both ids. If the new domain is confusable with a pinned one, say so in the dialog and require an explicit second confirmation. Record denials and do not re-ask on a retry loop.

## Typosquatting and lookalike domains

**Attack.** Register `publisher.example` lookalike (`pubIisher.example`, `publisher.examp1e.com`, a mixed-script IDN). DV issuance is automatic, so HTTPS and a well-known key URL exist. The bundle is honestly signed by a key the attacker owns. Point 3 says `user.xdg.origin.url` never blocks, and that xattr is writable by anyone who can `setxattr`, and it falls off on copy to a filesystem without xattrs. A download from the lookalike can carry an origin xattr of the real publisher, or the reverse. The signature check succeeds because the attacker is a "real" publisher under rule 1.

**Preconditions.** A registrable confusable name, a DV certificate, and a user who installs from search, an ad, or a mistyped URL.

**Impact.** A fully valid install of attacker code. No cryptographic control fails. If consent is per domain string and the string looks right in the UI font, the pin is stored under a name the user cannot reliably read.

**Fix.** Identity in the UI is the fingerprint and the package id. Compare new domains to pinned publishers with a confusable-string check (mixed script, punycode, edit distance) and warn on a hit. Bind the download URL's host to the publisher host inside the signed manifest. A mismatch is a hard stop, not a hint. Ignore `user.xdg.origin.url` for every decision. It may be shown labeled as an unverified leftover of the transport. A curated registry with its own pinned root is an optional additional root for users who want a store. It does not replace per-publisher pins.

## curl | sh bootstrap and self-upgrade

**Attack.** The documented install is `curl --proto '=https' --tlsv1.2 -fsS https://<oma-domain>/install.sh | sh`. The script runs while it is still downloading. Nothing checks a signature or a checksum before the shell starts. `--proto '=https'` and a TLS 1.2 floor are real constraints, and they still leave the trust decision on WebPKI for the oma domain plus whatever that script does. The same domain's website is what users copy the command from, so a compromise of the site, its CDN, or a certificate for that name changes the installer and the instructions together. People routinely feed the script to a root shell. `oma self-upgrade` is unspecified. If it re-fetches and pipes the same script, every upgrade repeats the hole. If it replaces the oma binary and then uses the new binary to decide what is trusted, one malicious upgrade owns the verifier.

**Preconditions.** A moment of control over the install URL or its certificate, and a user running the documented command. Root if the user escalates the pipe.

**Impact.** Code execution as the installing user or as root, before any pin exists. A malicious client then pins attacker publisher keys and persists through binfmt and future self-upgrades.

**Fix.** First install: download to a file, print a root-key fingerprint that was announced out of band, verify a signature on the script with a key the user can compare to that fingerprint, then execute. Refuse to run under `sudo` unless the user passes an explicit flag. Install per-user by default. Self-upgrade: the running binary contains the offline root hash. It accepts a new client only when that root's signature verifies, the manifest version is above the stored high-water mark, and the payload digest matches. The launcher that checks the mark is updated by the same signed path and is not a shell script fetched from the network. Publish the root fingerprint in the transparency log and on a second channel (a release signed by a different maintainer key, or a DNS TXT the user can compare once).

## Other weaknesses

**Unsigned key-list union.** Point 5 says a different key is a new publisher. A file that still contains the old key and adds another one is not "a different key" under a literal reading. A client that unions every key in the file silently widens the signer set for the whole domain. The same file is unsigned, so the only integrity is TLS on that request. Fix: the pinned object is an exact key-id set. Additions and removals both require a root signature. Unknown schema versions are rejected.

**No signed manifest, so mix-and-match.** A signature over an opaque blob does not name the package, the version, the entrypoint, the update URL, or the digest of the preamble separately from the image. An attacker who can move bytes can pair a signed image with a different preamble, point the updater at a different host, or present package `malware` under the file name of package `editor`. Fix: one manifest structure, signed as a unit, with package id, version, entrypoint path, payload digest, preamble digest, key id, timestamps, and update URL. The client checks each field against the local pin and the high-water mark.

**Equivocal history.** Two clients can be shown two valid histories and never notice. Fix: inclusion proofs for the key set and for each release, checked when online, with witnesses the client already has hashes for.

**Standing grant is the whole user account.** A correct signature means "run this as the user", with the user's files, keys, and session. The trust model has no confinement story, so a single bad pin is total account compromise, and a parser bug in the host is outside any later app sandbox. Fix: launch through a user namespace or landlock profile that defaults to the bundle mount, a private config dir, and an explicit network grant named in the manifest. The grant is part of what the consent dialog shows.

**Shared-machine trust store.** A world-writable cache, a system-wide "trusted domains" file, or a binfmt registration writable by a non-root user lets one local account approve a publisher or replace the interpreter for another. Fix: per-user pin stores, mode `0700` bundle directories, root-owned binfmt and launcher.

**Fail-open gaps the spec leaves blank.** Launch when the pin file is missing, when the signature algorithm field is unrecognized, or when the image hash mismatches, is not specified. An implementer in a hurry logs the error and execs. Fix: any mismatch, unknown algorithm, or missing pin is a hard failure. The only successful launch path is the one through the sealed fd.

**Algorithm freeze.** Point 6 forbids rotation, which also forbids a planned algorithm change. A future break of ed25519 has no migration that users can survive without the fatigue prompt, so the migration will be delayed. Fix: the manifest names the algorithm. The root signs an algorithm policy. Dual signatures during a transition are a normal rotation, not a new publisher.

## Fixes ranked by severity

### Critical

1. Pin the public key, not the domain. Domain control and a fresh `oma-keys` never inherit installed packages. Rebind only via a rotation document signed by the current root, naming the successor and the package ids.
2. Split an offline threshold root from a short-lived online signing key. Serve a root-signed revocation list. Refuse revoked or expired signing keys on install and upgrade. Keep launch of already-installed apps working.
3. Sign one manifest: package id, monotonic version, entrypoint, payload digest, preamble digest, key id, `issued-at`, `expires-at`, update URL. Persist a high-water mark per `(key id, package id)` and reject anything lower or expired.
4. Make the file inert. Magic must not be ELF and must not be a shebang. Verify, seal, mount, and exec on one file descriptor opened `O_NOFOLLOW`. Cover the preamble in the manifest. Register `binfmt_misc` only for that magic, with a root-owned interpreter, a fixed argument vector, and no credentials flag.
5. Stop piping `install.sh` into a shell. Verify a signature against an out-of-band root fingerprint, then run. Self-upgrade checks a root hash compiled into the binary and a high-water mark stored outside the replaced binary.

### High

6. Treat a later `oma-keys` fetch as data, not authority. No cross-origin redirects, no cache trust, strict size and schema, SCT required. Ignore added keys until a root signature authorizes them. NXDOMAIN on a pinned name is a suspected takeover, not a quiet freeze.
7. Close replay with a short-lived timestamp role that names the current version. When the stamp is fresh, other versions do not install. When it is unavailable, launch stays local and upgrades wait. Surface staleness past the manifest threshold.
8. Hard-stop when the download host and the signed publisher host differ. Ignore `user.xdg.origin.url`. Consent UI is built only from the signed manifest and the URL that was fetched. Show fingerprint, package id, version, and punycode. Scope consent to `(key id, package id)`. Warn on confusable domains.
9. Mount only after the manifest verifies, from the sealed fd, unprivileged, `nosuid,nodev`, size-capped, parser under seccomp. No setuid helper. No extraction onto the host filesystem.
10. Give oma's own updater the same manifest, rollback, and pin rules as every other package, with the root hash inside the current binary so a downgraded client cannot be the thing that weakens policy.

### Medium

11. Publish key sets and releases to an append-only log, and check inclusion when online, so a split view of `oma-keys` or of a bundle is detectable.
12. Sandbox the launched app (user namespace or landlock) with the grants written into the manifest and shown at consent. Sandbox erofsfuse to the same standard.
13. Keep pins and bundles per-user and unreadable by other accounts. Root owns the binfmt registration and the launcher path.
14. Put an algorithm id in the manifest and an algorithm policy on the root, so a later migration is a dual-signed rotation.
15. Fail closed on unknown algorithms, missing pins, digest mismatches, and parser errors. Never honor a `verified` xattr or an embedded public key.

### Low

16. If the origin xattr is shown at all, label it as an unverified transport leftover.
17. Document that a local TLS intercept root stops the installer, with an explicit override for that run.
18. Keep a monotonic last-seen timestamp next to the pin. Allow small clock skew. Reject stamps and manifests older than the last accepted time so a wound-back clock cannot revive an expired signature.
