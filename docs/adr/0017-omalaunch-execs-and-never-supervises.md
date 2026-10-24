---
status: superseded by ADR 0024
---

# omalaunch sets up the sandbox and `exec`s the ELF; it never supervises

omalaunch builds the sandbox and then `exec`s the application's ELF, so omalaunch is gone and the application is a normal process with no supervisor. This works because the sandbox has no PID namespace (ADR 0005). It applies to installed applications and to "Run once" alike, because a Bundle runs in place and leaves nothing to clean up (ADR 0022).
