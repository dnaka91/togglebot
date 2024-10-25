use std::num::NonZeroU64;

use anyhow::Result;
use tracing::{info, instrument};

use super::AsyncState;
use crate::{
    api::response::{self, AdminAction},
    state,
};

#[instrument(skip_all)]
pub fn help() -> response::Owner {
    info!("received `help` command");
    response::Owner::Help
}

#[instrument(skip_all)]
pub async fn admins_list(state: AsyncState) -> response::Owner {
    info!("received `admins list` command");
    response::Owner::Admins(response::Admins::List(
        state.read().await.admins.iter().copied().collect(),
    ))
}

#[instrument(skip_all)]
pub async fn admins_edit(
    state: AsyncState,
    action: Action,
    user_id: NonZeroU64,
) -> response::Owner {
    info!("received `admins` command");

    let res = || async {
        update_admins(state, action, user_id).await?;
        Ok(action.into())
    };

    response::Owner::Admins(response::Admins::Edit(res().await))
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Action {
    Add,
    Remove,
}

#[instrument(skip(state))]
async fn update_admins(state: AsyncState, action: Action, user_id: NonZeroU64) -> Result<()> {
    let mut state = state.write().await;

    match action {
        Action::Add => {
            state.admins.insert(user_id);
        }
        Action::Remove => {
            state.admins.remove(&user_id);
        }
    }

    state::save(&state).await?;

    Ok(())
}

impl From<Action> for AdminAction {
    fn from(value: Action) -> Self {
        match value {
            Action::Add => Self::Added,
            Action::Remove => Self::Removed,
        }
    }
}
