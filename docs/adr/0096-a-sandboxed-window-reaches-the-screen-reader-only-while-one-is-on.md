---
status: accepted
---

# A sandboxed window reaches the screen reader only while one is on

ADR 0089 put a screen-reader grant in the Rune port: a sandboxed window is readable by Orca while the session bus stays refused (ADR 0012, ADR 0068). Reading the pinned `accesskit_unix` 0.24.1 on 2026-11-11 shows what that takes. The library connects to the **session** bus first, watches `org.a11y.Status.IsEnabled` there, and only when it is true asks for the AT-SPI bus address (`AT_SPI_BUS_ADDRESS` if set, else `org.a11y.Bus.GetAddress` on the session bus) and connects. If the session bus cannot be reached it gives up silently, with no retry, and never reads `AT_SPI_BUS_ADDRESS`; no Rune option or environment variable turns the adapter off or changes this (`docs/checks.md`). So the environment variable cannot stand in for the session bus, and the sandbox, which sets neither variable and has no `$XDG_RUNTIME_DIR/bus`, leaves the window without a tree. The AT-SPI bus is not filtered: a client on it can read what other applications show. The owner chose, on 2026-11-11, to grant it only while a screen reader is on.

## Decision

**A sandboxed window gets a stand-in session bus and the AT-SPI bus only while the user's accessibility is enabled.**

- **`odkd` runs the stand-in.** A private `dbus-daemon` (a stock Omarchy has it) in `$XDG_RUNTIME_DIR/odk/`, a directory oma already binds read-only (ADR 0062), configured to let a peer own and read nothing but `org.a11y.Status`. `odkd`, which has the real session bus, mirrors that service's `IsEnabled` onto it and updates it when the real one changes. No other service is reachable on it, so it is a session bus in name only.
- **oma binds the AT-SPI bus and says where both are.** While `odkd` reports accessibility enabled at launch, oma binds the AT-SPI bus socket and the stand-in's socket into the sandbox, sets `DBUS_SESSION_BUS_ADDRESS` to the stand-in and `AT_SPI_BUS_ADDRESS` to the AT-SPI socket, and gives each a Landlock rule. With accessibility off, nothing is bound and the sandbox is what it is today. The grant is not a permission an application asks for (ADR 0023): it follows the user's own setting, like `odkd`'s directory.
- **`docs/spec/sandbox-baseline.md` lists both mounts and both variables as conditional**, and `docs/spec/consent-dialog.md` is unchanged: a sandboxed application never asks for it.

ADR 0089's sentence that the session bus stays refused still holds: the stand-in is not the user's session bus and carries one property.

## Consequences

- While a screen reader is on, a sandboxed application can read other applications' accessible trees. The user turned accessibility on, so the exposure is theirs to weigh, and an application launched before it was turned on does not have it until it is launched again. A filtering proxy would close this, and `xdg-dbus-proxy` is not in a stock Omarchy (ADR 0068).
- Rune connects once, when a window is created, and `accesskit_unix` shows no initial value of `IsEnabled`, only changes; `odkd` therefore sets the stand-in's property after the application has connected so that the library sees a change. This is unverified and is the first thing the check settles.
- The AT-SPI bus may be an abstract socket, which a sandbox with its own network namespace cannot reach (an inference from the launcher's strings; `GetAddress` has not been run on a real session). If it is, the grant needs the bus's path form, and the check says so.
- The check is a headless test on a private bus with a fake `org.a11y.atspi.Registry` (`crates/odkd/tests/chooser.rs` is the pattern), one `tools/headless-shot` run of a story with the variables set, and an entry in `docs/checks.md` that says what was seen and what was not.
