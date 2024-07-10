#![doc = include_str!("actor_doc.md")]
// Those are copied from the tests anyway.
#![allow(clippy::test_attr_in_doctest)]

use std::future::Future;

use anyhow::{Context, Result};
use tokio::{
    select, spawn,
    sync::{
        mpsc::{channel, error::SendError, Receiver, Sender},
        oneshot,
    },
    task::{AbortHandle, JoinHandle, JoinSet},
};
use tokio_util::sync::CancellationToken;

pub mod actor;
pub mod bctor;

pub mod prelude {
    #[doc(inline)]
    pub use super::{
        actor::{Actor, ActorEnv, ActorExt, ActorMsg, ActorRef, ActorRunResult},
        bctor::{Bctor, BctorEnv, BctorExt, BctorMsg, BctorRef, BctorRunResult},
    };
}

#[cfg(test)]
mod tests;
