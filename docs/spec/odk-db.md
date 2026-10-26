# ODK database, version 1

Normative. This spec says what `odk::db` (the `odk-db` crate) gives an application and what it promises. It fills in ADR 0049 and must not change it; where the two disagree, the ADR wins.

## What it is

A `Database` is a cloneable handle to one SQLite connection that lives on a thread of its own. Everything an application asks of it runs there, one thing at a time, in the order it was asked for, and never on the thread that draws the window. SQLite is the bundled build of `rusqlite` (`odk::db::rusqlite` is the raw binding, for what the helpers do not cover). Applications reach it as `odk::db` by enabling the `db` feature of the `odk` crate, which is off by default (ADR 0049).

| | |
|---|---|
| `Database::open(path, &Migrations)` | The file `path`, made with its directory if it is not there, in WAL mode with `synchronous = NORMAL`, foreign keys on and a five-second busy timeout, brought up to date. Blocks. |
| `Database::memory(&Migrations)` | A database in memory, gone with its last handle. |
| `call(f)`, `call_blocking(f)` | `f: FnOnce(&mut Connection) -> Result<T>` on the connection's thread; the first returns a future that needs no particular executor and may be dropped without stopping the call, the second waits. |
| `execute(sql, values![..])`, `query(sql, values![..], map)`, `transaction(f)` | The common cases of `call`: a statement, a query mapped row by row, and a closure that commits when it returns `Ok` and is undone when it does not. |
| `values![..]` | A statement's parameters: text, `i32`, `i64`, `u32`, `f64`, `bool`, bytes, `None`, and `Value`. |
| `data_file(app, file)` | `file` under `app` in `$XDG_DATA_HOME` or `~/.local/share`. Inside the sandbox `$HOME` is the application's own data directory (`docs/spec/sandbox-baseline.md`), so this is private to the application and kept between runs. |

## Promises

- **Order.** Calls run in the order they were made, whichever handle made them, whether or not anyone waits for the answer. A call made after another, on any handle, sees what the first did. This is what lets a view send a change and go on, and what lets a test wait for all of them by making one more call.
- **Migrations** are an append-only list of scripts. Each runs in its own transaction and sets `PRAGMA user_version` to the number applied, so a script that fails leaves the database as it was before it, and the ones before it applied. A database whose version is past the end of the list is refused with `Error::TooNew` and left alone. A script that has been applied is never edited; a change is a new script.
- **A closure that panics** ends the database. That call returns `Error::Panicked`, the connection is closed, calls queued behind it and every later call return `Error::Closed`, and nothing waits for a thread that is gone.
- **Errors** are `Error::Sqlite` (what SQLite said), `Io` (the directory could not be made, with its path), `TooNew`, `Panicked` and `Closed`. Nothing here panics on bad SQL or a bad path.
- **No GPUI.** The crate depends on none, so a database is tested without a window, and a future from it runs on GPUI's executor as on any.

## How an application uses it

Open once, before the window, and read what the first frame needs with `call_blocking`, as the theme is read. Keep what the view draws in memory. Send each change with `call` and do not wait for it; if it fails, the future says so, and the view shows a notice (`examples/todo` does this). A change that failed means the database may hold less than the view shows, so a later change that merely worked must not take the notice away: `examples/todo` sends the whole list in one transaction as its next change after a failure, and only that working clears the notice. A view that waited on the database would be slower than the disk and would make a test depend on a thread.

## What it does not do

No ORM, no query builder, no connection pool (one writer, so one connection; a second `Database` on the same file serves a read-heavy case), and no access for Plugins, which run in the application's process with Luau as their only boundary (ADR 0049).

## In the sandbox

The data directory is read-write and holds the `-wal` and `-shm` files; `/tmp` is a private tmpfs for what SQLite spills there; seccomp is a denylist that does not name what SQLite uses. The sandbox test runs the system's `sqlite3` (WAL, a transaction, a sort that spills), not ODK's own build; see `docs/checks.md`.
