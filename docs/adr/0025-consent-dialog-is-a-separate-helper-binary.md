---
status: amended by ADR 0033
---

# The consent dialog is a separate helper binary, and oma does not link GPUI

The dialog of ADR 0016 is shown by a small separate program, `omagate`, not by oma itself. oma stays a short-lived command-line program with no GPUI in it. This matters because the init is oma re-executed (ADR 0024) and one runs beside every application; linking GPUI into the binary would put the whole UI toolkit into each of them. We considered making oma a resident daemon that owns the window, and rejected it for v1: ADR 0009 and ADR 0015 chose path units so that nothing has to stay running, a resident GPUI process costs memory all day for a dialog that is rarely shown, and every launch would depend on a daemon being up and on a client protocol. Nothing here prevents adding a socket-activated daemon later.

The helper is given the request and returns the answer, and decides nothing:

- oma passes the Bundle's name and the permission lines already worded (the wording lives in the `.oma` format library, ADR 0013), together with which buttons apply (`consent-dialog.md`).
- The helper shows the window and prints the user's answer: Install, Run once, Allow, Keep previous version or Cancel. Closing the window is Cancel.
- oma acts on the answer and writes the consent record (ADR 0023). The helper never reads or writes oma's state and never sees the Bundle.
- oma finds the helper next to its own executable. If the helper is missing, cannot start or fails, oma treats it as Cancel and fails closed, as in the non-interactive mode of ADR 0016.

The helper uses the pinned GPUI directly, for the reason in ADR 0016: the OMA crates must not depend on ODK (ADR 0013). It runs outside the sandbox, like oma, but holds no authority: it can only report an answer, and oma still checks that the answer is one it asked for.

## Consequences

ADR 0016 stands except for where the dialog lives; it is marked amended. The exact request and answer format is to be written in `docs/spec/consent-dialog.md` before the helper is built. The helper adds a second binary to install beside oma, and no ADR yet says where either is installed.
