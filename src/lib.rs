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
    #[doc(inline)]
    pub use super::{
        actor::{AMsg as ActorMsg, ARef as ActorRef, Actor, ActorExt},
        bctor::{AMsg as BctorMsg, ARef as BctorRef, Bctor, BctorExt},
    };
}

#[cfg(test)]
mod tests;
