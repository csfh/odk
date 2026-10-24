//! The database handle on in-memory databases and files of its own. Nothing here reads the
//! real home.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake};
use std::time::Duration;

use odk_db::{Database, Error, Migrations, params, values};
use tempfile::TempDir;

const SCHEMA: Migrations = Migrations::new(&[
    "CREATE TABLE items (id INTEGER PRIMARY KEY, text TEXT NOT NULL, done INTEGER NOT NULL DEFAULT 0)",
    "ALTER TABLE items ADD COLUMN note TEXT",
]);

/// Runs a future to its end on this thread, so that the async calls are tried without an
/// executor of the application's.
fn block_on<T>(future: impl Future<Output = T>) -> T {
    struct Unpark(std::thread::Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Arc::new(Unpark(std::thread::current())).into();
    let mut cx = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park_timeout(Duration::from_secs(5)),
        }
    }
}

fn version(db: &Database) -> u32 {
    db.call_blocking(|c| Ok(c.query_row("PRAGMA user_version", [], |r| r.get(0))?))
        .unwrap()
}

// Migrations.

#[test]
fn migrations_are_applied_in_order_and_recorded() {
    let db = Database::memory(&SCHEMA).unwrap();
    assert_eq!(version(&db), 2);
    // The second script's column is there.
    block_on(db.execute(
        "INSERT INTO items (text, note) VALUES (?1, ?2)",
        values!["milk", "semi"],
    ))
    .unwrap();
}

#[test]
fn opening_again_applies_only_what_is_new_and_a_newer_database_is_refused() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("a/b/app.db");
    const ONE: Migrations = Migrations::new(&["CREATE TABLE t (n INTEGER)"]);
    const TWO: Migrations =
        Migrations::new(&["CREATE TABLE t (n INTEGER)", "INSERT INTO t VALUES (7)"]);

    let db = Database::open(&path, &ONE).unwrap();
    assert_eq!(version(&db), 1);
    drop(db);
    let db = Database::open(&path, &TWO).unwrap();
    assert_eq!(version(&db), 2);
    let rows = block_on(db.query("SELECT n FROM t", values![], |r| r.get::<_, i64>(0))).unwrap();
    assert_eq!(rows, [7], "the second script ran once, on the old database");
    drop(db);

    // An older application meets a newer file: refused, and the file is as it was.
    let error = Database::open(&path, &ONE).unwrap_err();
    assert!(
        matches!(error, Error::TooNew { found: 2, known: 1 }),
        "{error}"
    );
    let db = Database::open(&path, &TWO).unwrap();
    assert_eq!(version(&db), 2);
}

#[test]
fn a_script_that_fails_is_undone_and_the_ones_before_it_stay() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("app.db");
    const BAD: Migrations = Migrations::new(&[
        "CREATE TABLE a (n INTEGER)",
        "CREATE TABLE b (n INTEGER); NOT SQL AT ALL",
    ]);
    assert!(matches!(Database::open(&path, &BAD), Err(Error::Sqlite(_))));

    const FIRST: Migrations = Migrations::new(&["CREATE TABLE a (n INTEGER)"]);
    let db = Database::open(&path, &FIRST).unwrap();
    assert_eq!(version(&db), 1, "the first script was kept");
    let tables: Vec<String> = block_on(db.query(
        "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name",
        values![],
        |r| r.get(0),
    ))
    .unwrap();
    assert_eq!(tables, ["a"], "nothing of the second script is left");
}

// Files.

#[test]
fn a_file_is_in_wal_mode_with_foreign_keys_and_keeps_what_was_written() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("deeper/app.db");
    let db = Database::open(&path, &SCHEMA).unwrap();
    let mode: String = db
        .call_blocking(|c| Ok(c.query_row("PRAGMA journal_mode", [], |r| r.get(0))?))
        .unwrap();
    assert_eq!(mode, "wal");
    let keys: i64 = db
        .call_blocking(|c| Ok(c.query_row("PRAGMA foreign_keys", [], |r| r.get(0))?))
        .unwrap();
    assert_eq!(keys, 1);
    block_on(db.execute("INSERT INTO items (text) VALUES (?1)", values!["kept"])).unwrap();
    drop(db);

    let db = Database::open(&path, &SCHEMA).unwrap();
    let texts = block_on(db.query("SELECT text FROM items", values![], |r| {
        r.get::<_, String>(0)
    }))
    .unwrap();
    assert_eq!(texts, ["kept"]);
}

#[test]
fn a_path_that_cannot_be_made_is_an_error_naming_it() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("file");
    std::fs::write(&file, "").unwrap();
    // A file where a directory should be.
    let error = Database::open(file.join("app.db"), &SCHEMA).unwrap_err();
    assert!(
        matches!(&error, Error::Io { path, .. } if path == &file),
        "{error}"
    );
}

// Calls.

#[test]
fn calls_run_in_the_order_they_were_made_even_when_nobody_waits_for_them() {
    let db = Database::memory(&SCHEMA).unwrap();
    // Sent and dropped, as a view sends its writes.
    for n in 0..200 {
        drop(db.execute(
            "INSERT INTO items (text) VALUES (?1)",
            values![format!("{n}")],
        ));
    }
    let texts: Vec<String> = block_on(db.query(
        "SELECT text FROM items ORDER BY id",
        values![],
        |r| r.get(0),
    ))
    .unwrap();
    assert_eq!(texts.len(), 200);
    assert!(texts.iter().enumerate().all(|(n, t)| *t == n.to_string()));
}

#[test]
fn handles_are_clones_of_one_connection_and_work_from_many_threads() {
    let db = Database::memory(&SCHEMA).unwrap();
    let threads: Vec<_> = (0..4)
        .map(|t| {
            let db = db.clone();
            std::thread::spawn(move || {
                for n in 0..50 {
                    block_on(db.execute(
                        "INSERT INTO items (text) VALUES (?1)",
                        values![format!("{t}-{n}")],
                    ))
                    .unwrap();
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    let count: i64 = db
        .call_blocking(|c| Ok(c.query_row("SELECT count(*) FROM items", [], |r| r.get(0))?))
        .unwrap();
    assert_eq!(count, 200);
}

#[test]
fn a_transaction_is_committed_whole_or_not_at_all() {
    let db = Database::memory(&SCHEMA).unwrap();
    block_on(db.transaction(|tx| {
        tx.execute("INSERT INTO items (text) VALUES (?1)", params!["one"])?;
        tx.execute("INSERT INTO items (text) VALUES (?1)", params!["two"])?;
        Ok(())
    }))
    .unwrap();

    let failed = block_on(db.transaction(|tx| {
        tx.execute("INSERT INTO items (text) VALUES (?1)", params!["three"])?;
        // A violation of NOT NULL: the whole transaction goes.
        tx.execute("INSERT INTO items (text) VALUES (NULL)", [])?;
        Ok(())
    }));
    assert!(matches!(failed, Err(Error::Sqlite(_))));
    let count: i64 = db
        .call_blocking(|c| Ok(c.query_row("SELECT count(*) FROM items", [], |r| r.get(0))?))
        .unwrap();
    assert_eq!(count, 2, "'three' was undone with the rest");
}

#[test]
fn values_carry_text_numbers_nothing_and_bytes() {
    let db = Database::memory(&Migrations::new(&["CREATE TABLE v (a, b, c, d)"])).unwrap();
    block_on(db.execute(
        "INSERT INTO v VALUES (?1, ?2, ?3, ?4)",
        values!["text", 42_i64, None::<String>, vec![1_u8, 2, 3]],
    ))
    .unwrap();
    let row = block_on(db.query("SELECT a, b, c, d FROM v", values![], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Vec<u8>>(3)?,
        ))
    }))
    .unwrap();
    assert_eq!(row, [("text".to_owned(), 42, None, vec![1, 2, 3])]);
}

#[test]
fn an_error_in_a_statement_is_an_error_and_the_database_goes_on() {
    let db = Database::memory(&SCHEMA).unwrap();
    assert!(matches!(
        block_on(db.execute("NOT SQL", values![])),
        Err(Error::Sqlite(_))
    ));
    block_on(db.execute(
        "INSERT INTO items (text) VALUES (?1)",
        values!["still here"],
    ))
    .unwrap();
}

// A closure that panics.

#[test]
fn a_call_that_panics_closes_the_database_and_nothing_waits_for_it() {
    let db = Database::memory(&SCHEMA).unwrap();
    let other = db.clone();
    // Queued behind the one that panics, before it has run.
    let panicking = db.call(|_| -> odk_db::Result<()> { panic!("on purpose") });
    let queued = other.call(|_| Ok(1));
    assert!(matches!(block_on(panicking), Err(Error::Panicked)));
    assert!(matches!(block_on(queued), Err(Error::Closed)));
    // Every later call, on either handle, says so at once.
    assert!(matches!(db.call_blocking(|_| Ok(())), Err(Error::Closed)));
    assert!(matches!(
        block_on(other.execute("SELECT 1", values![])),
        Err(Error::Closed)
    ));
}
