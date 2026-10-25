---
status: superseded by ADR 0022 and ADR 0050
---

# A Bundle is a plain tar+zstd archive of a canonical directory

The canonical form of an application is a directory (`Foo.oma/`) holding the Manifest at a fixed path, the ELF and its resources. The distributed `.oma` file is an ordinary tar archive of that directory, compressed with zstd, so any tool can list and extract it without knowing `.oma`. This lets file explorers such as Flea show a Bundle's contents instead of running it, and lets omalaunch run a directory directly during development or an archive from an ephemeral directory under `$XDG_RUNTIME_DIR` without installing it. A custom container, or a mounted filesystem image, would have made every browsing tool implement our format; mounting can be added later without changing the layout.

## Consequences

The signature reserved by ADR 0007 must be stored so that a plain extractor sees the same directory with or without it. Running and installing remain omalaunch's job; browsing is a read-only view any tool can provide.
