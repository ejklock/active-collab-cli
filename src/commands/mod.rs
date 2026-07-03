mod comment;
mod mine;
mod presenter;
mod resolve;
mod setup;
mod task;

pub(crate) use comment::comment_core;
pub(crate) use mine::{collect_mine_rows, mine_core, MineOutcome};
pub(crate) use resolve::*;
pub(crate) use setup::{
    setup_add, setup_language, setup_list, setup_remove, setup_test, SetupAddFields,
};
pub(crate) use task::{current_core, get_core, DisplayFlags};
#[cfg(test)]
pub(crate) use task::{do_get_task, load_task};

#[cfg(test)]
use crate::client::ActiveCollabClient;
#[cfg(test)]
use crate::http::Http;
#[cfg(test)]
use crate::i18n::SUPPORTED;
#[cfg(test)]
use crate::store::cache::TaskCache;
#[cfg(test)]
use crate::store::instances::{Instance, InstanceRepository};
#[cfg(test)]
use crate::store::settings::SettingsRepository;

#[cfg(test)]
#[path = "../../tests/unit/commands.rs"]
mod tests;
