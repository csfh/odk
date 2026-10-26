# Changelog

Every crate is at version 0.0.0 and nothing has been released, so there is one section, "Unreleased", and no version number has been chosen. When a first release is cut, this section becomes its notes. The decisions behind each line are the ADRs it names.

## Unreleased

### The design language (ADR 0040 to 0047)

- Four token layers, each specified on its own: the source palette from Omarchy's `colors.toml`, an adaptive colour resolver that makes semantic colours in OKLCH with contrast checked on what is drawn, foundation tokens (geometry, density, type, spacing, motion) and a component layer of typed styles by variant, density and state. `odk::ui::theme_snapshot()`, `foundation()` and `components()` reach them.
- A corner is 0 or 2 px and nothing is rounder (ADR 0042 replaced the first draft's 0.8 px, which could not be seen). Borders 1 px, the focus ring 2 px outside the control. Controls are 28, 34 and 40 px at Omarchy's default 12 px text size, in three densities, and scale with the user's text size.
- A theme that cannot be loaded or resolved never stops an application: the last good snapshot stays (ADR 0041). Themes are followed live and any installed theme can be previewed without touching Omarchy's (`odk::ui::preview_theme`).
- `odk-theme` follows `odkd` (ADR 0062): `ThemeManager::current()` and `from_host` take the palette, the text size, the request for reduced motion and the font from it and read no file, `with_host` and `following_host` give a manager on a source the host's text size and motion, and `Host`, `HostState` and `socket_path` are the client. `ODKD_SOCKET=none` says there is no host. `with_users_files`, `with_user_shell`, `with_user_fonts`, `user_files`, `with_compositor` and `compositor` are removed, and an application no longer runs `fc-match`.
- Text follows Omarchy's font and the user's text size, and reduced motion follows the compositor: Hyprland's `animations:enabled` set to false removes transitions and loops, live, in every application, a sandboxed one too (ADR 0043, ADR 0048, accepted; ADR 0046 is superseded). The user's own `[motion] reduced` key is ignored. When Hyprland gives no answer, motion is not reduced. `odkd`, a per-user daemon outside every sandbox, reads these and the theme's colours and tells every application over a socket systemd starts on demand (ADR 0062); `oma` binds its directory into a sandbox, and the copy of Omarchy's settings that ADR 0044 made, with `oma omarchy sync` and its units, is gone.
- Components: `Button` (eight variants, any role at any emphasis, icon-only, loading), `Input`, `SearchInput`, `PasswordInput`, `NumberInput`, `Textarea`, `InputGroup`, `Select`, `Command`, `Checkbox`, `RadioGroup`, `Switch`, `FormField`, `Text`, `Heading`, `Tabs`, `ToggleGroup`, `Breadcrumb`, `NavList`, `Badge`, `StatusIndicator`, `Alert`, `Progress`, `Spinner`, `Skeleton`, `Separator`, `Kbd`, and the overlays `DropdownMenu`, `ContextMenu`, `Popover`, `Tooltip`, `HoverCard`, `Dialog`, `Sheet` and `Toast` (ADR 0045).
- The gallery is a design-system explorer (`docs/gallery.md`).

- `odk::db`: a SQLite handle for an application's own data, over `rusqlite` with a bundled SQLite, on a thread of its own so that the window never waits for a disk, with migrations (`PRAGMA user_version`; a newer database is refused), `execute`, `query`, `transaction` and the raw connection (ADR 0049, accepted, `docs/spec/odk-db.md`). It is behind the `db` feature of `odk`, off by default.
- `examples/todo`, an example application with a list kept in that database, and its Bundle `Todo.oma`.

### Breaking changes (ADR 0047, accepted)

Removed: `odk::theme::Tokens` and the types under it, `ThemeSnapshot::tokens`, `odk::ui::tokens()`, the radius that was 0, `odk::ui::Variant` and `Badge::variant`. Replacements, with before and after code: `docs/migration.md`. A theme's `shell.toml` is no longer read for colour (only `[font] base-size`).

### Phase 12: validation and release readiness

- Reduced motion follows Omarchy's own setting: Hyprland's `animations:enabled`, asked over its socket and followed on `configreloaded` (ADR 0048, accepted). A Hyprland restart is followed: the watcher reconnects to the new instance, and the last answer stands meanwhile. When Hyprland gives no answer, motion is not reduced; a sandboxed application cannot ask it, and `odkd` tells it (ADR 0062), and the user's own `shell.toml` key is ignored (ADR 0046, superseded). `Hyprland` and `ReloadWatcher` are new (`odkd` uses them); `ThemeDiagnostics` has a `compositor_animations` field; a previewed theme carries the followed theme's request.
- Fixed: `Toast::timeout(Duration::MAX)` panicked on the `Instant` addition and is now a toast that stays; `NumberInput` showed `NaN` or `inf` for such a value, ignored a NaN `min` or `max`, and turned 400 digits into `inf`; the resolver's one `expect` on a label is an error, not a panic; `Skeleton::block` takes no negative or non-finite size.
- Added source checks to the component checklist: no palette, colour maths or snapshot read outside the theme layers in any `odk-ui` file (item 40), no timing or named corner size in a component (41), no process-wide theme state outside the store (42), and more colour forms in item 1.
- Added a cross-theme matrix test (six themes, three densities, both radii, every button variant and state) and a timing printout for theme resolution.
- Added `tools/visual-scenarios` and the gallery's `GALLERY_DENSITY`, `GALLERY_CORNERS` and `GALLERY_MOTION` for reproducible headless screenshots.
- Documentation: `docs/design-language.md`, `docs/migration.md` (its code is compiled by `crates/odk/tests/migration.rs`), `docs/contributing-components.md`, `docs/release-readiness.md`; stale statements about the removed tokens and `Variant` corrected in the specs.

### OMA

`oma` verifies, installs, sandboxes and runs Bundles, with the consent dialog. Not built: `.desktop` registration, upgrades, `rollback`, `purge`, and the registration and inbox path units (`TODO.md`).

- Accepted, not implemented: a Bundle is one EROFS `.oma` file (ADR 0052), the desktop opens it with `oma` (ADR 0053), and a Bundle's trust is the publisher's domain, then its pinned ed25519 key, then the download origin (ADR 0055). Any new key is the first-install consent flow. `oma` itself is installed by a script from its domain, which verifies the release against that domain's well-known key, and later by Omarchy's repository (ADR 0054). The code still installs a directory and still verifies no signature.
