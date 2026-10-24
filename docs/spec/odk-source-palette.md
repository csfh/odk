# ODK source palette, version 1

Normative. This spec defines the source layer of ADR 0040: how `odk-theme` finds a theme, reads its `colors.toml` into a Palette, keeps it and replaces it, and what an application draws when it cannot (ADR 0041). It must not change a decision recorded in an ADR; if it needs to, write a new ADR first. Where the two disagree, the ADR wins.

Built in `odk-theme` (`palette.rs`, `discovery.rs`, `manager.rs`, `watch.rs`) and in `odk-ui`'s store (`theme.rs`), on 2026-10-22.

## Layers

| Layer | What it holds | Built from | Where |
|---|---|---|---|
| Source | The Palette: the theme's colours by Omarchy's names, exactly as written | the theme's `colors.toml` | `odk_theme::Palette`, this spec |
| Foundation | Geometry, density, spacing, type, motion: ODK's own (ADR 0035), scaled by the user's text size (ADR 0043) | the application's configuration and the text size | `odk_theme::foundation`, `docs/spec/odk-foundation-tokens.md` |
| Semantic | Colours by role and emphasis (ADR 0040) | the Palette, through the resolver | `odk_theme::semantic`, `docs/spec/odk-semantic-tokens.md` |
| Component | The final style each component draws | semantic and foundation tokens | not built |

The source layer gives no colour a meaning: `red` is the colour the theme called `red`, not "danger". The semantic layer gives colours their roles, and every component draws it through the component tokens. A change of theme never touches the foundation layer, which does not read the Palette at all.

## `colors.toml`

| Key | Required | Short name also read |
|---|---|---|
| `mode` | yes, `"dark"` or `"light"` | |
| `accent` | yes | |
| `selection` | | |
| `selection_background` | | |
| `selection_foreground` | | |
| `muted` | yes | |
| `background` | yes | `bg` |
| `dark_background` | yes | `dark_bg` |
| `darker_background` | | `darker_bg` |
| `lighter_background` | yes | `lighter_bg` |
| `foreground` | yes | `fg` |
| `dark_foreground` | | `dark_fg` |
| `light_foreground` | | `light_fg` |
| `bright_foreground` | | `bright_fg` |
| `red`, `yellow`, `green`, `blue` | yes | |
| `orange`, `cyan`, `magenta`, `brown` | | |
| `bright_red`, `bright_yellow`, `bright_green`, `bright_cyan`, `bright_blue`, `bright_magenta` | | |

That is 27 colours and the mode. `odk_theme::PaletteKey` names each colour key, and `Palette` has one field per key under the same name: `Color` for a required key, `Option<Color>` for an optional one.

- **Required keys** are the ones the semantic layer cannot resolve without (the legacy mapping that first needed them is gone, ADR 0047). Every stock Omarchy theme defines them, and so does every theme installed on the development machine (see Checked). A theme without one is an error naming the long key.
- **Optional keys** are `None` when the theme leaves them out. Nothing is filled in at this layer: a missing `bright_red` stays missing, a monochrome theme stays monochrome, and an equal `red` and `bright_red` stay equal. Which colour stands in for a missing one is the semantic layer's choice (ADR 0041, `docs/spec/odk-semantic-tokens.md`); the choice of a label on the accent when `selection_foreground` is missing is the semantic layer's too.
- **Short names** are the eight that Omarchy's own resolver, `omarchy-theme-color`, accepts as legacy palette names; Aether writes some of them. When a theme writes both names, the long one wins and the short one is not read. An error in a value written under the short name names the short name, where the author must fix it.
- **Colours** are TOML strings `#RRGGBB` or `#RRGGBBAA`, hex digits in either case, whitespace around them ignored. Channels are kept as the 8-bit values written: `Color::to_rgba8` and `Color::to_hex` give them back exactly. `#RRGGBBAA` is accepted because `docs/spec/odk-theme-tokens.md` always accepted it; no installed theme uses it.
- **Every key ODK knows is checked**, required or not. A malformed value is an error that names the key and says what is wrong; it is never replaced with something else.
- **Keys ODK does not know are ignored**, whatever they hold, so a theme written for a newer Omarchy, or one with Hyprland gradients such as `hyprland_active_border`, still loads.
- **Not followed from Omarchy's resolver**: the legacy `theme_type` key, a `light.mode` file, guessing the mode from the background, the ANSI names `color0` to `color15`, and `purple`. Every installed theme writes `mode` and the semantic names, so none of them is needed today. A theme that relies on them does not load, and ADR 0041's fallbacks apply.

### Errors

`odk_theme::Error` tells each case apart, and its message names the file and the key:

| Case | Error |
|---|---|
| `colors.toml` missing | `Io` with `ErrorKind::NotFound` and the path |
| `colors.toml` or `shell.toml` not readable | `Io` with `ErrorKind::PermissionDenied` (or another kind) and the path |
| not TOML | `Toml`, naming the file |
| a required key missing | `MissingKey`, naming the long key |
| a colour malformed | `BadColor`, with the key as written, the value, and a `ColorError` saying why (no `#`, a non-hex character, or a digit count other than 6 or 8) |
| `mode` not `"dark"` or `"light"` | `BadMode` |
| a known key holding a number, table or other non-string | `WrongType`, naming the key and the TOML type |
| a named theme missing | `NoTheme`, listing both places looked in |
| a theme name refused | `BadThemeName` |

No error panics, and none reaches the user as a crash (ADR 0041).

## Discovery

`odk_theme::ThemeSource` says which theme an application follows. Paths come from the environment only in `ThemeSource::current()` and `ThemeSource::named()`; tests and tools build a source from explicit paths.

- **The current theme**, `ThemeSource::Current`: `$XDG_STATE_HOME/omarchy/current/theme/`, with `$XDG_STATE_HOME` defaulting to `~/.local/state` (`docs/checks.md`). Its name is `current/theme.name`, when that holds a valid theme name. `odk::ui::run` follows this source.
- **A named theme**, `ThemeSource::Named`, looked up as `omarchy-theme-set` does. The name is normalised as Omarchy normalises it (lower case, spaces to dashes: `"Tokyo Night"` is `tokyo-night`); a name that is empty, starts with `.` or holds `/` is refused, so none can leave the themes directory. Each file comes from `~/.config/omarchy/themes/<name>/` when the user's copy has it, else from `$OMARCHY_PATH/themes/<name>/` (`$OMARCHY_PATH` defaults to `/usr/share/omarchy`), so a user theme lays its files over the stock theme of that name. A user theme that is a symlink is followed; one whose target is gone counts as absent.
- **A directory**, `ThemeSource::Dir`: read as it is, named after itself.

- **Which themes are installed**, `ThemeSource::installed()` (`installed_in(user, stock)` takes the two directories): the sorted names of the directories in those two places that hold a `colors.toml`, in the user's copy or the stock one, as `resolve` would find it. Names `theme_name` would change or refuse are left out. Only the directories' listings are read. It exists for tools that offer a choice of themes, such as the gallery (added 2026-10-23).

Nothing else is searched. Inside oma's sandbox only `current/` is reachable (`docs/spec/sandbox-baseline.md`), so a named theme is for tools and tests. Where Omarchy is not installed, the current theme's files are missing and the application draws the emergency palette.

## Text size

The user's text size is the one thing besides colours that is read (ADR 0043), with, in the user's own file only, a request for reduced motion that counts when the compositor does not say (ADR 0046, ADR 0048: Hyprland's `animations:enabled` is the user's request, and is not read from a file). `odk_theme::TextScale` is `[font] base-size` over 12 px: `ThemeManager::current()` reads it from `~/.config/omarchy/shell.toml`, where `omarchy display text size` writes it, over the theme's own `shell.toml`, and the snapshot carries it as `ThemeSnapshot::text`. Rules:

- Only `[font] base-size`, an integer or a float, is read, and `[motion] reduced`, a boolean, from the user's own file (ADR 0046; a theme's is ignored; it reaches `ThemeSnapshot::reduced_motion` unless the compositor answers, ADR 0048). Nothing else in the user's file is, and the per-token `[font]` keys, `[spacing]` and the `*-border-width` keys of either file stay ignored.
- It is kept to 9 to 20 px, as Omarchy keeps it, and is 12 when neither file has one.
- The user's file is never the reason a theme fails: missing, unreadable or not TOML, it says nothing and the theme's size is used. `ThemeManager::new`, named themes and directories read no user file, so tests and tools do not depend on the machine's own setting; `with_user_shell` adds one.
- A change of the scale alone is a change: the snapshot differs, it is published and every window is drawn again. `ThemeWatcher::follow_with` watches the directory of each of the user's files, `shell.toml` and `fonts.conf` (`ThemeManager::user_files`), without recursion, since Omarchy replaces such a file by renaming a new one over it, and reports only an event on the file itself; those directories hold other files that change often. A change to `fonts.conf` is not read by `odk-theme`: it makes `odk-ui` look the font up again (`docs/spec/odk-foundation-tokens.md`, "Typography").
- Inside oma's sandbox the file is oma's copy of it, mounted at the same path (ADR 0044, `docs/spec/sandbox-baseline.md`): it holds only `[font] base-size` and `[motion] reduced` (ADR 0046), which `oma` writes when the user's file has it or Hyprland's animations are off (ADR 0048), and is replaced by a rename inside the watched directory, so the watcher sees it. Where there is none, the size is the theme's.

## Reload

`odk_theme::ThemeManager` is the one place a theme is loaded and kept. It reads files only in `reload`, which the caller runs off the UI thread; reading its snapshot never touches the disk.

- **The snapshot**, `ThemeSnapshot`, is immutable and shared behind an `Arc`: the theme's name, the `colors.toml` it came from, the Palette, the user's text size and request for reduced motion, the semantic colours resolved from the same Palette and the report of how (`docs/spec/odk-semantic-tokens.md`), a generation, and whether it is the emergency palette.
- **A reload** resolves the source, reads `colors.toml` and `shell.toml` once each, and builds the Palette, the text scale and the semantic colours from that text. It is `Changed` when the name, the path, the Palette, the text size, the request for reduced motion or the semantic colours differ from the snapshot: the new snapshot replaces it, one generation on. The text size and the request are compared as well as the Palette because a change to `shell.toml` alone changes what is drawn; the rest of a theme's `shell.toml` is not read (ADR 0047), so an edit to it publishes nothing. It is `Unchanged` when all are the same, and nothing is published. It is `Failed` when the theme cannot be found or read, or its semantic colours cannot be resolved: the snapshot stays, and the failure is kept for diagnostics until a reload succeeds.
- **The generation** is 0 for the emergency palette a manager starts on and goes up by one with each published change, including a change of colour options (`ThemeManager::set_options`). A cache derived from a snapshot stays valid while the generation is the same.
- **Watching**: `ThemeWatcher::follow` watches the source's directories, recursively, without following symlinks, and coalesces events until they have stopped for 150 ms, counting only those that change something: `notify` also reports a file being opened, and a reload opens the theme's files, so counting those made every reload start the next (about eleven a second at rest, found on 2026-10-23). For the current theme it watches `current/`, which outlives every switch, so a theme directory moved into place and a file edited inside it (written in place or renamed over) are both seen. One theme switch reports several times; a report can arrive before the swap, a reload can land between Omarchy's delete and move and fail, and `theme.name` is written after the move, so a reload can see the new colours under the old name and the next one the new name. Each of those is handled by the rule above: a failure keeps the last theme, and a name arriving late is one more change.
- **In `odk-ui`**: `run` loads the theme before the first window, then follows it. Reloads run one at a time on GPUI's background executor, requested by the watcher or by `odk::ui::reload_theme`; requests that queue up during a reload are answered by it. Only a `Changed` reload is published, and then `cx.refresh_windows()` redraws every window on the next frame. The store holds one slot, the snapshot with the component tokens made from it, and the application's reduced motion is in the foundation as the snapshot is published. Components read `odk::ui::components()` and `odk::ui::foundation()` inside `render`; `odk::ui::theme_snapshot()` gives the snapshot to code that needs the Palette or the semantic colours.

## Previewing a theme

An application that is not choosing a theme but showing one, as the gallery's theme selector and comparison do, loads it and draws it in place of the followed theme without touching Omarchy (added 2026-10-23; ADR 0041's fallbacks and `run`'s source are unchanged):

- `ThemeManager::with_users_files(source)` is a manager for `source` that reads the user's `shell.toml` and watches `fonts.conf` as `ThemeManager::current()` does. A preview loaded with `ThemeManager::new` would carry the default text size and no request for reduced motion, and publishing it would rebuild the foundation at the wrong size. The caller sets the colour options (`ThemeManager::set_options`, with `odk::ui::colour_options`) before `reload`, so a preview is resolved as the followed theme is, and loads on an event or a background task, never in `render`.
- `odk::ui::preview_theme(cx, snapshot)` publishes that snapshot to every window and keeps it. The foundation (density, corners, reduced motion) stays the application's: `configure` while previewing applies to the previewed theme. `odk::ui::end_theme_preview(cx)` draws the followed theme again; `previewed_theme` and `followed_theme` say which is which.
- While a preview lasts, the followed theme still loads, reloads and is watched: what a reload finds is kept as the followed theme and is not drawn, so ending the preview draws the followed theme as it is then. `set_colour_options` applies to the followed theme only. A preview is process-wide, like the theme store: there is no preview scoped to a window or a subtree, so two themes cannot be drawn by real components at once.

## Fallbacks

As ADR 0041 orders them: the theme being followed when it loads; else the last theme that loaded, when a reload fails; else, when no theme has loaded since the application started, the emergency palette.

The emergency palette is `mode = "dark"`, `background` `#1c1c1c`, `dark_background` `#141414`, `lighter_background` `#2a2a2a`, `foreground` `#d0d0d0`, `muted` `#8a8a8a`, and `accent`, `red`, `yellow`, `green` and `blue` all `#d0d0d0`. It defines the required keys only and goes through the same parser and readability checks as any theme. It is not a theme: nothing names it, and diagnostics say when it is drawn.

## Diagnostics

`ThemeManager::diagnostics`, and `odk::ui::theme_diagnostics` in an application, give a `ThemeDiagnostics`: the source, the theme's name and `colors.toml` (whose directory is the theme's), the mode, how many of the 27 colours the theme defines, the optional keys it leaves out, the fallback in use, the latest reload's failure with the file it was reading, the generation, and the semantic layer's strategy, whether the palette is monochrome and how many colours were adjusted for contrast. `Display` writes a few lines and never a colour value. In an application it is kept from the latest reload, so asking for it never waits for one.

`odk-theme` logs through the `log` crate, which writes nothing unless the application installs a logger: `info` when a changed theme is published (name, mode, path, generation), `warn` when a reload fails while the emergency palette is drawn, and `debug` for any other failed reload, since every theme switch can fail once mid-swap. No colour is logged.

## Rules for code

- Components draw tokens. They never hold an Omarchy colour, never name a palette key, and never read the Palette to choose a colour themselves.
- Nothing reads or parses a theme file in `render`. Files are read only by `ThemeManager::reload`, off the UI thread.
- The source layer derives nothing: no generated shades, no fallback colours, no roles. Those belong to the semantic layer (`docs/spec/odk-semantic-tokens.md`).
- Do not assume a bright colour is brighter, or a status colour saturated. In Tokyo Night `bright_yellow` is darker than `yellow`; in Vantablack every colour is grey.
- Keep source values exact; compare and cache by generation, not by re-reading.
- Go through the one store: `odk::ui::components()`, `odk::ui::foundation()` and `odk::ui::theme_snapshot()`. Do not keep a palette of your own.
- A theme change never rebuilds foundation tokens, and a foundation change never re-reads the theme.

## Writing a theme that ODK reads

- Define `mode` and the ten required colours. Use the long names; the short names are read but are a compatibility path.
- Write every colour as a quoted `"#rrggbb"`. A colour ODK reads that is malformed rejects the whole theme, so check optional keys too.
- Leave out what your theme does not have rather than repeating a colour to fill a slot. A missing key is fine; keys ODK does not know are ignored.
- To check a theme, load it with `odk_theme::Palette::load` or run any ODK application with it as the current theme: if it does not load, `odk::ui::theme_diagnostics` names the key and the problem.

## Checked

On 2026-10-22, against the `omarchy` package 4.0.6-1 (its `version` file says 4.0.0.alpha): all 45 themes installed on the development machine, 22 stock and 23 the user's own, parse. The tests assert it for the stock themes (`every_stock_theme_parses`) and only report the user's own, which a test must not depend on. Every one writes `mode` and the required keys; only keys ODK does not know (`hyprland_active_border`, `hyprland_inactive_border`) hold anything but a `#RRGGBB` colour; 14 write `selection_foreground`, 3 `selection_background`, and 2 use the short names. The stock `white` leaves out `orange` and `brown`, so an optional key is missing even in a stock theme. The fixtures in `crates/odk-theme/tests/fixtures/` are copies of five stock themes and two custom ones. A live switch, a switch to a malformed theme, a start on a malformed theme and a start with no Omarchy state were each run in the gallery under `tools/headless-shot` with a private `XDG_STATE_HOME` (`docs/checks.md`).

Not checked: other Omarchy releases, and whether inotify reports changes through oma's read-only bind mount of `current/` (`docs/checks.md`).
