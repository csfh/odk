# Roadmap to v1

Not normative, and it decides nothing: `docs/plan.md` stays the order of the build, the ADRs and specs stay the source of truth, and a scope choice below is settled by an ADR, by the owner. Written on 2026-11-11 from `docs/plan.md`, `docs/checks.md`, the checklist's "Known gaps" and `docs/release-readiness.md` (whose rows from before 2026-10-31 are partly stale; this file wins where they differ, and `CLAUDE.md` wins over both).

## What v1 is (proposed, not yet accepted)

v1 is a release the owner can call production-ready, which `docs/release-readiness.md` could not say while one High item was open. That is when all four hold:

1. **A screen reader can read an ODK window.** The High item there is that GPUI 0.2.2 has no accessibility tree. ADR 0089 is the fix, so Track A ends with GPUI gone.
2. **The OMA chain works end to end on a stock Omarchy machine:** a release signed by the owner's root key, `install.sh`, install, run, upgrade, rollback and purge, each run for real and written into `docs/checks.md` (ADR 0063, ADR 0086, ADR 0087).
3. **`docs/checks.md` has no unverified claim.** Each is verified, or dropped with a line saying why.
4. **Plugins work.** `odk::plugin` loads a Luau Plugin from its manifest under a sandbox, with the four capabilities of `docs/spec/plugin-manifest.md` (the owner, 2026-11-11: Plugins are in v1).
5. **No known gap is left:** no row in the checklist's "Known gaps", no component without a story, no provisional value in a spec's "Open questions" that the owner has not settled or knowingly kept.

## Where it stands (2026-11-11)

| Area | State |
|---|---|
| OMA steps 0 to 5, trust phases, `odkd`, packaging text | Built. `flow.rs` still carries "not built yet (issue #14)" markers to re-check. |
| Design-language migration (steps 1 to 10, Phase 12) | Done. Provisional values remain (Track C). |
| Rune port, step 1 (host) and step 2 (Button, Input, Select) | Done. |
| Rune port, step 3 | In progress. In `main`: the leaf components, forms, the roving-focus groups, NavList, Tabs, Progress, Spinner, `FormField`, `Image`, `Resizable` and `Chart`, `Dialog`, `Sheet`, `Toast` and `Menu` with `DropdownMenu` (2026-11-11). Popover, HoverCard and Tooltip are in `main` too. `ContextMenu`, Breadcrumb and `Command` are built. `SearchInput`, `PasswordInput`, `NumberInput`, `InputGroup` and `Textarea` are built (`NumberInput` has no commit on blur). `DataTable` is built. Step 3's components are all built; the supporting surface (window setup, `FileChooser`, focus helpers) is not. |
| Rune port, step 4 (`oma-consent`) | Done, 2026-11-11, headless; a real click and the `hyprctl` float were not checked. |
| Rune port, steps 5 and 6 (screen-reader grant, GPUI leaves) | Not started. |
| `odk-plugin` (Luau host and Plugin loader) | First slice built. In v1 (the owner, 2026-11-11). ADR 0091 (vets `mlua`) and ADR 0092 (one state each, the API's shape, limits, errors) were accepted by the owner on 2026-11-11. `odk-plugin`'s first slice is built (`ui` and `network` are not). |
| Motion (ADR 0059) | Accepted, not built. |

## Track A: finish the Rune port (the critical path)

Work left to take GPUI out (a scout's list on 2026-11-11; ADR 0094 is the decision: one breaking switch, no second toolkit kept): (1) the facade's surface, decided by ADR 0094: `odk::ui` becomes `odk-view`'s and handlers take `&mut T` and not `Window` and `App`; (2) the theme-following host (`odk_view::run`, built 2026-11-11, colours and reduced motion only); (3) the Omarchy font, done on 2026-11-11 (ADR 0095: `Face::installed`, and the user's font directory shown to the sandbox); (4) `preview_theme`, `set_colour_options`, `configure`; (5) several windows (`open_window`, `Shell`, `WindowConfig`; Rune has `Windows`); (6) helpers `odk-ui` has and `odk-view` lacks: truncation, the decoration family, `Link`, `Grid` and `ContentColumn`, `StyledExt`, `scroll_y`, `sidebar_frame`; (7) `focused_component` and Tab actions, which wait on focus by key; (8) `FileChooser` over the client `odk-theme` has, done on 2026-11-11; (9) the checklist's 61 GPUI files, ported as `odk-view` tests; (10) a Rune benchmark for items 35 and 36; (11) delete the six GPUI stories; (12) remove `crates/odk-ui`, `patches/`, the `gpui` pin and `font-kit`, run `tools/check-deps` and `cargo machete`, keep `image`'s bmp, gif and webp; (13) docs and ADR statuses; (14) check that `cargo tree -i gpui` is empty. Plan step 5, the screen-reader grant, is decided by ADR 0096 and built on 2026-11-11 (`odkd`'s stand-in session bus, oma's conditional mount and variables; `docs/checks.md`), not yet seen against a real screen reader. The risks left are (5), (7), and that last check (an abstract-socket AT-SPI bus, `odkd`'s mirror on a real session).


1. **Overlay layer on Rune.** `Popover`, `HoverCard`, `Tooltip` and `Placement` (in flight on `feat/rune-overlay`), then `Dialog`, `Sheet` and `Toast`, with the host's root layer list.
2. **Menus.** `DropdownMenu`, `ContextMenu` and `Breadcrumb`, on the overlay layer and the shared menu list.
3. **Primary selection, then the text fields.** `Textarea`, `InputGroup`, `SearchInput`, `PasswordInput`, `NumberInput` and `Command`'s field wait on it (ADR 0089).
4. **The rest of step 3.** Done: `DataTable`, `Chart` and `Command` were the last. (`FormField`, `Image` and `Resizable` are done.)
5. **Rune-port gaps.** ToggleGroup's single mode is radios, a NavList's current page has a state, manual-activation Tabs are tabs (`docs/checks.md`); Arabic labels in `Checkbox`, `Switch` and `ToggleGroup` (checklist item 23); a `Checkbox`'s label wraps at about 150 px in a stretched column, in `rune-checkbox` too (seen 2026-11-11).
6. **`oma-consent` on Rune** (plan step 4): done. The icon and the upgrade screen it lacks (`docs/checks.md`) remain.
7. **The screen-reader grant** (plan step 5): how a sandboxed window reaches the screen reader while the session bus stays refused, written into `docs/checks.md`.
8. **GPUI leaves** (plan step 6): `odk::gpui`, `patches/gpui` and the GPUI stories go, the `odk` facade turns to `odk-view`, and `cargo machete` and `tools/check-deps` are clean.

**What Rune allows the remaining components (checked 2026-11-11 in the pinned revision):**

- **`Resizable`** maps onto `El::on_drag`, whose `Drag` carries the element-local position, the travel since the press and `cancelled()` (for Escape), and `El::cursor(Cursor::ResizeColumn | ResizeRow)`. Nothing else is missing, so it can be ported as designed (ADR 0069).
- **`Chart`** was settled by the owner on 2026-11-11 (ADR 0093): the box is measured from the last layout, and arcs and text are built from Rune shapes and text elements. Built; see `docs/checks.md`.
- **`DataTable`** (ADR 0070, virtualised rows) was not checked yet.

Each component brings its `stories/rune/<name>.rs` in the same pull request (the owner, 2026-11-08), which clears the 36 "no story" rows in the checklist as it goes.

**Exit:** the workspace builds with no GPUI; a headless test reads an accessibility tree; the checklist has no Known gaps.

## Track B: OMA to a real release (parallel with A)

1. **The unbuilt trust-plan parts.** Re-checked on 2026-11-11 against the code: the pinned-root upgrade by `install`, opening a Bundle that is not installed, the inbox watches, `.desktop` entries and the daemon are built, and their stale "not built" wording is fixed. Still open: the revocation document has no schema (`docs/oma-trust-plan.md`, question 2); opening a newer version is refused on purpose and `install` upgrades; and the trust plan and `CLAUDE.md` disagree about whether `bundle` calls the image writer (the code at `crates/oma/src/bundle.rs:172` does).
2. **The owner's root key compiled in** (ADR 0086), then a real release signed with `odk oma sign-release`.
3. **A first real release** on `odk.csfh.dev`, then `install.sh` and `odk oma self-upgrade` run against it on a second machine, which clears the install rows of `docs/checks.md`.
4. **Registration.** The `binfmt_misc` line and the desktop handler are text in `packaging/`, and ADR 0087 forbids root. Decide, in an ADR, how v1 registers them or that it does not.
5. **The two open sandbox rows:** Electron's own sandbox inside ours (plan step 6, with any seccomp change it forces) and playback-only PipeWire.
6. **The sandbox's unverified rows:** `/etc`, ptys, Landlock ABI 6, unprivileged user namespaces, `odkd` under a real Hyprland, mount and unpack timings.

**Exit:** a signed release installed on a clean machine, upgraded, rolled back and purged, each step in `docs/checks.md`.

## Track C: verification and hardening (after A's components land)

1. The hand checks listed in `docs/checks.md` and `docs/release-readiness.md`: reduced motion under a real Hyprland, hover, press and focus on the form controls, IME, real-pointer use of `DataTable`, `Resizable`, `Chart` and `Image`.
2. The provisional values in `docs/plan.md`'s "Open questions": `Size` as a density step, the keyboard-only focus ring, `motion.cycle`, the status dot and progress bar sizes, focus offset, thumb gap. Settle each, or keep it knowingly.
3. Visual regression with `tools/headless-shot` baselines, since it is by eye now.
4. The figures "not measured": memory, overlay opening and theme-watch latency; `cargo bench` and `tools/idle-cpu` against the story budgets on the Rune stories.
5. `cargo audit` was run by hand on 2026-11-11 (0 vulnerabilities; the one yanked crate is fixed; the warnings are in `docs/checks.md`). `cargo deny` and `cargo udeps` are not installed here and have never been run.
6. Rename `Alert::variant` and `Toast::variant` (they take a `Tone`) while a breaking change is still free.

## Track E: Plugins (in v1, the owner, 2026-11-11; parallel with A)

The design is ADR 0003 (Luau through `mlua`, `Lua::sandbox()`, in the application's process), ADR 0020 (a Plugin declares capabilities and the host grants them), ADR 0028 (`odk::plugin` behind a feature) and `docs/spec/plugin-manifest.md` (version 1; four capabilities: `app.state`, `ui`, `network`, `storage`). Nothing is built.

1. **Vet `mlua` as `ureq` was (ADR 0066)**: written as ADR 0091 (`proposed`). Adding the crate, with `tools/check-deps` and `crates/deps/Cargo.toml`, waits for its acceptance.
2. **Settle what the spec leaves open**: written as ADR 0092 (`proposed`), with `docs/spec/plugin-api.md` to come with the first implementation. The open items are: the API functions of each capability, isolation between Plugins (a Lua state each, or removing `getfenv` and `setfenv`), memory and time limits (`docs/checks.md` has `mlua`'s findings: limits and interrupts work, an interrupt cannot stop a long C call, `require` is left in place, globals are shared across chunks), and error reporting.
3. **`odk-plugin`:** the manifest reader (unknown keys are an error; an invalid manifest means not loaded), the loader from `~/.local/share/oma/data/<identifier>/plugins/`, the capability grant (a refused capability means the Plugin is not loaded, and the host is told which), the sandboxed state, `require` replaced, and `odk::plugin` behind a feature of the `odk` facade. **First slice built on 2026-11-11** (`docs/spec/plugin-api.md`, `docs/checks.md`): the manifest reader, the loader, a state each, the capability tables, `require`, `app.get`, `storage`, the limits and the error values. Left: the `ui` and `network` functions, subscribing to `app.state`, and an application that uses the `plugin` feature.
4. **The four capabilities' APIs** on the Rune UI where `ui` touches it. Plugins get no database (ADR 0049) and drive no motion (`docs/spec/odk-motion.md`).
5. **Tests and a story** for each: hostile Plugins (a loop that never ends, memory growth, a global that leaks, a path that escapes), headless like the rest. Run a Plugin under `odk oma run` for real only with the owner's agreement.

**Exit:** a Plugin loads from a manifest, gets exactly the capabilities it declared and the host granted, cannot leave its sandbox in the hostile tests, and is unloaded at exit.

## Track D: scope choices for the owner (each an ADR)

- **Plugins.** Settled: in v1 (Track E).
- **Motion (ADR 0059).** Build the declarative driver before v1, or after.
- **Deferred components:** `Combobox`, `Pagination`, submenus, non-modal dialogs, a dialog's default action on Enter.
- **`odk::db`.** Built and off by default, with no consumer in the workspace. Keep it in v1 with one, or mark it experimental.

## Housekeeping

- `docs/release-readiness.md` and `docs/plan.md` carry stale lines (CI, the Todo example, the gallery, "design phase"). A dated note at the top of each says which; the rows are left as measured on their dates.
- The "v1" criteria above go into an ADR when the owner accepts them.

## Order

1. This file and the other Track D choices. 2. Track A steps 1 to 5, beside Track B steps 1 and 2 and Track E steps 1 and 2. 3. Track A steps 6 to 8, beside Track B steps 3 to 6 and Track E steps 3 to 5. 4. Track C, then a release candidate (`packaging/README.md`: the version commit on `main`, then `git tag -s`). 5. The last hand checks, then v1.

## What an agent cannot do

A window on the owner's desktop, the owner's keys (root key, GPG, YubiKey), a merge, a push and a tag are the owner's (`CLAUDE.md`). Work that needs them is queued here and asked one question at a time.
