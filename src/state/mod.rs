//! State management and load/save logic for it.

use std::sync::Arc;

use anyhow::Result;

pub use self::migrate::run as migrate;
use crate::{
    api::{AdminId, Source},
    db::{self, connection::Connection},
};

/// Main state structure holding all dynamic (runtime changeable) settings.
pub struct State(Arc<Connection>);

impl State {
    pub fn new(conn: Connection) -> Self {
        Self(conn.into())
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        Connection::in_memory().map(Arc::new).map(Self)
    }

    pub fn add_admin(&self, id: AdminId) -> Result<()> {
        db::exec(&self.0, include_str!("../../queries/admins/add.sql"), id)
    }

    pub fn remove_admin(&self, id: AdminId) -> Result<()> {
        db::exec(&self.0, include_str!("../../queries/admins/remove.sql"), id)
    }

    pub fn is_admin(&self, id: AdminId) -> Result<bool> {
        db::query_one(&self.0, include_str!("../../queries/admins/exists.sql"), id)
            .map(|exists| exists.unwrap_or(false))
    }

    pub fn list_admins(&self) -> Result<Vec<AdminId>> {
        db::query_vec(
            &self.0,
            include_str!("../../queries/admins/list.sql"),
            db::NO_PARAMS,
        )
    }

    pub fn add_custom_command(&self, source: Source, name: &str, content: &str) -> Result<()> {
        db::exec(
            &self.0,
            include_str!("../../queries/custom_cmds/add.sql"),
            (source, name, content),
        )
    }

    pub fn remove_custom_command(&self, source: Source, name: &str) -> Result<()> {
        db::exec(
            &self.0,
            include_str!("../../queries/custom_cmds/remove.sql"),
            (source, name),
        )
    }

    pub fn remove_custom_command_by_name(&self, name: &str) -> Result<()> {
        db::exec(
            &self.0,
            include_str!("../../queries/custom_cmds/remove_name.sql"),
            name,
        )
    }

    pub fn get_custom_command(&self, source: Source, name: &str) -> Result<Option<String>> {
        db::query_one(
            &self.0,
            include_str!("../../queries/custom_cmds/get.sql"),
            (source, name),
        )
    }

    pub fn list_custom_commands(&self) -> Result<Vec<(String, Source)>> {
        db::query_vec(
            &self.0,
            include_str!("../../queries/custom_cmds/list.sql"),
            db::NO_PARAMS,
        )
    }

    pub fn list_custom_command_names(&self, source: Source) -> Result<Vec<String>> {
        db::query_vec(
            &self.0,
            include_str!("../../queries/custom_cmds/list_names.sql"),
            source,
        )
    }
}

mod migrate {
    use std::{
        collections::{HashMap, HashSet},
        fs,
        io::ErrorKind,
        num::NonZeroU64,
    };

    use anyhow::{Context, Result};
    use serde::Deserialize;

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

    fn load() -> Result<Option<State>> {
        let state = match fs::read(DIRS.state_file()) {
            Ok(buf) => buf,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e).context("failed reading state file"),
        };

        serde_json::from_slice(&state)
            .context("failed parsing state data")
            .map(Some)
    }

    pub fn run(conn: &mut Connection) -> Result<()> {
        let Some(state) = load()? else { return Ok(()) };

        let tx = conn.transaction()?;
        let mut stmt = tx.prepare(include_str!("../../queries/admins/add.sql"))?;

        for admin in state.admins {
            stmt.execute(serde_rusqlite::to_params(admin)?)?;
        }

        drop(stmt);
        let mut stmt = tx.prepare(include_str!("../../queries/custom_cmds/add.sql"))?;

        for (name, contents) in state.custom_commands {
            for (source, content) in contents {
                let source = match source {
                    Source::Discord => crate::api::Source::Discord,
                    Source::Twitch => crate::api::Source::Twitch,
                };

                stmt.execute(serde_rusqlite::to_params((source, &name, content))?)?;
            }
        }

        drop(stmt);
        tx.commit()?;

        fs::remove_file(DIRS.state_file()).context("failed deleting obsolete state file")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_roundtrip() {
        let state = State::in_memory().unwrap();
        let id = AdminId::new(1).unwrap();

        assert!(!state.is_admin(id).unwrap());

        state.add_admin(id).unwrap();
        assert!(state.is_admin(id).unwrap());
        assert_eq!([id], state.list_admins().unwrap().as_slice());

        state.remove_admin(id).unwrap();
        assert!(!state.is_admin(id).unwrap());
    }

    #[test]
    fn commands_roundtrip() {
        let state = State::in_memory().unwrap();

        for source in [Source::Discord, Source::Twitch] {
            assert!(state.list_custom_command_names(source).unwrap().is_empty());
        }

        state
            .add_custom_command(Source::Discord, "hi", "hello")
            .unwrap();
        assert_eq!(
            Some("hello".to_owned()),
            state.get_custom_command(Source::Discord, "hi").unwrap()
        );

        state.remove_custom_command(Source::Discord, "hi").unwrap();
        assert_eq!(
            None,
            state.get_custom_command(Source::Discord, "hi").unwrap()
        );

        state
            .add_custom_command(Source::Twitch, "hi", "hello")
            .unwrap();
        assert_eq!(
            Some("hello".to_owned()),
            state.get_custom_command(Source::Twitch, "hi").unwrap()
        );

        state.remove_custom_command_by_name("hi").unwrap();
        assert_eq!(
            None,
            state.get_custom_command(Source::Twitch, "hi").unwrap()
        );

        assert!(state.list_custom_commands().unwrap().is_empty());
    }
}
