---
status: accepted
---

# odk-ui and oma-consent are rebuilt on Rune

The owner decided this on 2026-11-07, after the Rune spike (`spikes/rune-ui`, recorded in `docs/checks.md`). `odk-ui` is rebuilt on Rune (`github.com/anomalyco/rune`) instead of GPUI, and `oma-consent` moves in the same port, so GPUI leaves the workspace when that port is done. The spike drew a `Button`, a `Select` and an `Input` with Rune's own elements and none of `rune-kit`. It left ADR 0004, ADR 0027 and ADR 0029 unchanged because nothing was decided then. The components stay ours.

## Decision

- **Rune is the toolkit under `odk-ui` and under `oma-consent`.** It is pinned to one git revision in the workspace. A newer revision is a deliberate bump, reviewed the way ADR 0066 reviewed `ureq`. The spike checked `dev` at `7ead7d6`. Default features stay off and `accessibility` stays on. `rune-kit` is not a dependency. Rune is 0.0.1 and published as git only, so the pin is a revision rather than the crates.io pin ADR 0029 gave GPUI.
- **The components stay ours.** An application still writes `Button::new` and the rest, and the token layers stay the source of colour, geometry and type. ADR 0027's refusal of a third-party component library covers `rune-kit` as it covered `gpui-component`.
- **A component's own state stays inside `odk-ui`.** An open menu, a held key and a caret belong to the component. Rune's view is one function of one state value, and `odk-ui` is what holds that state and hands Rune the view. The application does not gain a field for every open menu.
- **Activation stays the interaction spec.** A control activates on the release of Enter or Space (`docs/spec/odk-interaction.md`). Rune activates a focused control on Space's press and does not activate a button on Enter. The component handles those keys.
- **Primary selection stays.** Making a non-empty selection writes it, and a middle-click pastes it (`docs/spec/odk-text-input.md`). A masked field still writes neither the clipboard nor the primary selection. `Input`, `Textarea`, `InputGroup`, `SearchInput`, `PasswordInput`, `NumberInput` and `Command`'s field are done on Rune only once that works. The other components may land before those fields.
- **`oma-consent` links Rune directly.** It still does not depend on ODK (ADR 0013). It is still a separate helper that only reports an answer, and `oma` and its init still link no UI toolkit (ADR 0025). `odk::gpui`, the escape hatch of ADR 0004, leaves with GPUI.
- **A sandboxed window is readable by a screen reader before this port is done.** `Describe` is still what a component reports (ADR 0036), and `odk-ui` hands those descriptions to Rune's tree. The session bus stays refused (ADR 0012, ADR 0068, `docs/spec/sandbox-baseline.md`). `accesskit_unix` 0.24.1 opens that bus before it reads `AT_SPI_BUS_ADDRESS`, so the port's grant is only what the screen reader needs. Which grant that is, the port settles and writes into `docs/checks.md`.
- **`Stack`, `Row` and `Grid` keep their names and jobs** (ADR 0088). They cease to be newtypes over GPUI's `Div`.
- **This record moves no code.** The tree still builds on GPUI 0.2.2. `spikes/rune-ui` stays a spike. The order of the port is in `docs/plan.md` under "Rune".

## Considered options

- **One Rune state value owned by the application.** The spike's `Select` already reached its slice of that state through an accessor. The owner kept the state inside `odk-ui`, so the component API stays and an application does not hold every open menu.
- **Drop primary selection until Rune grows it.** Rune's source has none, and a middle press reaches canvas handlers only. The owner kept the text-input spec, so the fields wait.
- **Leave `oma-consent` on GPUI** until the new components can draw the dialog. The owner included the helper in this port, so GPUI can leave in the same change.
- **Leave the screen reader for a later sandbox decision.** The spike's headless runtime returned an accessibility tree, and a sandboxed window would give Orca nothing while the session bus is refused. The owner included that grant in this port.
- **Give the application the session bus so AT-SPI can connect.** ADR 0068 already refused that shape for the file chooser: the bus reaches every service, and `odkd` asks the portal on the application's behalf. The screen-reader grant has to be at least that narrow.

## Consequences

- ADR 0003, ADR 0004, ADR 0016, ADR 0025, ADR 0027, ADR 0029, ADR 0036, ADR 0038 and ADR 0088 are amended in the parts named above. The rest of each stands. Luau stays the plugin layer. The consent helper gains no authority. ADR 0029 and `patches/gpui` govern GPUI for as long as a crate still links it.
- GPUI leaves in the change that finishes the port: `odk-ui` and `oma-consent` both on Rune, the text fields meeting the primary-selection rule, and a sandboxed window readable by the screen reader. Until that change the code, the checklist and the specs that name GPUI 0.2.2 stay as they are.
- The checklist moves onto Rune's headless runtime with the components. A known gap stays a gap.
- A font `odkd` names is bytes given to Rune before the window opens. Rune finds system fonts only for fontconfig's `monospace` slot, asks once per process, and has no public API to load a family while running (`docs/checks.md`). A font changed with `omarchy-font-set` while an application runs is picked up at the next start, until Rune can load one at run time.
- `odk-theme` and `odk-a11y` stay free of the toolkit. `oma` stays free of it.
- Drag, virtualised rows and a window on Hyprland were outside the spike. `DataTable`, `Resizable` and `Chart` are part of the port. The spike panicked when its Wayland socket broke. That exit is part of the port's work.
