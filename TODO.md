# TODO

Suggestions for what to do next, most useful first. Not normative: the ADRs and specs are the
source of truth.

## 1. Components that build on `Input`

Built, following `docs/spec/odk-text-input.md`.

- [x] `Textarea`: reuses the editing model from ADR 0038 and adds multiple lines, wrapping and
  vertical scrolling. The smallest next step; needs no new design decisions.
- [x] `InputGroup`: prefixes, suffixes and buttons around an `Input`. Mostly layout.
- [x] `Command`: a palette made of an `Input` and a filtered list, with `Select`'s keyboard and
  typeahead logic. Shows whether the `Select` and `Input` foundations work together.

Order they were built: `Textarea`, `InputGroup`, `Command`. Each has a checklist test module and an
entry in `COMPONENTS` in `crates/odk-ui/src/checklist/source.rs`.

## 2. Foundations that block other components

Components that need drag or virtualisation are blocked until these have ADRs. Write the ADRs
before any code.

- [ ] Drag: sliders, resizable panels, scroll bars. `Input` drag-selection already settled part
  of it.
- [ ] Virtualised lists.
- [ ] The motion driver of ADR 0059 (accepted 2026-10-27): the steps under "First implementation" in
  `docs/spec/odk-motion.md`, starting with one driver behind today's private calls.

## 3. Left for the user

- [ ] Hand checks on `Select`, items 5 and 11 of `docs/spec/component-checklist.md` (hover,
  press and the focus ring under a real pointer). They cannot be tested headlessly; about two
  minutes in the gallery.
- [ ] Hand checks on `Textarea`, `InputGroup` and `Command` (items 4, 5, 11 of `docs/spec/component-checklist.md`: both stock themes, and hover, press and the focus ring under a real pointer; the group's ring and the palette's highlight in particular). Their layout, worst-case content and keyboard behaviour were checked headlessly.
- [ ] `Input::on_submit` and `InputGroup::on_submit` fire on the press of Enter. An application that closes a `Dialog` from `on_submit` gets focus back on its opener in time for the release, which presses it (GPUI presses a focused button on the release of Enter or Space; see `docs/checks.md`). `Command` acts on the release for this reason. Decide whether `Input` should too.
- [ ] Decide whether the host should draw every `Dialog` and `Sheet`. ADR 0039 lifts only the
  overlays opened inside a modal. Drawing every modal in the host needs its own answer to layer
  order, so do it only if the current design is a problem.

## 4. The OMA side

Not built: `.desktop` registration, upgrades, `rollback`, `purge` and the path units. None
depends on ODK, so this can run in parallel with the ODK work. `.desktop` registration is the one
users would notice, because installed applications would show up in launchers.

ADR 0052 to 0054 and ADR 0060 (accepted) replace the directory Bundle this list assumed with one `.oma`
file, and describe the MIME type, the path units, the install of `odk-oma` (a verified file, later
Omarchy's repository) and the signature. ADR 0060 supersedes ADR 0055. Ed25519ctx is ADR 0061. The code still installs a directory (ADR 0022). The
phases are issues #10 to #16, indexed from `docs/oma-trust-plan.md`, which also lists the questions ADR 0060 does not settle.
The work below follows the accepted ADRs.

- [ ] `.desktop` registration
- [ ] Upgrades
- [ ] `rollback`
- [ ] `purge`
- [ ] Path units

## 5. Housekeeping

- [x] `main` is pushed. The commits an earlier draft of this list named (`53797ff`, `b6a4561`)
  are not in the history.
- [x] ADR 0046 is superseded by ADR 0048: the user's own `shell.toml` is not a reduced-motion
  source, even when Hyprland does not answer. `odkd` tells every application, a sandboxed one too (ADR 0062).
  ADR 0047 and ADR 0048 are accepted. ADR 0049 is accepted with `odk::db` behind the `db`
  feature of `odk`, off by default, and the bundled SQLite kept.
- [ ] Update `docs/checks.md` with whatever the `Select` and overlay work settled, if anything
  is still marked unverified.
