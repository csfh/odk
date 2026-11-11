# Roadmap to v1

Not normative, and it decides nothing: `docs/plan.md` stays the order of the build, the ADRs and specs stay the source of truth, and a scope choice below is settled by an ADR, by the owner. Written on 2026-11-11 from `docs/plan.md`, `docs/checks.md`, the checklist's "Known gaps" and `docs/release-readiness.md` (whose rows from before 2026-10-31 are partly stale; this file wins where they differ, and `CLAUDE.md` wins over both).

## What v1 is (proposed, not yet accepted)

v1 is a release the owner can call production-ready, which `docs/release-readiness.md` could not say while one High item was open. That is when all four hold:

1. **A screen reader can read an ODK window.** The High item there is that GPUI 0.2.2 has no accessibility tree. ADR 0089 is the fix, so Track A ends with GPUI gone.
2. **The OMA chain works end to end on a stock Omarchy machine:** a release signed by the owner's root key, `install.sh`, install, run, upgrade, rollback and purge, each run for real and written into `docs/checks.md` (ADR 0063, ADR 0086, ADR 0087).
3. **`docs/checks.md` has no unverified claim.** Each is verified, or dropped with a line saying why.
4. **No known gap is left:** no row in the checklist's "Known gaps", no component without a story, no provisional value in a spec's "Open questions" that the owner has not settled or knowingly kept.

## Where it stands (2026-11-11)

| Area | State |
|---|---|
| OMA steps 0 to 5, trust phases, `odkd`, packaging text | Built. `flow.rs` still carries "not built yet (issue #14)" markers to re-check. |
| Design-language migration (steps 1 to 10, Phase 12) | Done. Provisional values remain (Track C). |
| Rune port, step 1 (host) and step 2 (Button, Input, Select) | Done. |
| Rune port, step 3 | In progress: leaf components, forms, the roving-focus groups, NavList, Tabs, Progress and Spinner are in; Popover, HoverCard and Tooltip are in a worktree. Menus, Breadcrumb, Dialog, Sheet, Toast, Command, FormField, the other text fields, DataTable, Resizable, Chart and Image are not. |
| Rune port, steps 4 to 6 (`oma-consent`, screen-reader grant, GPUI leaves) | Not started. |
| `odk-plugin` (Luau host and Plugin loader) | Not started; "later" in `docs/plan.md`. |
| Motion (ADR 0059) | Accepted, not built. |

## Track A: finish the Rune port (the critical path)

1. **Overlay layer on Rune.** `Popover`, `HoverCard`, `Tooltip` and `Placement` (in flight on `feat/rune-overlay`), then `Dialog`, `Sheet` and `Toast`, with the host's root layer list.
2. **Menus.** `DropdownMenu`, `ContextMenu` and `Breadcrumb`, on the overlay layer and the shared menu list.
3. **Primary selection, then the text fields.** `Textarea`, `InputGroup`, `SearchInput`, `PasswordInput`, `NumberInput` and `Command`'s field wait on it (ADR 0089).
4. **The rest of step 3.** `FormField`, `Command`'s panel, `Image`, `DataTable`, `Resizable` and `Chart`.
5. **Rune-port gaps.** ToggleGroup's single mode is radios, a NavList's current page has a state, manual-activation Tabs are tabs (`docs/checks.md`); Arabic labels in `Checkbox`, `Switch` and `ToggleGroup` (checklist item 23).
6. **`oma-consent` on Rune** (plan step 4), with the icon and the upgrade screen it lacks (`docs/checks.md`).
7. **The screen-reader grant** (plan step 5): how a sandboxed window reaches the screen reader while the session bus stays refused, written into `docs/checks.md`.
8. **GPUI leaves** (plan step 6): `odk::gpui`, `patches/gpui` and the GPUI stories go, the `odk` facade turns to `odk-view`, and `cargo machete` and `tools/check-deps` are clean.

Each component brings its `stories/rune/<name>.rs` in the same pull request (the owner, 2026-11-08), which clears the 36 "no story" rows in the checklist as it goes.

**Exit:** the workspace builds with no GPUI; a headless test reads an accessibility tree; the checklist has no Known gaps.

## Track B: OMA to a real release (parallel with A)

1. **The unbuilt trust-plan parts:** the pinned-root upgrade, opening a Bundle that is not installed, running a newer version, the inbox watches, the pinned-host lookup, lockfile use (`docs/oma-trust-plan.md`, `crates/oma/src/flow.rs`), and `mkfs.erofs`'s label in ADR 0065. Re-check each marker first; some are stale.
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
5. `cargo deny` and `cargo udeps`, never run.
6. Rename `Alert::variant` and `Toast::variant` (they take a `Tone`) while a breaking change is still free.

## Track D: scope choices for the owner (each an ADR)

- **Plugins.** The project says an embedded Luau machine for Plugins, and `odk-plugin` is not started. In v1 or after? This sizes everything else.
- **Motion (ADR 0059).** Build the declarative driver before v1, or after.
- **Deferred components:** `Combobox`, `Pagination`, submenus, non-modal dialogs, a dialog's default action on Enter.
- **`odk::db`.** Built and off by default, with no consumer in the workspace. Keep it in v1 with one, or mark it experimental.

## Housekeeping

- `docs/release-readiness.md` and `docs/plan.md` carry stale lines (CI, the Todo example, the gallery, "design phase"). A dated note at the top of each says which; the rows are left as measured on their dates.
- The "v1" criteria above go into an ADR when the owner accepts them.

## Order

1. This file and the Track D choices. 2. Track A steps 1 to 5, beside Track B steps 1 and 2. 3. Track A steps 6 to 8, beside Track B steps 3 to 6. 4. Track C, then a release candidate (`packaging/README.md`: the version commit on `main`, then `git tag -s`). 5. The last hand checks, then v1.

## What an agent cannot do

A window on the owner's desktop, the owner's keys (root key, GPG, YubiKey), a merge, a push and a tag are the owner's (`CLAUDE.md`). Work that needs them is queued here and asked one question at a time.
