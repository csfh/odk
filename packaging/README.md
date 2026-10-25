# Packaging

What the `oma` package installs besides the programs (ADR 0044). Nothing here is run from the
repository.

ADR 0054 (accepted) is the rest of that package: the MIME type, the desktop file, the
registration units and the `binfmt_misc` entry (ADR 0052, ADR 0053). Those files are not added
here yet. ADR 0054 also fixes the install path these units assume, `/usr/bin/oma`.

| File | Installed as |
|---|---|
| `systemd/user/oma-omarchy.path` | `/usr/lib/systemd/user/oma-omarchy.path` |
| `systemd/user/oma-omarchy.service` | `/usr/lib/systemd/user/oma-omarchy.service` |

The units keep oma's copy of the user's text size and font current: the path unit starts the
service when `~/.config/omarchy/shell.toml` or `~/.config/fontconfig/fonts.conf` changes, and the
service runs `oma omarchy sync`. The service is also wanted by `default.target`, so the copy is
made at login. They are enabled for every user when the package is installed:

    systemctl --global enable oma-omarchy.path oma-omarchy.service

and disabled when it is removed (`systemctl --global disable ...`). They assume `oma` is
installed at `/usr/bin/oma`; if it is not, change `ExecStart` to where it is.
