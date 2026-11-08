---
status: accepted
---

# A Bundle file's icon is written out for the launcher

A directory Bundle's launcher entry names the icon file inside the directory (ADR 0014). A Bundle file's icon is inside the image. `odk oma dev`, `install`, `run` of an installed copy and `--register` write that entry, and they left `Icon` off, so the launcher showed a generic icon. The consent dialog still does not read an icon from the image (ADR 0060). On 2026-11-08 the owner asked for the launcher entry to carry the icon.

## Decision

- **What is written.** When the launcher entry of an installed Bundle file is written, and its manifest names an icon, that file is copied from the verified image to `apps/<root key id>/<app id>/icon.png` or `icon.svg`, mode `0644`. The pair directory is `0700` when this creates it. `Icon` is that absolute path. The other of the two names is removed, so a change from PNG to SVG does not leave the old file. A manifest with no icon removes both, and the entry has no `Icon` line.
- **When.** The same moments the entry is written: `odk oma dev`, `odk oma install`, `odk oma run` of an installed copy, and `odk oma --register`. Run once writes neither the entry nor the icon. The bytes are copied only after the image has been checked, with `odk-oma`'s own reader, and only when they differ.
- **The dialog.** The consent dialog still does not show the image's name or icon (ADR 0060, section 7).
- **Purge.** `odk oma purge` removes the icon with the pair's directory.

## Considered options

- **Point `Icon` into a mount of the image.** The image is not mounted while the launcher reads the entry, and a mount would parse it in the kernel for a desktop scan.
- **Leave the icon out of file bundles.** That is what the code did. `odk oma dev` installs a Bundle file (ADR 0085), so a project that names an icon still had none in the launcher.

## Consequences

- ADR 0084's rule that one file of a Bundle's image is written to the host gains this second file, the icon. The daemon's program is unchanged.
- ADR 0067's rule that nothing of an image reaches a host filesystem has these two exceptions.
