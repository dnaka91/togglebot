//! State management and load/save logic for it.
#![expect(clippy::cast_possible_wrap)]

use std::num::NonZero;

use anyhow::Result;

pub use self::migrate::run as migrate;
use crate::{
    api::{AdminId, Source},
    db::connection::Connection,
};

/// Main state structure holding all dynamic (runtime changeable) settings.
pub struct State(Connection);

impl State {
    #[must_use]
    pub fn new(conn: Connection) -> Self {
        Self(conn)
    }

    #[cfg(test)]
    pub async fn in_memory() -> Result<Self> {
        Connection::in_memory().await.map(Self)
    }

    pub async fn add_admin(&self, id: AdminId) -> Result<()> {
        let id = id.get() as i64;
        sqlx::query_file!("queries/admins/add.sql", id)
            .execute(&*self.0)
            .await?;
        Ok(())
    }

    pub async fn remove_admin(&self, id: AdminId) -> Result<()> {
        let id = id.get() as i64;
        sqlx::query_file!("queries/admins/remove.sql", id)
            .execute(&*self.0)
            .await?;
        Ok(())
    }

    pub async fn is_admin(&self, id: AdminId) -> Result<bool> {
        let id = id.get() as i64;
        let res = sqlx::query_file_scalar!("queries/admins/exists.sql", id)
            .fetch_one(&*self.0)
            .await?;
        Ok(res > 0)
    }

    pub async fn list_admins(&self) -> Result<Vec<AdminId>> {
        sqlx::query_file!("queries/admins/list.sql")
            .map(|row| AdminId::from(row.id))
            .fetch_all(&*self.0)
            .await
            .map_err(Into::into)
    }

    pub async fn add_custom_command(
        &self,
        source: Source,
        name: &str,
        content: &str,
    ) -> Result<()> {
        sqlx::query_file!("queries/custom_cmds/add.sql", source, name, content)
            .execute(&*self.0)
            .await?;
        Ok(())
    }

    pub async fn remove_custom_command(&self, source: Source, name: &str) -> Result<()> {
        sqlx::query_file!("queries/custom_cmds/remove.sql", source, name)
            .execute(&*self.0)
            .await?;
        Ok(())
    }

    pub async fn remove_custom_command_by_name(&self, name: &str) -> Result<()> {
        sqlx::query_file!("queries/custom_cmds/remove_name.sql", name)
            .execute(&*self.0)
            .await?;
        Ok(())
    }

    pub async fn get_custom_command(&self, source: Source, name: &str) -> Result<Option<String>> {
        sqlx::query_file_scalar!("queries/custom_cmds/get.sql", source, name)
            .fetch_optional(&*self.0)
            .await
            .map_err(Into::into)
    }

    pub async fn list_custom_commands(&self) -> Result<Vec<(String, Source)>> {
        sqlx::query_file!("queries/custom_cmds/list.sql")
            .map(|row| (row.name, row.source))
            .fetch_all(&*self.0)
            .await
            .map_err(Into::into)
    }

    pub async fn list_custom_command_names(&self, source: Source) -> Result<Vec<String>> {
        sqlx::query_file_scalar!("queries/custom_cmds/list_names.sql", source)
            .fetch_all(&*self.0)
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
impl From<Connection> for State {
    fn from(value: Connection) -> Self {
        Self(value)
    }
}

mod migrate {
    use std::{
        collections::{HashMap, HashSet},
        io::ErrorKind,
        num::NonZeroU64,
    };

    use anyhow::{Context, Result};
    use serde::Deserialize;
    use tokio::fs;

    use super::Connection;
    use crate::dirs::DIRS;

    #[derive(Deserialize)]
    struct State {
        #[serde(default)]
        custom_commands: HashMap<String, HashMap<Source, String>>,
        #[serde(default)]
        admins: HashSet<NonZeroU64>,
    }

    #[derive(Eq, Hash, PartialEq, Deserialize)]
    enum Source {
        Discord,
        Twitch,
    }

    async fn load() -> Result<Option<State>> {
        let state = match fs::read(DIRS.state_file()).await {
            Ok(buf) => buf,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e).context("failed reading state file"),
        };

        serde_json::from_slice(&state)
            .context("failed parsing state data")
            .map(Some)
    }

    pub async fn run(conn: &Connection) -> Result<()> {
        let Some(state) = load().await? else {
            return Ok(());
        };

        let mut tx = conn.begin().await?;

        for admin in state.admins {
            let id = admin.get() as i64;
            sqlx::query_file!("queries/admins/add.sql", id)
                .execute(&mut *tx)
                .await?;
        }

        for (name, contents) in state.custom_commands {
            for (source, content) in contents {
                let source = match source {
                    Source::Discord => crate::api::Source::Discord,
                    Source::Twitch => crate::api::Source::Twitch,
                };

                sqlx::query_file!("queries/custom_cmds/add.sql", source, name, content)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        tx.commit().await?;

        fs::remove_file(DIRS.state_file())
            .await
            .context("failed deleting obsolete state file")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn admin_roundtrip() {
        let state = State::in_memory().await.unwrap();
        let id = AdminId::new(1).unwrap();

        assert!(!state.is_admin(id).await.unwrap());

        state.add_admin(id).await.unwrap();
        assert!(state.is_admin(id).await.unwrap());
        assert_eq!([id], state.list_admins().await.unwrap().as_slice());

        state.remove_admin(id).await.unwrap();
        assert!(!state.is_admin(id).await.unwrap());
    }

    #[tokio::test]
    async fn commands_roundtrip() {
        let state = State::in_memory().await.unwrap();

        for source in [Source::Discord, Source::Twitch] {
            assert!(
                state
                    .list_custom_command_names(source)
                    .await
                    .unwrap()
                    .is_empty()
            );
        }

        state
            .add_custom_command(Source::Discord, "hi", "hello")
            .await
            .unwrap();
        assert_eq!(
            Some("hello".to_owned()),
            state
                .get_custom_command(Source::Discord, "hi")
                .await
                .unwrap()
        );

        state
            .remove_custom_command(Source::Discord, "hi")
            .await
            .unwrap();
        assert_eq!(
            None,
            state
                .get_custom_command(Source::Discord, "hi")
                .await
                .unwrap()
        );

        state
            .add_custom_command(Source::Twitch, "hi", "hello")
            .await
            .unwrap();
        assert_eq!(
            Some("hello".to_owned()),
            state
                .get_custom_command(Source::Twitch, "hi")
                .await
                .unwrap()
        );

        state.remove_custom_command_by_name("hi").await.unwrap();
        assert_eq!(
            None,
            state
                .get_custom_command(Source::Twitch, "hi")
                .await
                .unwrap()
        );

        assert!(state.list_custom_commands().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn overwrite_command() {
        let state = State::in_memory().await.unwrap();

        state
            .add_custom_command(Source::Discord, "test", "one")
            .await
            .unwrap();
        state
            .add_custom_command(Source::Discord, "test", "two")
            .await
            .unwrap();

        let cmd = state
            .get_custom_command(Source::Discord, "test")
            .await
            .unwrap();
        assert_eq!(Some("two"), cmd.as_deref());
    }
}
