---
status: amended by ADR 0052 and ADR 0053
---

# Bundles live in `~/Applications`; data and rollback copies live under XDG directories

The installed Bundle is the directory `~/Applications/<App>.oma/` (ADR 0022), as `/Applications/<App>.app` is on macOS, so removing an application means deleting its directory. omalaunch's own state lives under `~/.local/share/oma/`, never inside `~/Applications`: `data/<identifier>/` for per-application data, `previous/<identifier>/` for rollback copies (ADR 0011), `inbox/<identifier>/` for staged upgrades (ADR 0015) and `grants/<identifier>.toml` for consent records (ADR 0023). A Bundle run from anywhere else still works. Installs are per-user only and omalaunch never needs root.

Omarchy's launcher is assumed to list only applications with a `.desktop` entry (unverified, ADR 0014), so omalaunch must discover Bundles that were merely dropped into `~/Applications`. A `systemd --user` path unit runs `omalaunch --register` when `~/Applications` changes, and a template path unit instance per installed application does the same when that application's inbox changes (ADR 0015), instead of a long-running daemon. A one-shot run at login catches changes made while the session was not running. `omalaunch --register` reconciles rather than only adds: it generates entries for Bundles that exist and removes the entries of Bundles that are gone, and it is idempotent. No Omarchy hooks are installed; they cannot signal removals (ADR 0014). The cost is a dependency on systemd user units, which Omarchy provides, and the directory must exist before the path unit can watch it, so omalaunch creates it and enables the unit on first use.
