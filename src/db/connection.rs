use std::ops::{Deref, DerefMut};

use anyhow::{Context, Result};
use sqlx::{
    SqlitePool,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};

use crate::dirs::DIRS;

static MIGRATOR: Migrator = sqlx::migrate!();

#[derive(Clone)]
pub struct Connection(SqlitePool);

impl Connection {
    pub async fn new() -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(DIRS.database_file())
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .min_connections(1)
            .connect_with(options)
            .await
            .with_context(|| format!("failed opening database at {:?}", DIRS.database_file()))?;

        MIGRATOR
            .run(&pool)
            .await
            .context("failed running migrations")?;

        Ok(Self(pool))
    }

    #[cfg(test)]
    pub async fn in_memory() -> Result<Self> {
        use sqlx::ConnectOptions;
        use tracing::log::LevelFilter;

        let pool = SqlitePoolOptions::new()
            .min_connections(1)
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .foreign_keys(true)
                    .log_statements(LevelFilter::Info),
            )
            .await?;

        MIGRATOR.run(&pool).await?;

        Ok(Self(pool))
    }

    pub async fn close(self) {
        self.0.close().await;
    }
}

impl Deref for Connection {
    type Target = SqlitePool;

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
    use sqlx::SqlitePool;

    use super::MIGRATOR;

    #[tokio::test]
    async fn run_migrations() {
        let conn = SqlitePool::connect(":memory:").await.unwrap();
        MIGRATOR.run(&conn).await.unwrap();
        MIGRATOR.undo(&conn, 0).await.unwrap();
    }
}
