# Review record

| | |
|---|---|
| Model | GPT-6.1-Sol |
| Tool | Codex CLI |
| Date | 2026-10-25 |
| Subject | The six-point trust model in ADR 0055, which [ADR 0060](../adr/0060-a-bundles-publisher-is-the-pinned-root-key.md) supersedes |
| Status | Advisory. ADR 0060 is the decision. This file is the review as it was written, not a specification. |

The reviewer's own note that an exact model id was not exposed to that session is left as written.

---

**Exact running model ID:** unavailable to this session. The runtime instructions identify me as Codex based on GPT-6, but expose no exact model ID that I can verify.

**Security review of oma's proposed trust model**

This reviews the supplied design, without implementation access. Findings with implementation preconditions are conditional. Severity reflects the impact if those preconditions hold; root execution is called out separately.

The design leaves publisher continuity, release freshness, and enforcement before bundle execution unspecified. HTTPS can establish control of a domain during enrollment. Existing installations need a persistent publisher identity anchored in locally trusted keys. Adopting authenticated key succession would change points 5 and 6 of the proposal.

1. **Critical: oma's bootstrap and self-upgrade can compromise every later verification.**

   **Attack:** Replace `install.sh` through domain, hosting/CDN, or TLS compromise. The script installs a verifier that accepts attacker bundles. An unauthenticated `oma self-upgrade` creates the same result later.

   **Preconditions:** Control of the delivery path accepted by HTTPS; for self-upgrade, missing independent release authentication.

   **Impact:** Code execution as the installing user, or root if the installer obtains elevation; permanent control of app trust decisions.

   **Fix:** Prefer an authenticated distribution package. Otherwise obtain oma's initial release key through an independent authenticated channel, download the complete release, verify it, then execute. Pin an offline oma release root and authenticate self-upgrades with rollback protection and recovery keys. A checksum or key fetched beside the compromised script adds no independent trust. Streaming into `sh` also lets a partial transfer execute commands before curl reports failure. The supplied HTTPS/TLS flags constrain transport; they provide no separate release identity. [curl options](https://curl.se/docs/manpage.html)

2. **Critical, conditional: the bundle preamble runs before its own verifier.**

   **Attack:** Replace a shell/ELF preamble's verification code, invoke it through `sh`, or use a desktop launch path that bypasses oma. Malicious code runs before any signature check.

   **Preconditions:** A launch path executes bundle-controlled instructions before a trusted external verifier. A MIME association alone does not enforce verification.

   **Impact:** Arbitrary code execution despite the promise to verify every launch.

   **Fix:** Route supported launches through an installed, trusted interpreter that authenticates the artifact before executing any bundle code. Use a distinct inert bundle format if necessary. Audit terminal, file-manager, desktop-entry, and binfmt paths. State the enforcement boundary: a user allowed to execute arbitrary scripts can deliberately bypass a user-space launcher. `binfmt_misc` matches magic bytes or extensions, and invokes a configured interpreter. [Kernel documentation](https://docs.kernel.org/admin-guide/binfmt-misc.html)

3. **Critical, conditional: unsigned executable bytes or ambiguous signature coverage.**

   **Attack:** Keep a signed EROFS image while replacing its preamble, image offset, launch command, or appended executable data. Exploit different interpretations of duplicate manifest fields or multiple embedded images.

   **Preconditions:** The signature covers only part of what affects execution, or verification and loading parse the artifact differently.

   **Impact:** An accepted signature authorizes attacker-selected execution.

   **Fix:** Specify one canonical envelope. Authenticate every execution-relevant byte, including preamble and image, with a precisely excluded signature field or detached signature. Bind publisher identity, app ID, release counter, platform, lengths, offsets, entry point, and permissions. Reject overlapping regions, duplicate fields, unsupported formats, and extra bytes. Use explicit signature domain separation and a maintained Ed25519 implementation.

4. **Critical, conditional: verification and mount/exec use different bytes.**

   **Attack:** Swap a pathname or symlink after verification, modify the same inode through another writable descriptor, or mutate the backing image after a read-only FUSE mount starts serving it.

   **Preconditions:** The attacker can alter the source or a parent directory; the loader reopens it or the backing bytes remain mutable.

   **Impact:** Execution of bytes that never passed verification. A read-only mount does not freeze its backing file.

   **Fix:** Copy into a fresh object isolated from attacker writes, make it immutable, then verify that object. Mount from the same object and retain it for the mount's lifetime. One option is a memfd with write, grow, and shrink seals, passed to a loader that supports descriptors; otherwise use a store writable only by the trusted service. An open descriptor alone does not prevent in-place writes. [Linux file sealing](https://www.man7.org/linux/man-pages/man2/memfd_create.2.html)

5. **High: domain expiry, registrar takeover, or delegated hosting takeover resets identity.**

   **Attack:** Acquire the old domain or its hosting, publish a new `oma-keys`, and present malware under the previous app's name. Reuse existing app data after the user accepts the new publisher.

   **Preconditions:** Domain/hosting control plus fresh enrollment, acceptance of a replacement publisher, or automatic replacement of a cached key.

   **Impact:** Impersonation for new users; existing users lose continuity if a new identity inherits the old installation. Correct pinning blocks silent replacement.

   **Fix:** Persist the enrolled publisher root and app identity across outages, uninstall/reinstall, and domain changes. Hold updates on an unexplained key change. A separately enrolled publisher gets a separate namespace and no inherited permissions or private app data. Domain recovery alone must not authorize migration. Use old-root-authorized succession or an independently enrolled recovery root.

6. **High: TLS/CA compromise can supply the initial publisher key.**

   **Attack:** A CA mis-issues a certificate, a trusted TLS interception root is abused, or an attacker steals a usable server certificate key and intercepts traffic. Serve an attacker-controlled enrollment key and matching bundles.

   **Preconditions:** A certificate chain the client accepts plus control of the connection; successful initial enrollment or user-approved key replacement.

   **Impact:** Authentic-looking attacker publisher identity. Previously pinned keys prevent silent takeover unless trust is refreshed from HTTPS alone.

   **Fix:** Retain enrolled roots independently of live TLS. For stronger first-install assurance, distribute a publisher fingerprint through an independently authenticated channel or trusted catalog. Enforce hostname and chain validation without an insecure fallback. Certificate transparency can aid detection; it cannot independently establish publisher continuity. [TLS service identity rules](https://www.rfc-editor.org/rfc/rfc9525.html)

7. **Medium: DNS attacks suppress updates; some also enable enrollment takeover.**

   **Attack:** Poison DNS or divert resolution to block updates. If the attacker also obtains an accepted certificate, redirect enrollment to attacker keys.

   **Preconditions:** DNS/resolver control. Content substitution additionally requires defeating TLS authentication or compromising the authentic endpoint.

   **Impact:** Update starvation; publisher impersonation under the additional TLS precondition. DNS redirection alone cannot forge a correctly authenticated HTTPS response.

   **Fix:** Preserve normal TLS validation and publisher pins. Treat resolution failures as update failures, with bounded retries and visible staleness. DNSSEC validation can reduce forged DNS answers where deployed; it does not fix registrar takeover or a compromised origin.

8. **Critical: theft of a signing key, or control of its signing service, produces valid malware.**

   **Attack:** Steal an Ed25519 private key from CI, or compromise the release pipeline and request signatures for malicious output without extracting the key.

   **Preconditions:** Access to a sufficiently authorized signing key or signing workflow.

   **Impact:** Malicious releases pass install, upgrade, and launch checks for every trusting client.

   **Fix:** Separate an offline publisher root from scoped release keys. Use independent threshold approval for high-impact releases, short-lived delegations, and a signing workflow that checks release provenance. Hardware-backed keys reduce extraction risk but still need authorization controls. Limit each delegated key to named apps and channels. TUF separates roles to contain key compromise. [TUF FAQ](https://theupdateframework.io/docs/faq/)

9. **High: no rotation mechanism leaves no authenticated recovery from theft or loss.**

   **Attack:** Keep signing with a stolen key while the publisher asks users to accept a replacement. Impersonate that recovery request, or replay an old key list that restores a revoked key.

   **Preconditions:** Key compromise/loss, or acceptance of unversioned key metadata.

   **Impact:** Prolonged compromise, permanent update failure, or a recovery process users cannot authenticate.

   **Fix:** Enroll an independent recovery authority initially. Define versioned root metadata, authenticated revocation, and threshold-authorized succession. A stolen sole key can cross-sign an attacker's successor, so sole-key cross-signing is insufficient recovery. Persist revocations; retain retired keys only for explicitly approved historic artifacts. If the no-rotation rule stays, require independent re-enrollment and accept that continuity cannot be proved through the domain alone.

10. **High: valid signatures permit rollback and downgrade.**

    **Attack:** Replace an installed release with an older signed version containing a known vulnerability; reset local version records to make it acceptable.

    **Preconditions:** Access to an old signed bundle and its installation path or update source; missing protected version history.

    **Impact:** Reintroduction of exploitable code while every signature check succeeds.

    **Fix:** Sign monotonic release counters per publisher/app/channel. Persist the highest accepted counter and reject lower counters. Bind a counter to one artifact hash. Explicit rollback should select a specific previously approved hash and preserve the high-water mark. [TUF rollback checks](https://theupdateframework.github.io/specification/latest/)

11. **High: fresh clients can replay old signed bundles, and updates can mix releases.**

    **Attack:** Give a new or reinstalled client an obsolete signed release, or combine valid metadata from different releases to select the wrong artifact or dependency.

    **Preconditions:** No retained release history or authenticated current-release statement; independently accepted metadata.

    **Impact:** First installation starts vulnerable, or the selected release differs from the publisher's authorized set.

    **Fix:** Require fresh signed metadata selecting exact artifact hashes and lengths for each app/channel/platform. Bind related metadata into one authenticated snapshot. A fresh online install must validate that selection; an offline install needs previously obtained authenticated metadata or an explicit stale-install exception. Bundle signatures alone cannot establish the newest release.

12. **High: an attacker can make a publisher appear permanently offline.**

    **Attack:** Block the domain indefinitely, suppress security updates or revocations, or trigger an error path that skips launch verification.

    **Preconditions:** Network interference, an actual outage, or missing/incorrect cached-trust behavior.

    **Impact:** Continued exposure to old vulnerabilities; arbitrary execution if network failure makes verification optional.

    **Fix:** Verify installed bytes locally against persisted trust on every launch. Network failure may hold updates, but signature failure or missing trust must block that launch. Persist known revocations and the last successful refresh. Expiring update metadata should stop new installs/upgrades and expose staleness while previously approved artifacts keep running under point 4. A pinned local artifact policy must distinguish offline launch from fresh update authorization. Complete network suppression prevents learning new revocations; unlimited offline launch accepts that residual risk.

13. **High, potentially Critical with elevated privileges: malicious envelope, EROFS, or FUSE input exploits a parser.**

    **Attack:** Feed malformed lengths, integer overflows, corrupt compression blocks, hostile symlinks, or crafted filesystem metadata to verification, preview, or mount code. A malicious accepted publisher can sign the exploit image.

    **Preconditions:** A reachable parser bug. Previews and install inspection may expose unsigned input before consent.

    **Impact:** Parser-process code execution or denial of service; host compromise if privileges or an escape bug permit it.

    **Fix:** Bound and fuzz the unauthenticated envelope parser; authenticate raw artifact bytes before deep image parsing. Run erofsfuse and image inspection without root, inside a restricted mount namespace with resource limits and minimal filesystem access. Use `ro,nosuid,nodev`, disable unnecessary external-device features, and keep erofs-utils, FUSE helpers, and the kernel patched. Avoid `allow_other` unless required and reviewed. erofsfuse supports unprivileged mounting, but its parser and the FUSE kernel/helper path remain attack surfaces. [EROFS mount guide](https://erofs.docs.kernel.org/en/latest/mkfs.html), [FUSE security discussion](https://docs.kernel.org/filesystems/fuse/fuse.html)

14. **High: a signed, read-only application can still steal or modify host data.**

    **Attack:** A malicious publisher or compromised build ships correctly signed code that reads SSH keys, changes autostart files, accesses session services, or exfiltrates documents.

    **Preconditions:** The application runs with ordinary user access and the user installs or updates it.

    **Impact:** Account compromise and persistence. Read-only bundle storage restricts changes to the bundle, leaving host access available.

    **Fix:** Launch with a real app sandbox: isolate home access, expose selected files through portals, filter session/system bus access, and restrict network/device access by policy. Bind grants to publisher/app identity and require consent for expansions. Make unrestricted execution an explicit permission choice. Signing authenticates a release's source; authorization must constrain what it can do.

15. **Critical, conditional: launcher helpers cross a privilege boundary unsafely.**

    **Attack:** Abuse a privileged mount/install helper with crafted paths, symlinks, shell interpolation, inherited `PATH`/loader variables, or a forged caller-selected trust database. Bundle-supplied install hooks execute as root.

    **Preconditions:** Elevated helper operations consume attacker-controlled arguments, environment, state, or hooks.

    **Impact:** Local privilege escalation or system-wide trust replacement.

    **Fix:** Keep app execution, hooks, and image parsing unprivileged. Give privileged helpers a narrow typed API with caller authorization and descriptor-based file handling. Use fixed helper paths, clean environments, argv arrays, and private mountpoints. A privileged verifier must use protected system trust and version state. Inspect binfmt credential flags; `C` can derive credentials from a setuid bundle. [binfmt flags](https://docs.kernel.org/admin-guide/binfmt-misc.html)

16. **High, conditional: writable trust state defeats pinning and downgrade defenses.**

    **Attack:** Replace cached keys, erase revocations, restore an old trust database, or redirect shared caches through symlinks. A sandboxed app writes oma's state if it is exposed.

    **Preconditions:** Attacker write access to state that oma treats as authoritative.

    **Impact:** Acceptance of attacker keys or obsolete releases.

    **Fix:** Protect roots, counters, revocations, and artifact stores from the actors being defended against. Keep them outside app sandboxes and use atomic writes with secure ownership. Define backup/restore behavior so restore does not silently authorize rollback. Protection against arbitrary same-UID malware requires an additional isolation boundary; root-owned state alone also cannot defeat an attacker who controls root or restores the whole disk.

17. **Medium: the origin xattr can steer misleading consent or unsafe network requests.**

    **Attack:** Set `user.xdg.origin.url` to a trusted-looking URL, an internal HTTPS endpoint, or an endpoint that redirects to another publisher. Delete the hint to trigger a weaker path.

    **Preconditions:** Write access to the downloaded file; the hint influences trust discovery, presentation, or automatic fetching.

    **Impact:** Publisher misattribution, internal-network requests, or bypass if absence weakens verification. User xattrs follow file write permissions. [xattr rules](https://www.man7.org/linux/man-pages/man7/xattr.7.html)

    **Fix:** Keep the hint optional and untrusted. Establish publisher identity through signed metadata plus independent enrollment. Missing or conflicting hints must preserve every verification requirement. Bound discovery requests, require HTTPS, validate every redirect/destination, and prevent silent changes to the publisher origin. Avoid privileged fetching of arbitrary hint URLs; gate local-network access explicitly.

18. **High: repeated first-install prompts normalize publisher takeover.**

    **Attack:** Repeatedly change keys or manufacture a plausible recovery story until the user accepts. Reuse an old app's icon and name to imply continuity.

    **Preconditions:** User interaction and a replacement flow that resembles an ordinary upgrade.

    **Impact:** Consent authorizes an attacker publisher to replace a trusted app.

    **Fix:** Block routine updates on unexplained key changes. Present old and new identities with the concrete consequences, including data access. Default to cancel, suppress repeated prompts, and require a separate migration action. Authenticated succession should handle routine rotation without retraining users to accept arbitrary new keys.

19. **Medium: typosquatting and lookalike domains survive valid HTTPS and signatures.**

    **Attack:** Register a confusable domain, obtain its legitimate certificate, and sign a counterfeit app under an attacker key.

    **Preconditions:** A user follows the misleading domain or trusts app branding over the displayed publisher identity.

    **Impact:** Installation of attacker software through an otherwise correct first-install flow.

    **Fix:** Display the canonical full hostname prominently, with an ASCII/Punycode representation for internationalized names. Escape control characters and bidirectional formatting in publisher/app labels. Keep branding separate from authenticated identity; offer independently curated publisher identities or fingerprint verification. A certificate for a lookalike domain provides no proof of the intended publisher.

20. **Medium: interrupted updates and hostile images can exhaust or corrupt runtime state.**

    **Attack:** Interrupt installation between artifact and trust-state writes, race concurrent upgrades, serve endless downloads, or cause excessive decompression, mounts, and verification work.

    **Preconditions:** Nontransactional activation, inadequate bounds, or attacker-controlled downloads/images.

    **Impact:** Broken launches, lost version history, disk/RAM exhaustion, or blocked security updates.

    **Fix:** Stage and verify before atomic activation; journal artifact selection and counters as one recoverable transaction. Serialize per-app upgrades and retain a previously verified recovery artifact without lowering counters. Set limits for downloads, metadata, expanded data, CPU, mounts, and timeouts. Recover incomplete operations without relaxing authentication, and clean up only private owned staging objects.

21. **Low: online verification at every launch leaks app usage and adds an outage dependency.**

    **Attack:** A publisher, hosting provider, or traffic observer correlates launch-triggered requests with an app's use.

    **Preconditions:** Every launch fetches publisher keys or metadata.

    **Impact:** Usage disclosure and avoidable launch failures or delays.

    **Fix:** Perform each launch's signature verification locally. Refresh update/revocation metadata separately on a bounded schedule, cache authenticated results, and omit launch telemetry by default.

**Fixes ranked by severity, in implementation order within each level**

| Severity | Fix |
| --- | --- |
| Critical | Authenticate oma's bootstrap through an independent trust path and protect self-upgrade with a pinned release root and rollback defense (#1). |
| Critical | Authenticate before any bundle-controlled instruction executes; close supported launch bypasses (#2). |
| Critical | Define complete, unambiguous signature coverage and bind launch identity/parameters (#3). |
| Critical | Verify and mount the same immutable bytes for the entire mount lifetime (#4). |
| Critical | Protect release signing with scoped keys and independent authorization (#8). |
| Critical, if applicable | Remove elevated parsing/hooks and constrain every privileged helper (#13, #15). |
| High | Persist publisher identity across domain/TLS changes; isolate new publishers and require authenticated migration (#5, #6). |
| High | Enroll independent recovery authority and define authenticated revocation/succession (#9). |
| High | Protect trust state, release counters, and exact artifact selection against rollback and replay (#10, #11, #16). |
| High | Specify offline behavior that retains local verification, exposes staleness, and preserves known revocations (#12). |
| High | Isolate image parsers and sandbox applications with identity-bound permissions (#13, #14). |
| High | Separate unexplained publisher changes from ordinary updates and suppress consent spam (#18). |
| Medium | Bound and expose DNS-related update failures while retaining TLS and key pinning (#7). |
| Medium | Keep origin hints untrusted and constrain key-discovery requests and redirects (#17). |
| Medium | Make canonical publisher domains visible and handle confusable names safely (#19). |
| Medium | Make update activation recoverable and impose resource budgets (#20). |
| Low | Verify locally at launch and decouple background metadata refresh (#21). |
