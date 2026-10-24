//! The schema, as a list of scripts applied in order.

use rusqlite::Connection;

use crate::{Error, Result};

/// An application's schema: SQL scripts that are only ever added to the end, applied in order,
/// each in a transaction. The database records how many have been applied in `PRAGMA
/// user_version`, so opening it again applies only the new ones.
///
/// A script that was applied is never edited: a change to a table is a new script. A database
/// whose version is past the end of the list was written by a newer application, and is
/// refused with [`Error::TooNew`] and left alone.
#[derive(Debug, Clone, Copy)]
pub struct Migrations {
    scripts: &'static [&'static str],
}

impl Migrations {
    pub const fn new(scripts: &'static [&'static str]) -> Self {
        Self { scripts }
    }

    /// How many scripts there are.
    pub const fn len(&self) -> usize {
        self.scripts.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.scripts.is_empty()
    }

    /// Brings `connection` up to date. A script that fails is undone with its own transaction,
    /// and the scripts before it stay applied.
    pub(crate) fn apply(&self, connection: &mut Connection) -> Result<()> {
        let applied: u32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        let known = self.scripts.len() as u32;
        if applied > known {
            return Err(Error::TooNew {
                found: applied,
                known,
            });
        }
        for (index, script) in self.scripts.iter().enumerate().skip(applied as usize) {
            let tx = connection.transaction()?;
            tx.execute_batch(script)?;
            // A pragma cannot take a parameter; the number is ours.
            tx.execute_batch(&format!("PRAGMA user_version = {}", index + 1))?;
            tx.commit()?;
        }
        Ok(())
    }
}
