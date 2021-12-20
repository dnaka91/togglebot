use std::{num::NonZeroU64, str::FromStr};

use anyhow::{bail, Result};
use tracing::info;

use super::AsyncState;
use crate::{settings, AdminAction, AdminsResponse, OwnerResponse};

pub fn help() -> OwnerResponse {
    info!("owner: received `help` command");
    OwnerResponse::Help
}

pub async fn admins_list(state: AsyncState) -> OwnerResponse {
    info!("owner: received `admins list` command");
    OwnerResponse::Admins(AdminsResponse::List(
        state.read().await.admins.iter().copied().collect(),
    ))
}

pub async fn admins_edit(state: AsyncState, action: &str, user_id: NonZeroU64) -> OwnerResponse {
    info!("owner: received `admins` command");

    let res = || async {
        let action = action.parse()?;
        update_admins(state, action, user_id).await?;

        Ok(action.into())
    };

    OwnerResponse::Admins(AdminsResponse::Edit(res().await))
}

#[derive(Clone, Copy)]
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
            s => bail!("unknown action `{}`", s),
        })
    }
}

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

    settings::save_state(&state).await?;

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
