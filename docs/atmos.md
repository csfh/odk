# What ODK gives Atmos's Rust window

Not normative. Atmos (`../atmos`) is moving its QML window to ODK page by page, built from `odk::prelude` alone (its `frontend/tests/purity.rs`). On 2026-10-28 it listed what ODK still lacked: charts, a data table, resizable panels, a file picker, and a window's title and smallest size; and ten pages that rely on Quickshell modules or Qt's file dialogs. This page says what ODK now has for each, and what is Atmos's own to build, so the two sides do not wait on each other. The decisions are the ADRs named; the APIs are in `docs/spec/odk-ui-components.md`.

## What ODK has now

| Atmos needed | ODK | Decision |
|---|---|---|
| A window title and smallest size (the QML window is 960×680, at least 800×560) | `WindowConfig::new("dev.csfh.atmos").title("Atmos").size(960.0, 680.0).min_size(800.0, 560.0)`, given to `odk::ui::open_window` in place of `WindowOptions` | spec, `docs/checks.md` "Window sizes" |
| A file or folder dialog | `FileChooser::open(title)`, `save(title)`, `folder(title)` with `filter`, `name`, `accept_label`, `multiple`, `start_in(StartFolder::…)`, then `.choose(cx, \|chosen, cx\| …)` | ADR 0068 |
| A list beside its detail | `Resizable::horizontal(id, name)` of `ResizablePanel`s, sizes as fractions, `on_layout` to keep them | ADR 0069 |
| Process, service and package lists | `DataTable::new(id, name, rows)` with `Column`s, `row_key`, `sort_by`, `row_menu`, `indent` for the process tree, `Column::action` for End or Restart | ADR 0070 |
| Monitor's charts and the small data widgets | `Chart::new(id, name).paint(\|scene, t\| …)`: paths, arcs, curves, rectangles and haloed text in named `Ink`s, `transition(sample)` for the move between samples, `on_press`, `on_pan`, `on_zoom` for the treemap | ADR 0071 |
| The avatar and the wallpaper preview | `Image::avatar(id, name).source(bytes)` and `Image::new(id, name).fit(ImageFit::Contain).source(bytes)`, from bytes, never a path | ADR 0072 |

Three things to know when using them:

- **The file chooser needs the new `odkd`.** It is `odkd`, outside the sandbox, that opens the dialog (ADR 0068); an install older than it answers `FileChooserError::NoHost`. Install ODK's `main` as `CLAUDE.md` says ("Branches, pull requests and releases"). A chosen path is a name, not access: hand it to `atmosd`, as the QML pages do, never read it in the window.
- **Named folders replace `Quickshell.env("HOME")`.** Inside the sandbox `$HOME` is the window's data directory. `StartFolder::Home`, `Pictures`, `Documents` and the rest are looked up by `odkd` outside it; a path `atmosd` read (an export folder, the last report's folder) is `StartFolder::Path`.
- **Sizes are plain numbers.** `WindowConfig::size` and `min_size` take them, as `ResizablePanel` takes fractions and `Column::chars` a count of characters; `purity.rs`'s rule against a literal as the first argument of a sized method may want them as named constants.

## The ten pages

"Unblocked" means ODK has every piece the page's Quickshell or Qt part needs; the rest of the page is Atmos's port as usual.

| Page | What relied on Quickshell or Qt | Status |
|---|---|---|
| Accounts | The avatar `FileDialog` (PNG, JPEG) → `avatar.set`; the avatar preview (`Image` with a `file://` URL) | The picker is unblocked (`FileChooser::open("Choose a face").filter("Images", ["*.png", "*.jpg", "*.jpeg"])`). The preview takes `host.image` with `which: "avatar"` (csfh/atmos#102), whose base64 bytes go to `Image::avatar(…).source(…)`. |
| Hooks | The "Install a hook" `FileDialog` → `hook.install` | Unblocked. |
| Bar | The agent usage sync `FolderDialog` → the `agentsSyncDir` setting | Unblocked (`FileChooser::folder`). |
| Network | The WireGuard import `FileDialog` (`*.conf`, `*.nmconnection`) → `wifi.wireguard-import`; share a file → `share.file` or `tailscale.send`; share a folder → `share.folder` | Unblocked. |
| Wi-Fi | `Quickshell.Networking`: the live list of networks in range (name, signal, security, connected, known) and the scanner switch | `atmosd`'s `wifi.scan` (csfh/atmos#102) rescans and answers the networks in range, with signal, security, whether one is in use and whether it is saved; the page asks for it while it is shown (a Scan button, or on opening), in place of Quickshell's scanner. ODK's `DataTable` draws the list. |
| Bluetooth | `Quickshell.Bluetooth`: discovery on and off, the adapter's discoverable switch, devices found but not paired, a device's battery | `atmosd`'s `bluetooth.scan` (csfh/atmos#102) runs a discovery of a few seconds, which stops on its own, and answers the adapter and every device found, paired or not, with a connected device's battery; the write `bluetooth.discoverable` sets the adapter's switch, read back as `bluetoothDiscoverable`. Pairing, trusting, connecting and forgetting were there already. |
| Export | The save and open `FileDialog`s (`*.md`) with the export folder as the start, and `Quickshell.env("HOME")` | Unblocked (`FileChooser::save(…).name(…).start_in(StartFolder::Path(export_dir))`, `StartFolder::Home`); `host.read` and `host.write` exist. |
| Boot | The two PNG `FileDialog`s → `plymouth.set` and `plymouth.preview` | Unblocked. |
| Background | The image `FileDialog` → the `background` setting; the wallpaper preview | The picker is unblocked, and the preview takes `host.image` with `which: "background"` (csfh/atmos#102) into `Image::new(…).fit(ImageFit::Contain)`. |
| Diagnostics | The save `FileDialog` (`*.txt`), starting in the last report's folder, then Pictures, then `HOME` | Unblocked (`FileChooser::save("Save diagnostic report").name("omarchy-debug.txt").filter("Text", ["*.txt"]).start_in(…)`). |

So six pages are unblocked by ODK alone: Hooks, Bar, Network, Export, Boot and Diagnostics. The other four have their backend paths in `atmosd` (csfh/atmos#102, on top of `atmos-v1.0.0-rc`): `host.image` for the Accounts and Background previews, `wifi.scan` for Wi-Fi, and `bluetooth.scan` with the write `bluetooth.discoverable` for Bluetooth. They run `nmcli` and `bluetoothctl`, as `atmosd` already does, so no new foreign call (Atmos's ADR 0007).
