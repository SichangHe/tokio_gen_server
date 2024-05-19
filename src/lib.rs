#![doc = include_str!("actor_doc.md")]

use std::future::Future;

use anyhow::{Context, Result};
use tokio::{
    select, spawn,
    sync::{
        mpsc::{channel, error::SendError, Receiver, Sender},
        oneshot,
    },
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

pub mod actor;

pub mod prelude {
    pub use super::actor::{Actor, ActorExt, Msg, Ref};
}

#[cfg(test)]
mod tests;
