---
status: amended by ADR 0046
---

# oma shows a sandbox a copy of the user's text size and font

ADR 0043 has ODK applications follow the user's text size and Omarchy's font, which live in `~/.config/omarchy/shell.toml` and `~/.config/fontconfig/fonts.conf`; the sandbox of ADR 0012 shows an application neither. Exposing the files does not work: `omarchy display text size` replaces `shell.toml` by renaming a new file over it, so a bind mount of that one file stays on the old one (shown with `unshare -rm`, `docs/checks.md`), and the directory it is in holds `shell.json`, hooks and plugins that no application should read. The user asked on 2026-10-23 whether `oma` could keep the state and tell running applications, and chose, over a resident `oma` or a socket, this: `oma run` writes a copy of the two settings into a directory `oma` owns (`~/.local/share/oma/omarchy/`), and the sandbox mounts its two parts read-only where the real files are, `~/.config/omarchy` and `~/.config/fontconfig`. The application reads the same paths it would read outside the sandbox, so `odk-theme` needs nothing for it.

The copy holds only what an application needs: `omarchy/shell.toml` has `[font] base-size` and nothing else of the user's file (a number, 1 to 1000, or no file), and `fontconfig/fonts.conf` is the user's file as it is, if it is a regular file of at most 64 KiB. A copy is replaced by writing a temporary file in the same directory and renaming it over, so a reader sees the old file or the new, and the directory a sandbox has mounted is never replaced; a setting the user has taken away is removed. A copy that cannot be made is a note on stderr and never stops a launch.

## Considered Options

- **Mount the two files.** Stale for the size after the first change, as above.
- **Mount `~/.config/omarchy/` read-only.** Live, but shows `shell.json`, hooks, plugins and everything Omarchy keeps there, for every application.
- **A resident `oma`, or a socket-activated one, telling applications.** ADR 0025 left a socket-activated daemon open for later and ADR 0024 forbids a control socket in the init. It needs a protocol, a client in ODK and a socket in every sandbox, for two values that a file carries.
- **No change.** Bundled applications would draw the theme's size and fontconfig's default face, whatever the user chose.

## Consequences

The copy is refreshed whenever any application is launched with `oma run`, and a running application whose `odk-theme` watches `~/.config/omarchy` sees the new size at once, since the directory it watches is the one the copy is replaced in. To refresh it when the user changes the setting, and not only when the next application starts, the `oma` package ships two `systemd --user` units (`packaging/`) and enables them for every user when it is installed (`systemctl --global enable`), so there is nothing for the user to run (the user asked for this on 2026-10-23, in place of a command that installed them). `oma-omarchy.path` has `PathChanged=` on `~/.config/omarchy/shell.toml` and `~/.config/fontconfig/fonts.conf` and starts the one-shot `oma-omarchy.service`, which runs `oma omarchy sync`; the service is wanted by `default.target` too, for the copy at login. This is the path unit of ADR 0009 and ADR 0015, with nothing resident. `oma omarchy sync` is the one command, and `oma run` runs the same copy before every launch. The units assume `oma` is installed at `/usr/bin/oma`, which no ADR has decided; the package changes `ExecStart` if it is elsewhere. A running application that does not watch (anything not on ODK) is not told; one on ODK sees both the size and the font change, since it watches the same two paths inside the sandbox.

A user who has set neither gets an empty copy, and `docs/spec/sandbox-baseline.md` gains a row for it.
