# Packaging

What is installed besides the programs. Nothing here is run from the repository.

| File | Installed as |
|---|---|
| `systemd/user/odkd.socket` | `~/.config/systemd/user/odkd.socket` |
| `systemd/user/odkd.service` | `~/.config/systemd/user/odkd.service` |

`odkd` tells every ODK application what the host says (ADR 0062). It is installed for one user,
by the install script and without root: the program as `~/.local/lib/odk/odkd`, and the two
units in the user's own systemd directory, enabled with

    systemctl --user enable --now odkd.socket

The socket exists from login, at `$XDG_RUNTIME_DIR/odk/odkd.sock`, and the first connection
starts the service. `odkd.service` names the program with `%h`, so the units hold no user name;
if the program is somewhere else, change `ExecStart`. They are disabled with
`systemctl --user disable --now odkd.socket odkd.service`.
