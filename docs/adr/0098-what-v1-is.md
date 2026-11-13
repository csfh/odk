---
status: accepted
---

# What v1 is

`docs/roadmap.md` proposed five criteria for v1 on 2026-11-11 and left them "proposed, not yet accepted". Since then GPUI is gone (ADR 0094), the Plugin host is built (ADR 0091, ADR 0092) and registration is settled (ADR 0097). The owner set the shape of v1 on 2026-11-13: **Rune only, no GPUI, `.oma` files, and Luau Plugins.** This record turns that into criteria a release can be held to. The owner accepted it as written on 2026-11-13.

## Decision

v1 is the first release the owner calls production-ready. It is cut when all of these hold:

1. **Rune only.** `odk::ui` is `odk-view` on Rune, and `cargo tree -i gpui` prints nothing (ADR 0089, ADR 0094). A screen reader reads an ODK window, a sandboxed one included, through ADR 0096's grant, checked once on a real session.
2. **A Bundle is a signed `.oma` file, and nothing else.** `install`, `run`, `consent`, `rollback`, `purge`, `--register` and the first-install dialog take a signed Bundle file (ADR 0052, ADR 0060). The unsigned Bundle directory (ADR 0022, ADR 0032) is retired: a directory is only an intermediate step inside `odk oma bundle --sign` and `odk oma dev`, and is never installed or run. A separate ADR says how, and what becomes of a directory installed before v1.
3. **The OMA chain works for real on a stock Omarchy machine.** A release signed with the owner's root key (ADR 0086) is published on `odk.csfh.dev`. `install.sh`, then install, run, upgrade, rollback and purge of a Bundle file, then `odk oma self-upgrade`, are each run on a clean machine and written into `docs/checks.md` (ADR 0063, ADR 0087, ADR 0097).
4. **Luau Plugins work.** `odk::plugin` loads a Plugin from its manifest, grants exactly the capabilities it declared and the application allowed (`app.state`, `ui`, `network`, `storage`; `docs/spec/plugin-manifest.md`, `docs/spec/plugin-api.md`), contains the hostile Plugins in its tests, and has run once for real inside an application started by `odk oma run`.
5. **No known gap.** The checklist's "Known gaps" is empty, every component has its story, and every provisional value is settled or kept by the owner.
6. **`docs/checks.md` has no unverified claim.** Each one is verified, or dropped with a line saying why.
7. **The scope choices are made.** Motion (ADR 0059), the deferred components (`Combobox`, `Pagination`, submenus, non-modal dialogs, a dialog's default action on Enter) and `odk::db` are each in v1 or out of it, by the owner, in an ADR or a dated line in `docs/roadmap.md`. What is out is not a gap.

The release itself follows `packaging/README.md`: the version commit on `main`, then a tag made with `git tag -s`.

## Why

The roadmap's criteria held the owner's intent but had no decision behind them, and they predate three facts: GPUI is gone, Plugins are in v1, and the code still installs and runs unsigned Bundle directories beside signed files. Criterion 2 is new. A v1 that still ran an unsigned directory would ship two trust models, one of which verifies nothing, while ADR 0060 says an unverifiable Bundle fails closed. Criterion 7 makes "not in v1" a decision and not a gap, so it does not hold the release up.

## Considered

- **Keep directory Bundles in v1 for development.** Rejected: `odk oma dev` already signs with the dev key (ADR 0085), so development does not need an unsigned path.
- **Make v1 a code milestone and leave the real-machine checks for 1.0.1.** Rejected: the checks in criteria 3 and 6 are what "production-ready" means. A release that has only passed tests on fakes is a release candidate.

## Consequences

- `docs/roadmap.md`'s "What v1 is" points here, and its Housekeeping line about putting the criteria into an ADR is done.
- An ADR retiring the unsigned Bundle directory comes next. It amends ADR 0022 and ADR 0032, and the "The code still installs a directory" lines in `GLOSSARY.md` and `CLAUDE.md` are removed with the code.
