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

pub use actor::{Actor, ActorExt, Msg, Ref};

pub mod actor;
#[cfg(test)]
mod tests;
