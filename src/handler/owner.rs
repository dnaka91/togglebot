use anyhow::Result;
use tracing::{info, instrument};

use crate::{
    api::{
        AdminId,
        response::{self, AdminAction},
    },
    state::State,
};

#[instrument(skip_all)]
pub fn help() -> response::Owner {
    info!("received `help` command");
    response::Owner::Help
}

#[instrument(skip_all)]
pub async fn admins_list(state: &State) -> Result<response::Owner> {
    info!("received `admins list` command");
    let list = state.list_admins().await?;

    Ok(response::Owner::Admins(response::Admins::List(list)))
}

#[instrument(skip_all)]
pub async fn admins_edit(state: &State, action: Action, id: AdminId) -> Result<response::Owner> {
    info!("received `admins` command");

    Ok(response::Owner::Admins(response::Admins::Edit(
        update_admins(state, action, id).await,
    )))
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Action {
    Add,
    Remove,
}

#[instrument(skip(state))]
async fn update_admins(state: &State, action: Action, id: AdminId) -> Result<AdminAction> {
    match action {
        Action::Add => {
            state.add_admin(id).await?;
        }
        Action::Remove => {
            state.remove_admin(id).await?;
        }
    }

    Ok(action.into())
}

impl From<Action> for AdminAction {
    fn from(value: Action) -> Self {
        match value {
            Action::Add => Self::Added,
            Action::Remove => Self::Removed,
        }
    }
}
