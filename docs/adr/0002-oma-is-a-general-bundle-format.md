# `.oma` is a general bundle format, not ODK-only

A `.oma` Bundle may wrap any native program that omalaunch knows how to run, the same way a macOS `.app` can wrap any executable. If omalaunch does not know how to run the Bundle's program, it fails rather than guessing. ODK is the first-class way to build a Bundle but the format does not require it.

## v1 scope

v1 supports exactly one program kind: a bundled native x86_64 ELF executable. omalaunch does not check that the ELF came from ODK. Electron and Tauri apps are expected to work only as ELFs that bundle everything they need beyond the host baseline (ADR 0012), where the sandbox allows it. First-class support for them (shared runtimes, host-dependency declarations) needs its own ADR.

## Consequences

The Bundle format and omalaunch's set of supported program kinds are versioned independently of ODK, and omalaunch and its sandbox cannot assume anything about how a Bundle's program was built, including that it uses ODK.
