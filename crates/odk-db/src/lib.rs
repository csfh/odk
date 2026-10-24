//! A SQLite database for an application's own data (ADR 0049).
//!
//! A [`Database`] is a cheap, cloneable handle to one connection that lives on a thread of its
//! own. SQL runs there, in the order it was asked for, so a window never waits for a disk:
//! [`Database::call`] returns a future that any executor can run, and the blocking
//! [`Database::call_blocking`] is for a start-up read or a test. What an application writes is a
//! closure over [`rusqlite::Connection`], which is the escape hatch (ADR 0004): everything
//! `rusqlite` does can be done, and [`Database::execute`], [`Database::query`] and
//! [`Database::transaction`] only write the common cases once.
//!
//! The schema is a [`Migrations`] list the application owns; the database says which of it has
//! been applied (`PRAGMA user_version`), and one that is newer than the list is refused.
//!
//! Nothing here depends on GPUI. A view keeps what it draws in memory and sends writes with
//! [`Database::call`] without waiting for them, so a test of the view never waits for a disk
//! either.

mod migrations;

use std::fmt;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

pub use migrations::Migrations;
/// The SQLite binding, for what the helpers do not cover (ADR 0049).
pub use rusqlite;
pub use rusqlite::types::Value;
pub use rusqlite::{Connection, Row, Transaction, params};

/// What went wrong with a database.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The file was written by a newer version of the application than this one: its schema is
    /// at version `found` and this application knows `known`. It is left as it is.
    #[error("the database is at version {found}, newer than this application's {known}")]
    TooNew { found: u32, known: u32 },
    /// A closure that was run on the database panicked, so it has been closed.
    #[error("a call on the database panicked, so it was closed")]
    Panicked,
    /// The database has been closed: a call panicked earlier, or the thread is gone.
    #[error("the database is closed")]
    Closed,
}

pub type Result<T> = std::result::Result<T, Error>;

/// A job for the connection's thread. It says whether the thread goes on: a job that panicked
/// ends it, and with it the connection.
type Job = Box<dyn FnOnce(&mut Connection) -> bool + Send>;

/// `f` as a job that sends its result, or [`Error::Panicked`], down `reply`.
fn job<T, F>(f: F, reply: async_channel::Sender<Result<T>>) -> Job
where
    T: Send + 'static,
    F: FnOnce(&mut Connection) -> Result<T> + Send + 'static,
{
    Box::new(move |connection| {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(connection))) {
            Ok(result) => {
                let _ = reply.send_blocking(result);
                true
            }
            Err(_) => {
                let _ = reply.send_blocking(Err(Error::Panicked));
                false
            }
        }
    })
}

/// A handle to one database. Clones share the connection; the connection closes, and a WAL is
/// folded back into the file, when the last handle is dropped.
#[derive(Clone)]
pub struct Database {
    jobs: Arc<Mutex<mpsc::Sender<Job>>>,
}

impl fmt::Debug for Database {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Database").finish_non_exhaustive()
    }
}

impl Database {
    /// The database in the file `path`, made if it is not there, with its directory, and brought
    /// up to date with `migrations`. Blocks while it does, which is for a start-up. The file is
    /// opened in WAL mode with foreign keys on and a busy timeout of five seconds.
    pub fn open(path: impl AsRef<Path>, migrations: &Migrations) -> Result<Self> {
        let path = path.as_ref();
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|source| Error::Io {
                path: dir.to_owned(),
                source,
            })?;
        }
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;",
        )?;
        Self::start(connection, migrations)
    }

    /// A database that lives in memory and is gone when its last handle is: for tests, and for
    /// what an application keeps only while it runs.
    pub fn memory(migrations: &Migrations) -> Result<Self> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        Self::start(connection, migrations)
    }

    fn start(mut connection: Connection, migrations: &Migrations) -> Result<Self> {
        migrations.apply(&mut connection)?;
        let (jobs, work) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("odk-db".into())
            .spawn(move || {
                // The thread ends when the last handle is dropped, or after a job that panicked:
                // the connection is closed with it either way, and what was still queued is
                // dropped with the channel, which answers each of its callers `Closed`.
                while let Ok(job) = work.recv() {
                    if !job(&mut connection) {
                        break;
                    }
                }
            })
            .map_err(|source| Error::Io {
                path: PathBuf::from("odk-db thread"),
                source,
            })?;
        Ok(Self {
            jobs: Arc::new(Mutex::new(jobs)),
        })
    }

    /// Runs `f` on the connection's thread, after everything asked for before it, and gives its
    /// result. The future needs no particular executor and may be dropped: the call still runs,
    /// which is how a write is sent without waiting for it.
    pub fn call<T, F>(&self, f: F) -> impl Future<Output = Result<T>> + Send + use<T, F>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T> + Send + 'static,
    {
        let (reply, answer) = async_channel::bounded::<Result<T>>(1);
        let sent = self.send(job(f, reply));
        async move {
            sent?;
            answer.recv().await.unwrap_or(Err(Error::Closed))
        }
    }

    /// [`Self::call`], waiting for the result. Not for the UI thread but at a start-up, and in
    /// tests.
    pub fn call_blocking<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T> + Send + 'static,
    {
        let (reply, answer) = async_channel::bounded::<Result<T>>(1);
        self.send(job(f, reply))?;
        answer.recv_blocking().unwrap_or(Err(Error::Closed))
    }

    fn send(&self, job: Job) -> Result<()> {
        // A sender is not `Sync`, so handles share it behind a lock held for one send.
        let jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        jobs.send(job).map_err(|_| Error::Closed)
    }

    /// Runs one statement with `params` and gives the rows it changed.
    pub fn execute(
        &self,
        sql: impl Into<String>,
        params: Vec<Value>,
    ) -> impl Future<Output = Result<usize>> + Send {
        let sql = sql.into();
        self.call(move |c| Ok(c.execute(&sql, rusqlite::params_from_iter(params))?))
    }

    /// Runs a query with `params` and maps each row with `map`.
    pub fn query<T, F>(
        &self,
        sql: impl Into<String>,
        params: Vec<Value>,
        map: F,
    ) -> impl Future<Output = Result<Vec<T>>> + Send
    where
        T: Send + 'static,
        F: FnMut(&Row<'_>) -> rusqlite::Result<T> + Send + 'static,
    {
        let sql = sql.into();
        self.call(move |c| {
            let mut statement = c.prepare(&sql)?;
            let rows = statement.query_map(rusqlite::params_from_iter(params), map)?;
            Ok(rows.collect::<rusqlite::Result<Vec<T>>>()?)
        })
    }

    /// Runs `f` in a transaction that is committed when it returns `Ok` and undone when it
    /// returns an error, or panics.
    pub fn transaction<T, F>(&self, f: F) -> impl Future<Output = Result<T>> + Send
    where
        T: Send + 'static,
        F: FnOnce(&Transaction<'_>) -> Result<T> + Send + 'static,
    {
        self.call(move |c| {
            let tx = c.transaction()?;
            let out = f(&tx)?;
            tx.commit()?;
            Ok(out)
        })
    }
}

/// What can be a statement's parameter: text, numbers, bytes, nothing (`None`), and a
/// [`Value`] itself.
pub trait IntoValue {
    fn into_value(self) -> Value;
}

macro_rules! into_value {
    ($($type:ty => |$v:ident| $value:expr),* $(,)?) => {
        $(impl IntoValue for $type {
            fn into_value(self) -> Value {
                let $v = self;
                $value
            }
        })*
    };
}

into_value! {
    Value => |v| v,
    bool => |v| Value::Integer(i64::from(v)),
    i32 => |v| Value::Integer(i64::from(v)),
    i64 => |v| Value::Integer(v),
    u32 => |v| Value::Integer(i64::from(v)),
    f64 => |v| Value::Real(v),
    &str => |v| Value::Text(v.to_owned()),
    String => |v| Value::Text(v),
    &String => |v| Value::Text(v.clone()),
    Vec<u8> => |v| Value::Blob(v),
    &[u8] => |v| Value::Blob(v.to_vec()),
}

impl<T: IntoValue> IntoValue for Option<T> {
    fn into_value(self) -> Value {
        self.map_or(Value::Null, IntoValue::into_value)
    }
}

/// The values of a statement's parameters: `values![name, 3, None::<String>]`.
#[macro_export]
macro_rules! values {
    ($($value:expr),* $(,)?) => {
        ::std::vec![$($crate::IntoValue::into_value($value)),*]
    };
}

/// Where an application keeps `file`: under `app` in `$XDG_DATA_HOME`, or `~/.local/share`.
/// Inside the sandbox `$HOME` is the application's own data directory, so this is private to it
/// and is kept between runs (`docs/spec/sandbox-baseline.md`).
pub fn data_file(app: &str, file: &str) -> Option<PathBuf> {
    data_file_in(|name| std::env::var_os(name), app, file)
}

/// [`data_file`] for the environment `var` gives, so that it can be tried without the process's
/// own.
pub fn data_file_in(
    var: impl Fn(&str) -> Option<std::ffi::OsString>,
    app: &str,
    file: &str,
) -> Option<PathBuf> {
    let var = |name: &str| var(name).filter(|v| !v.is_empty());
    let data = match var("XDG_DATA_HOME") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(var("HOME")?).join(".local/share"),
    };
    Some(data.join(app).join(file))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::data_file_in;

    #[test]
    fn a_file_is_kept_under_the_data_directory_the_environment_names() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| OsString::from(v))
            }
        };
        assert_eq!(
            data_file_in(
                env(&[("XDG_DATA_HOME", "/d"), ("HOME", "/h")]),
                "todo",
                "t.db"
            ),
            Some(PathBuf::from("/d/todo/t.db"))
        );
        assert_eq!(
            data_file_in(env(&[("HOME", "/h")]), "todo", "t.db"),
            Some(PathBuf::from("/h/.local/share/todo/t.db"))
        );
        assert_eq!(
            data_file_in(
                env(&[("XDG_DATA_HOME", ""), ("HOME", "/h")]),
                "todo",
                "t.db"
            ),
            Some(PathBuf::from("/h/.local/share/todo/t.db")),
            "empty is not set"
        );
        assert_eq!(data_file_in(env(&[]), "todo", "t.db"), None);
    }
}
