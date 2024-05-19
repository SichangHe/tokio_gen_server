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
pub mod bctor;

pub mod prelude {
    pub use super::{
        actor::{Actor, ActorExt, Msg as ActorMsg, Ref as ActorRef},
        bctor::{Bctor, BctorExt, Msg as BctorMsg, Ref as BctorRef},
    };
}

#[cfg(test)]
mod tests;
