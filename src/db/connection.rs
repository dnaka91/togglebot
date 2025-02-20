use std::{
    ops::{Deref, DerefMut},
    sync::LazyLock,
};

use anyhow::{Context, Result};
use include_dir::{Dir, include_dir};
use rusqlite_migration::Migrations;

use crate::dirs::DIRS;

static MIGRATIONS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/migrations");
static MIGRATIONS: LazyLock<Migrations<'_>> =
    LazyLock::new(|| Migrations::from_directory(&MIGRATIONS_DIR).unwrap());

pub struct Connection(rusqlite::Connection);

impl Connection {
    pub fn new() -> Result<Self> {
        let mut conn = rusqlite::Connection::open(DIRS.database_file())
            .with_context(|| format!("failed opening database at {:?}", DIRS.database_file()))?;

        MIGRATIONS
            .to_latest(&mut conn)
            .context("failed running migrations")?;

        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        Ok(Self(conn))
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        let mut conn = rusqlite::Connection::open_in_memory()?;

        MIGRATIONS.to_latest(&mut conn)?;

        conn.pragma_update(None, "foreign_keys", "ON")?;

        Ok(Self(conn))
    }
}

impl Deref for Connection {
    type Target = rusqlite::Connection;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Connection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    use super::MIGRATIONS;

    #[test]
    fn run_migrations() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();
        MIGRATIONS.to_version(&mut conn, 0).unwrap();
    }
}
