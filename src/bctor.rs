// DO NOT modify manually! Generate with `actor2bctor_and_doc.py`.
//! Blocking aCTOR. Mirrors functionalities in `actor` but blocking.
//!
//! Unlike Actors, Bctors are spawn using [`spawn`] from [`std`] and
//! cannot be cancelled during the handling of each message.
//! Bctors are supposed to be long-lived.
//!
//! Please see the documentation for [`Bctor`].
use super::*;
use std::thread::{spawn, JoinHandle};

/// The result when the [`Bctor`] exits.
pub struct BctorRunResult<A: Bctor> {
    /// The [`Bctor`] itself.
    pub bctor: A,
    /// The [`Bctor`]'s environment.
    pub env: BctorEnv<A>,
    /// The result of the [`Bctor`] exiting.
    pub exit_result: Result<()>,
}

/// The environment the [`Bctor`] runs in.
#[derive(Debug)]
pub struct Env<Call, Cast, Reply> {
    /// The reference to the [`Bctor`] itself.
    pub ref_: Ref<Call, Cast, Reply>,
    /// The [`Bctor`]'s message receiver.
    pub msg_receiver: Receiver<Msg<Call, Cast, Reply>>,
}

/// The environment the [`Bctor`] runs in.
pub type BctorEnv<A> = Env<<A as Bctor>::Call, <A as Bctor>::Cast, <A as Bctor>::Reply>;

/// A reference to an instance of [`Bctor`],
/// to cast or call messages on it or cancel it.
#[derive(Debug)]
pub struct Ref<Call, Cast, Reply> {
    /// A message sender to send messages to the [`Bctor`].
    pub msg_sender: Sender<Msg<Call, Cast, Reply>>,
    /// A token to cancel the [`Bctor`].
    pub cancellation_token: CancellationToken,
}

/// A reference to an instance of [`Bctor`],
/// to cast or call messages on it or cancel it.
pub type BctorRef<A> = Ref<<A as Bctor>::Call, <A as Bctor>::Cast, <A as Bctor>::Reply>;

impl<Call, Cast, Reply> Ref<Call, Cast, Reply> {
    /// Cast a message to the bctor and do not expect a reply.
    pub fn cast(&self, msg: Cast) -> Result<(), SendError<Msg<Call, Cast, Reply>>> {
        self.msg_sender.blocking_send(Msg::Cast(msg))
    }

    /// Same as [`Ref::cast`] but blocking.
    ///
    /// # Panics
    ///
    /// This function panics if called within an asynchronous execution context.
    pub fn blocking_cast(&self, msg: Cast) -> Result<(), SendError<Msg<Call, Cast, Reply>>> {
        self.msg_sender.blocking_send(Msg::Cast(msg))
    }

    /// Call the bctor and wait for a reply.
    ///
    /// To time out the call, use [`tokio::time::timeout`].
    pub fn call(&self, msg: Call) -> Result<Reply>
    where
        Msg<Call, Cast, Reply>: Send + Sync + 'static,
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
    pub fn blocking_call(&self, msg: Call) -> Result<Reply>
    where
        Msg<Call, Cast, Reply>: Send + Sync + 'static,
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
        &self,
        msg: Call,
        reply_sender: oneshot::Sender<Reply>,
    ) -> Result<(), SendError<Msg<Call, Cast, Reply>>> {
        self.msg_sender.blocking_send(Msg::Call(msg, reply_sender))
    }

    /// Cancel the bctor referred to, so it exits.
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
        _ = self.msg_sender.blocking_send(Msg::Exit)
    }
}

impl<Call, Cast, Reply> Clone for Ref<Call, Cast, Reply> {
    fn clone(&self) -> Self {
        Self {
            msg_sender: self.msg_sender.clone(),
            cancellation_token: self.cancellation_token.clone(),
        }
    }
}

/// A message sent to an bctor.
#[derive(Debug)]
pub enum Msg<Call, Cast, Reply> {
    Exit,
    Call(Call, oneshot::Sender<Reply>),
    Cast(Cast),
}

/// A message sent to an bctor.
pub type BctorMsg<A> = Msg<<A as Bctor>::Call, <A as Bctor>::Cast, <A as Bctor>::Reply>;

#[doc = include_str!("bctor_doc.md")]
pub trait Bctor {
    /// "Call" message for requests that anticipate replies.
    type Call;
    /// "Cast" message for fire-and-forget requests.
    type Cast;
    /// "Reply" message for replying to "call" messages.
    type Reply;

    /// Called when the bctor starts.
    fn init(&mut self, _env: &mut BctorEnv<Self>) -> Result<()> {
        Ok(())
    }

    /// Called when the bctor receives a message and does not need to reply.
    fn handle_cast(&mut self, _msg: Self::Cast, _env: &mut BctorEnv<Self>) -> Result<()> {
        Ok(())
    }

    /// Called when the bctor receives a message and needs to reply.
    ///
    /// Implementations should send the reply using the `reply_sender`,
    /// otherwise the caller may hang.
    fn handle_call(
        &mut self,
        _msg: Self::Call,
        _env: &mut BctorEnv<Self>,
        _reply_sender: oneshot::Sender<Self::Reply>,
    ) -> Result<()> {
        Ok(())
    }

    /// Called before the bctor exits.
    /// There are 3 cases when this method is called:
    /// - The bctor is cancelled. `run_result` would be `Ok(())`.
    /// - All message senders are closed, so no message will be received.
    ///     `run_result` would be `Ok(())`.
    /// - [`Bctor::init`], [`Bctor::handle_cast`],
    ///     or [`Bctor::handle_call`] returned an error.
    ///     `run_result` would contain the error.
    ///
    /// This method's return value would become [`BctorRunResult::exit_result`].
    fn before_exit(&mut self, run_result: Result<()>, _env: &mut BctorEnv<Self>) -> Result<()> {
        run_result
    }
}

/// Provides convenience methods for spawning [`Bctor`] instances.
///
/// <details>
/// <summary>This trait is object-safe.</summary>
///
/// ```
/// use tokio_gen_server::prelude::*;
/// let _: Box<dyn BctorExt<Ref = (), Msg = (), RunResult = ()>>;
/// ```
///
/// </details>
pub trait BctorExt {
    type Ref;
    type Msg;
    type RunResult;

    /// Spawn the bctor in a thread.
    fn spawn(self) -> (JoinHandle<Self::RunResult>, Self::Ref);

    /// Same as [`BctorExt::spawn`] but with the given cancellation token.
    /// Useful for leveraging [`CancellationToken`] inheritance.
    fn spawn_with_token(
        self,
        cancellation_token: CancellationToken,
    ) -> (JoinHandle<Self::RunResult>, Self::Ref);

    /// Same as [`BctorExt::spawn`] but with both ends of the channel given.
    /// Useful for relaying messages or reusing channels.
    fn spawn_with_channel(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
    ) -> (JoinHandle<Self::RunResult>, Self::Ref);

    /// [`BctorExt::spawn_with_channel`] + [`BctorExt::spawn_with_token`].
    fn spawn_with_channel_and_token(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
        cancellation_token: CancellationToken,
    ) -> (JoinHandle<Self::RunResult>, Self::Ref);

    // This comment preserves the blank line above for code generation.
}

impl<A> BctorExt for A
where
    A: Bctor + Send + 'static,
    BctorMsg<A>: Send,
{
    type Ref = BctorRef<A>;
    type Msg = BctorMsg<A>;
    type RunResult = BctorRunResult<A>;

    fn spawn(self) -> (JoinHandle<Self::RunResult>, Self::Ref) {
        let cancellation_token = CancellationToken::new();
        self.spawn_with_token(cancellation_token)
    }

    fn spawn_with_token(
        self,
        cancellation_token: CancellationToken,
    ) -> (JoinHandle<Self::RunResult>, Self::Ref) {
        let (msg_sender, msg_receiver) = channel(8);
        self.spawn_with_channel_and_token(msg_sender, msg_receiver, cancellation_token)
    }

    fn spawn_with_channel(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
    ) -> (JoinHandle<Self::RunResult>, Self::Ref) {
        let cancellation_token = CancellationToken::new();
        self.spawn_with_channel_and_token(msg_sender, msg_receiver, cancellation_token)
    }

    fn spawn_with_channel_and_token(
        mut self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
        cancellation_token: CancellationToken,
    ) -> (JoinHandle<Self::RunResult>, Self::Ref) {
        let bctor_ref = Ref {
            msg_sender,
            cancellation_token,
        };
        let handle = {
            let mut env = Env {
                ref_: bctor_ref.clone(),
                msg_receiver,
            };
            spawn(move || {
                let exit_result = self.run_and_handle_exit(&mut env);
                BctorRunResult {
                    bctor: self,
                    env,
                    exit_result,
                }
            })
        };
        (handle, bctor_ref)
    }

    // This comment preserves the blank line above for code generation.
}

/// Provides convenience methods for running [`Bctor`] instances.
/// Not intended for users.
///
/// <details>
/// <summary>This trait is object-safe.</summary>
///
/// ```
/// use tokio_gen_server::bctor::BctorRunExt;
/// let _: Box<dyn BctorRunExt<Env = (), Msg = ()>>;
/// ```
///
/// </details>
pub trait BctorRunExt {
    type Env;
    type Msg;

    fn handle_call_or_cast(&mut self, msg: Self::Msg, env: &mut Self::Env) -> Result<()>;

    fn handle_continuously(&mut self, env: &mut Self::Env) -> Result<()>;

    fn run_and_handle_exit(&mut self, env: &mut Self::Env) -> Result<()>;

    fn run_till_exit(&mut self, env: &mut Self::Env) -> Result<()>;
}

impl<A> BctorRunExt for A
where
    A: Bctor,
    BctorMsg<A>: Send,
{
    type Env = BctorEnv<A>;
    type Msg = BctorMsg<A>;

    fn handle_call_or_cast(&mut self, msg: Self::Msg, env: &mut Self::Env) -> Result<()> {
        match msg {
            Msg::Exit => unreachable!(
                "Exit signals should be handled before handling `handle_call_or_cast`."
            ),
            Msg::Call(msg, reply_sender) => self.handle_call(msg, env, reply_sender),
            Msg::Cast(msg) => self.handle_cast(msg, env),
        }
    }

    fn handle_continuously(&mut self, env: &mut Self::Env) -> Result<()> {
        let cancellation_token = env.ref_.cancellation_token.clone();
        loop {
            if cancellation_token.is_cancelled() {
                return Ok(());
            }
            let maybe_msg = env.msg_receiver.blocking_recv();
            let msg = match maybe_msg {
                Some(m) => m,
                None => return Ok(()),
            };
            if cancellation_token.is_cancelled() {
                return Ok(());
            }
            match msg {
                Msg::Exit => return Ok(()),
                _ => self.handle_call_or_cast(msg, env)?,
            };
        }
    }

    fn run_and_handle_exit(&mut self, env: &mut Self::Env) -> Result<()> {
        let run_result = self.run_till_exit(env);
        self.before_exit(run_result, env)
    }

    fn run_till_exit(&mut self, env: &mut Self::Env) -> Result<()> {
        self.init(env)?;
        self.handle_continuously(env)
    }
}

#[cfg(test)]
mod tests;
