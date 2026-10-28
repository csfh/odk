---
status: accepted
---

# ODK ships a SQLite database handle over `rusqlite`

The user asked on 2026-10-24 that ODK come with a SQLite wrapper. An application with state of its own (the Todo example is the first) needs to keep it between runs, and the sandbox gives every application one place to do that, its own data directory (`docs/spec/sandbox-baseline.md`), where a database file works with no permission asked for. The request named the wrapper, not how to build it. The user accepted this on 2026-10-25 with one change: `odk::db` is behind the `db` feature of the `odk` crate, off by default. The bundled `rusqlite` stays.

**What it is.** A crate `odk-db`, which applications reach as `odk::db`, over `rusqlite` with its `bundled` feature. It has no GPUI dependency. A `Database` is a cheap, cloneable handle to one connection, which lives on a thread of its own: SQL runs there, in the order it was asked for, and never on the UI thread. `Database::call(f)` hands the thread a closure over `&mut rusqlite::Connection` and returns a future of its result, which any executor can run (GPUI's `cx.spawn` included); `call_blocking` is the same for a start-up read or a test. `execute`, `query` and `transaction` are the common cases written once (`docs/spec/odk-db.md`). Opening a file creates its directory and sets WAL, `synchronous = NORMAL`, foreign keys on and a five-second busy timeout.

**Migrations** are an append-only list of SQL scripts, applied in order, each in a transaction with `PRAGMA user_version` set to the number of scripts applied; a script that fails leaves the database as it was before it. A database whose version is newer than the application's list is refused (`Error::TooNew`), never opened and changed, so an older build cannot damage what a newer one wrote.

**A closure that panics** ends the database: the call that ran it returns `Error::Panicked`, the connection is closed, and every later call returns `Error::Closed`. Nothing waits for a thread that is gone.

**The escape hatch** is `odk::db::rusqlite`, as `odk::gpui` is for the UI (ADR 0004): a closure gets the real connection, and anything the helpers do not cover is plain `rusqlite`.

## Considered Options

- **The system's `libsqlite3`.** It is visible, read-only, inside the sandbox, and would save a C build. Rejected: the SQLite an application sees would be the machine's and not the one it was tested with, and features such as FTS5 and JSON are a build option there. The bundled build is the same everywhere and costs a C compiler at build time and a few hundred KiB.
- **`sqlx`, or a pure-Rust database.** A second async runtime, or a different file format, for something a thread and a channel do.
- **An ORM or a query builder.** SQL is the interface; the type mapping is a closure over a `Row`.
- **A connection on the UI thread.** Simple, and a slow query or a busy timeout freezes the window.
- **A pool.** SQLite has one writer; a single connection on one thread keeps order and has nothing to configure. A read-heavy application can open a second `Database` on the same file.

## Consequences

- ODK's facade has a fourth module beside `odk::ui`, `odk::theme` and `odk::a11y`, and `odk-db` is an internal crate under it like the others. Applications reach it as `odk::db` only with the `db` feature of the `odk` crate, which is off by default, so a build that does not ask for a database does not compile the bundled SQLite. ADR 0028, which lists the facade's modules, is marked amended. `oma` does not depend on it (ADR 0013).
- The first C dependency of the workspace besides the system's, and only for a build that enables that feature: `libsqlite3-sys`, built by `cc` from `rusqlite`'s `bundled` feature, so that build needs a C compiler and sees the same SQLite everywhere. An application that does not enable the feature does not link it: measured on 2026-10-24, before the feature existed, the release `hello` and `gallery` already contained no `sqlite3_` symbol (the linker dropped it), and `todo` contained 283 and was 3.2 MB larger than `hello` (SQLite and the application's own code together). The feature is what guarantees that now, rather than the linker. `cargo audit` runs in CI; it was not installed on the development machine when this was first written (`docs/release-readiness.md`).
- It works in the sandbox as it is: the data directory is read-write and holds the `-wal` and `-shm` files, `/tmp` is a private tmpfs for what SQLite spills there, and seccomp is a denylist that does not name what SQLite uses. The sandbox test runs the system's `sqlite3` through WAL and a sort that spills, not ODK's own build (`docs/checks.md`). `PRAGMA temp_store = MEMORY` was not needed and is not set.
- A database file is private to its application, and so is not shared between applications, nor visible to the user's other tools unless the application grants a directory. Backing it up is copying the file with its `-wal` while no write is in flight, or `VACUUM INTO`.
- Plugins (ADR 0003) run in the application's process with Luau as their only boundary and get no access to a `Database`; if they are to have it, that is a separate decision about what a Plugin may read and write.
- The application's own data directory under Run once (ADR 0018) is a tmpfs, so a database there disappears with the application, as everything does.
