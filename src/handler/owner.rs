use std::{num::NonZeroU64, str::FromStr};

use anyhow::{bail, Context, Result};
use tracing::{info, instrument};

use super::AsyncState;
use crate::{state, AdminAction, AdminsResponse, OwnerResponse};

#[instrument(skip_all)]
pub fn help() -> OwnerResponse {
    info!("received `help` command");
    OwnerResponse::Help
}

#[instrument(skip_all)]
pub async fn admins_list(state: AsyncState) -> OwnerResponse {
    info!("received `admins list` command");
    OwnerResponse::Admins(AdminsResponse::List(
        state.read().await.admins.iter().copied().collect(),
    ))
}

#[instrument(skip_all)]
pub async fn admins_edit(
    state: AsyncState,
    action: &str,
    user_id: Option<NonZeroU64>,
) -> OwnerResponse {
    info!("received `admins` command");

    let res = || async {
        let action = action.parse()?;
        let user_id = user_id.context("no user ID, is the user in the channel?")?;
        update_admins(state, action, user_id).await?;

        Ok(action.into())
    };

    OwnerResponse::Admins(AdminsResponse::Edit(res().await))
}

#[derive(Clone, Copy, Debug)]
enum Action {
    Add,
    Remove,
}

impl FromStr for Action {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "add" => Self::Add,
            "remove" => Self::Remove,
            s => bail!("unknown action `{s}`"),
        })
    }
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
