// DO NOT modify manually! Generate with `actor2bctor_and_doc.py`.
//! Blocking actor. Mirrors functionalities in `actor` but blocking.
//!
//! Please see the documentation for [`Bctor`].
use super::*;
use std::thread::{spawn, JoinHandle};

// TODO: Error type.

/// A reference to an instance of [`Bctor`],
/// to cast or call messages on it or cancel it.
#[derive(Debug)]
pub struct Ref<L, T, R> {
    pub msg_sender: Sender<Msg<L, T, R>>,
}

/// A reference to an instance of [`Bctor`],
/// to cast or call messages on it or cancel it.
pub type ARef<A> = Ref<<A as Bctor>::L, <A as Bctor>::T, <A as Bctor>::R>;

impl<L, T, R> Ref<L, T, R> {
    /// Cast a message to the bctor and do not expect a reply.
    pub fn cast(&mut self, msg: T) -> Result<(), SendError<Msg<L, T, R>>> {
        self.msg_sender.blocking_send(Msg::Cast(msg))
    }

    /// Same as [`Ref::cast`] but blocking.
    ///
    /// # Panics
    ///
    /// This function panics if called within an asynchronous execution context.
    pub fn blocking_cast(&mut self, msg: T) -> Result<(), SendError<Msg<L, T, R>>> {
        self.msg_sender.blocking_send(Msg::Cast(msg))
    }

    /// Call the bctor and wait for a reply.
    ///
    /// To time out the call, use [`tokio::time::timeout`].
    pub fn call(&mut self, msg: L) -> Result<R>
    where
        L: Send + Sync + 'static,
        T: Send + Sync + 'static,
        R: Send + Sync + 'static,
    {
        // NB: Using the `oneshot` channel here is inexpensive because its only
        // overhead is 1 `Arc` and 5 extra words of allocation.
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.msg_sender
            .blocking_send(Msg::Call(msg, reply_sender))
            .context("Failed to send call to bctor")?;
        reply_receiver
            .blocking_recv()
            .context("Failed to receive bctor's reply")
    }

    /// Same as [`Ref::call`] but blocking.
    ///
    /// # Panics
    ///
    /// This function panics if called within an asynchronous execution context.
    pub fn blocking_call(&mut self, msg: L) -> Result<R>
    where
        L: Send + Sync + 'static,
        T: Send + Sync + 'static,
        R: Send + Sync + 'static,
    {
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.msg_sender
            .blocking_send(Msg::Call(msg, reply_sender))
            .context("Failed to send call to bctor")?;
        reply_receiver
            .blocking_recv()
            .context("Failed to receive bctor's reply")
    }

    /// Call the bctor and let it reply via a given channel sender.
    /// Useful for relaying a call from some other caller.
    pub fn relay_call(
        &mut self,
        msg: L,
        reply_sender: oneshot::Sender<R>,
    ) -> Result<(), SendError<Msg<L, T, R>>> {
        self.msg_sender.blocking_send(Msg::Call(msg, reply_sender))
    }

    /// Cancel the bctor referred to, so it exits.
    pub fn cancel(&mut self) {
        _ = self.msg_sender.blocking_send(Msg::Exit)
    }
}

impl<L, T, R> Clone for Ref<L, T, R> {
    fn clone(&self) -> Self {
        Self {
            msg_sender: self.msg_sender.clone(),
        }
    }
}

/// A message sent to an bctor.
#[derive(Debug)]
pub enum Msg<L, T, R> {
    Exit,
    Call(L, oneshot::Sender<R>),
    Cast(T),
}

/// A message sent to an bctor.
pub type AMsg<A> = Msg<<A as Bctor>::L, <A as Bctor>::T, <A as Bctor>::R>;

#[doc = include_str!("bctor_doc.md")]
pub trait Bctor {
    /// Cal***L*** message type.
    type L;
    /// Cas***T*** message type.
    type T;
    /// ***R***eply message type.
    type R;

    /// Called when the bctor starts.
    fn init(&mut self, _env: &mut ARef<Self>) -> Result<()> {
        Ok(())
    }

    /// Called when the bctor receives a message and does not need to reply.
    fn handle_cast(&mut self, _msg: Self::T, _env: &mut ARef<Self>) -> Result<()> {
        Ok(())
    }

    /// Called when the bctor receives a message and needs to reply.
    ///
    /// Implementations should send the reply using the `reply_sender`,
    /// otherwise the caller may hang.
    fn handle_call(
        &mut self,
        _msg: Self::L,
        _env: &mut ARef<Self>,
        _reply_sender: oneshot::Sender<Self::R>,
    ) -> Result<()> {
        Ok(())
    }

    /// Called before the bctor exits.
    fn before_exit(
        &mut self,
        _run_result: Result<()>,
        _env: &mut ARef<Self>,
        _msg_receiver: &mut Receiver<AMsg<Self>>,
    ) -> Result<()> {
        Ok(())
    }
}

pub type BctorHandle<Msg> = JoinHandle<(Receiver<Msg>, Result<()>)>;

/// Provides convenience methods for spawning [`Bctor`] instances.
pub trait BctorExt {
    type Ref;
    type Msg;

    /// Spawn the bctor in a thread.
    fn spawn(self) -> (BctorHandle<Self::Msg>, Self::Ref);

    /// Same as [`BctorExt::spawn`] but with both ends of the channel given.
    /// Useful for relaying messages or reusing channels.
    fn spawn_with_channel(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
    ) -> (BctorHandle<Self::Msg>, Self::Ref);
}

/// Provides convenience methods for running [`Bctor`] instances.
/// Not intended for users.
pub trait BctorRunExt {
    type Ref;
    type Msg;

    fn handle_call_or_cast(&mut self, msg: Self::Msg, env: &mut Self::Ref) -> Result<()>;

    fn handle_continuously(
        &mut self,
        receiver: &mut Receiver<Self::Msg>,
        env: &mut Self::Ref,
    ) -> Result<()>;

    fn run_and_handle_exit(
        self,
        env: Self::Ref,
        msg_receiver: Receiver<Self::Msg>,
    ) -> (Receiver<Self::Msg>, Result<()>);

    fn run_till_exit(
        &mut self,
        env: &mut Self::Ref,
        msg_receiver: &mut Receiver<Self::Msg>,
    ) -> Result<()>;
}

impl<A> BctorRunExt for A
where
    A: Bctor + Send,
    A::L: Send,
    A::T: Send,
    A::R: Send,
{
    type Ref = ARef<A>;
    type Msg = AMsg<A>;

    fn handle_call_or_cast(&mut self, msg: Self::Msg, env: &mut Self::Ref) -> Result<()> {
        match msg {
            Msg::Exit => unreachable!(
                "Exit signals should be handled before handling `handle_call_or_cast`."
            ),
            Msg::Call(msg, reply_sender) => self.handle_call(msg, env, reply_sender),
            Msg::Cast(msg) => self.handle_cast(msg, env),
        }
    }

    fn handle_continuously(
        &mut self,
        receiver: &mut Receiver<Self::Msg>,
        env: &mut Self::Ref,
    ) -> Result<()> {
        loop {
            let maybe_msg = receiver.blocking_recv();

            let msg = match maybe_msg {
                Some(m) => m,
                None => return Ok(()),
            };

            match msg {
                Msg::Exit => return Ok(()),
                _ => self.handle_call_or_cast(msg, env)?,
            };
        }
    }

    fn run_and_handle_exit(
        mut self,
        mut env: Self::Ref,
        mut msg_receiver: Receiver<Self::Msg>,
    ) -> (Receiver<Self::Msg>, Result<()>) {
        let run_result = self.run_till_exit(&mut env, &mut msg_receiver);
        let exit_result = self.before_exit(run_result, &mut env, &mut msg_receiver);
        (msg_receiver, exit_result)
    }

    fn run_till_exit(
        &mut self,
        env: &mut Self::Ref,
        msg_receiver: &mut Receiver<Self::Msg>,
    ) -> Result<()> {
        self.init(env)?;
        self.handle_continuously(msg_receiver, env)
    }
}

impl<A> BctorExt for A
where
    A: Bctor + Send + 'static,
    A::L: Send,
    A::T: Send,
    A::R: Send,
{
    type Ref = ARef<A>;
    type Msg = AMsg<A>;

    fn spawn(self) -> (BctorHandle<Self::Msg>, Self::Ref) {
        let (msg_sender, msg_receiver) = channel(8);
        self.spawn_with_channel(msg_sender, msg_receiver)
    }

    fn spawn_with_channel(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
    ) -> (BctorHandle<Self::Msg>, Self::Ref) {
        let bctor_ref = Ref { msg_sender };
        let handle = {
            let env = bctor_ref.clone();
            spawn(|| self.run_and_handle_exit(env, msg_receiver))
        };
        (handle, bctor_ref)
    }
}

#[cfg(test)]
mod tests;
