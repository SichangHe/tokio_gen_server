//! Please see the documentation for [`Actor`].
use super::*;

/// The result when the [`Actor`] exits.
pub struct ActorRunResult<A: Actor> {
    /// The [`Actor`] itself.
    pub actor: A,
    /// The [`Actor`]'s environment.
    pub env: ActorEnv<A>,
    /// The result of the [`Actor`] exiting.
    pub exit_result: Result<()>,
}

/// The environment the [`Actor`] runs in.
#[derive(Debug)]
pub struct Env<Call, Cast, Reply> {
    /// The reference to the [`Actor`] itself.
    pub ref_: Ref<Call, Cast, Reply>,
    /// The [`Actor`]'s message receiver.
    pub msg_receiver: Receiver<Msg<Call, Cast, Reply>>,
}

/// The environment the [`Actor`] runs in.
pub type ActorEnv<A> = Env<<A as Actor>::Call, <A as Actor>::Cast, <A as Actor>::Reply>;

/// A reference to an instance of [`Actor`],
/// to cast or call messages on it or cancel it.
#[derive(Debug)]
pub struct Ref<Call, Cast, Reply> {
    /// A message sender to send messages to the [`Actor`].
    pub msg_sender: Sender<Msg<Call, Cast, Reply>>,
    /// A token to cancel the [`Actor`].
    pub cancellation_token: CancellationToken,
}

/// A reference to an instance of [`Actor`],
/// to cast or call messages on it or cancel it.
pub type ActorRef<A> = Ref<<A as Actor>::Call, <A as Actor>::Cast, <A as Actor>::Reply>;

impl<Call, Cast, Reply> Ref<Call, Cast, Reply> {
    /// Cast a message to the actor and do not expect a reply.
    pub async fn cast(&self, msg: Cast) -> Result<(), SendError<Msg<Call, Cast, Reply>>> {
        self.msg_sender.send(Msg::Cast(msg)).await
    }

    /// Same as [`Ref::cast`] but blocking.
    ///
    /// # Panics
    ///
    /// This function panics if called within an asynchronous execution context.
    pub fn blocking_cast(&self, msg: Cast) -> Result<(), SendError<Msg<Call, Cast, Reply>>> {
        self.msg_sender.blocking_send(Msg::Cast(msg))
    }

    /// Call the actor and wait for a reply.
    ///
    /// To time out the call, use [`tokio::time::timeout`].
    pub async fn call(&self, msg: Call) -> Result<Reply>
    where
        Msg<Call, Cast, Reply>: Send + Sync + 'static,
    {
        // NB: Using the `oneshot` channel here is inexpensive because its only
        // overhead is 1 `Arc` and 5 extra words of allocation.
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.msg_sender
            .send(Msg::Call(msg, reply_sender))
            .await
            .context("Failed to send call to actor")?;
        reply_receiver
            .await
            .context("Failed to receive actor's reply")
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
            .context("Failed to send call to actor")?;
        reply_receiver
            .blocking_recv()
            .context("Failed to receive actor's reply")
    }

    /// Call the actor and let it reply via a given channel sender.
    /// Useful for relaying a call from some other caller.
    pub async fn relay_call(
        &self,
        msg: Call,
        reply_sender: oneshot::Sender<Reply>,
    ) -> Result<(), SendError<Msg<Call, Cast, Reply>>> {
        self.msg_sender.send(Msg::Call(msg, reply_sender)).await
    }

    /// Cancel the actor referred to, so it exits.
    pub fn cancel(&self) {
        self.cancellation_token.cancel()
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

/// A message sent to an actor.
#[derive(Debug)]
pub enum Msg<Call, Cast, Reply> {
    Call(Call, oneshot::Sender<Reply>),
    Cast(Cast),
}

/// A message sent to an actor.
pub type ActorMsg<A> = Msg<<A as Actor>::Call, <A as Actor>::Cast, <A as Actor>::Reply>;

#[doc = include_str!("actor_doc.md")]
pub trait Actor {
    /// "Call" message for requests that anticipate replies.
    type Call;
    /// "Cast" message for fire-and-forget requests.
    type Cast;
    /// "Reply" message for replying to "call" messages.
    type Reply;

    /// Called when the actor starts.
    /// # Snippet for copying
    /// ```ignore
    /// async fn init(&mut self, env: &mut ActorEnv<Self>) -> Result<()> {
    ///     Ok(())
    /// }
    /// ```
    fn init(&mut self, _env: &mut ActorEnv<Self>) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }

    /// Called when the actor receives a message and does not need to reply.
    /// # Snippet for copying
    /// ```ignore
    /// async fn handle_cast(&mut self, msg: Self::Cast, env: &mut ActorEnv<Self>) -> Result<()> {
    ///     Ok(())
    /// }
    /// ```
    fn handle_cast(
        &mut self,
        _msg: Self::Cast,
        _env: &mut ActorEnv<Self>,
    ) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }

    /// Called when the actor receives a message and needs to reply.
    ///
    /// Implementations should send the reply using the `reply_sender`,
    /// otherwise the caller may hang.
    /// # Snippet for copying
    /// ```ignore
    /// async fn handle_call(
    ///     &mut self,
    ///     msg: Self::Call,
    ///     env: &mut ActorEnv<Self>,
    ///     reply_sender: oneshot::Sender<Self::Reply>,
    /// ) -> Result<()> {
    ///     Ok(())
    /// }
    /// ```
    fn handle_call(
        &mut self,
        _msg: Self::Call,
        _env: &mut ActorEnv<Self>,
        _reply_sender: oneshot::Sender<Self::Reply>,
    ) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }

    /// Called before the actor exits.
    /// There are 3 cases when this method is called:
    /// - The actor is cancelled. `run_result` would be `Ok(())`.
    /// - All message senders are closed, so no message will be received.
    ///     `run_result` would be `Ok(())`.
    /// - [`Actor::init`], [`Actor::handle_cast`],
    ///     or [`Actor::handle_call`] returned an error.
    ///     `run_result` would contain the error.
    ///
    /// This method's return value would become [`ActorRunResult::exit_result`].
    /// # Snippet for copying
    /// ```ignore
    /// async fn before_exit(
    ///     &mut self,
    ///     run_result: Result<()>,
    ///     env: &mut ActorEnv<Self>,
    /// ) -> Result<()> {
    ///     run_result
    /// }
    /// ```
    fn before_exit(
        &mut self,
        run_result: Result<()>,
        _env: &mut ActorEnv<Self>,
    ) -> impl Future<Output = Result<()>> + Send {
        async { run_result }
    }
}

/// Provides convenience methods for spawning [`Actor`] instances.
///
/// <details>
/// <summary>This trait is object-safe.</summary>
///
/// ```
/// use tokio_gen_server::prelude::*;
/// let _: Box<dyn ActorExt<Ref = (), Msg = (), RunResult = ()>>;
/// ```
///
/// </details>
pub trait ActorExt {
    type Ref;
    type Msg;
    type RunResult;

    /// Spawn the actor in a task.
    fn spawn(self) -> (JoinHandle<Self::RunResult>, Self::Ref);

    /// Same as [`ActorExt::spawn`] but with the given cancellation token.
    /// Useful for leveraging [`CancellationToken`] inheritance.
    fn spawn_with_token(
        self,
        cancellation_token: CancellationToken,
    ) -> (JoinHandle<Self::RunResult>, Self::Ref);

    /// Same as [`ActorExt::spawn`] but with both ends of the channel given.
    /// Useful for relaying messages or reusing channels.
    fn spawn_with_channel(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
    ) -> (JoinHandle<Self::RunResult>, Self::Ref);

    /// [`ActorExt::spawn_with_channel`] + [`ActorExt::spawn_with_token`].
    fn spawn_with_channel_and_token(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
        cancellation_token: CancellationToken,
    ) -> (JoinHandle<Self::RunResult>, Self::Ref);

    /// Spawn the actor in a task from the given [`JoinSet`].
    fn spawn_from_join_set(
        self,
        join_set: &mut JoinSet<Self::RunResult>,
    ) -> (AbortHandle, Self::Ref);

    /// Same as [`ActorExt::spawn_from_join_set`] but with
    /// the given cancellation token.
    /// Useful for leveraging [`CancellationToken`] inheritance.
    fn spawn_with_token_from_join_set(
        self,
        cancellation_token: CancellationToken,
        join_set: &mut JoinSet<Self::RunResult>,
    ) -> (AbortHandle, Self::Ref);

    /// Same as [`ActorExt::spawn_from_join_set`] but with both ends of
    /// the channel given.
    /// Useful for relaying messages or reusing channels.
    fn spawn_with_channel_from_join_set(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
        join_set: &mut JoinSet<Self::RunResult>,
    ) -> (AbortHandle, Self::Ref);

    /// [`ActorExt::spawn_with_channel_from_join_set`] +
    /// [`ActorExt::spawn_with_token_from_join_set`].
    fn spawn_with_channel_and_token_from_join_set(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
        cancellation_token: CancellationToken,
        join_set: &mut JoinSet<Self::RunResult>,
    ) -> (AbortHandle, Self::Ref);

    // This comment preserves the blank line above for code generation.
}

impl<A> ActorExt for A
where
    A: Actor + Send + 'static,
    ActorMsg<A>: Send,
{
    type Ref = ActorRef<A>;
    type Msg = ActorMsg<A>;
    type RunResult = ActorRunResult<A>;

    fn spawn(self) -> (JoinHandle<ActorRunResult<A>>, ActorRef<A>) {
        let cancellation_token = CancellationToken::new();
        self.spawn_with_token(cancellation_token)
    }

    fn spawn_with_token(
        self,
        cancellation_token: CancellationToken,
    ) -> (JoinHandle<ActorRunResult<A>>, ActorRef<A>) {
        let (msg_sender, msg_receiver) = channel(8);
        self.spawn_with_channel_and_token(msg_sender, msg_receiver, cancellation_token)
    }

    fn spawn_with_channel(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
    ) -> (JoinHandle<ActorRunResult<A>>, ActorRef<A>) {
        let cancellation_token = CancellationToken::new();
        self.spawn_with_channel_and_token(msg_sender, msg_receiver, cancellation_token)
    }

    fn spawn_with_channel_and_token(
        mut self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
        cancellation_token: CancellationToken,
    ) -> (JoinHandle<ActorRunResult<A>>, ActorRef<A>) {
        let actor_ref = Ref {
            msg_sender,
            cancellation_token,
        };
        let handle = {
            let mut env = Env {
                ref_: actor_ref.clone(),
                msg_receiver,
            };
            spawn(async move {
                let exit_result = self.run_and_handle_exit(&mut env).await;
                ActorRunResult {
                    actor: self,
                    env,
                    exit_result,
                }
            })
        };
        (handle, actor_ref)
    }

    fn spawn_from_join_set(
        self,
        join_set: &mut JoinSet<ActorRunResult<A>>,
    ) -> (AbortHandle, ActorRef<A>) {
        let cancellation_token = CancellationToken::new();
        self.spawn_with_token_from_join_set(cancellation_token, join_set)
    }

    fn spawn_with_token_from_join_set(
        self,
        cancellation_token: CancellationToken,
        join_set: &mut JoinSet<ActorRunResult<A>>,
    ) -> (AbortHandle, ActorRef<A>) {
        let (msg_sender, msg_receiver) = channel(8);
        self.spawn_with_channel_and_token_from_join_set(
            msg_sender,
            msg_receiver,
            cancellation_token,
            join_set,
        )
    }

    fn spawn_with_channel_from_join_set(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
        join_set: &mut JoinSet<ActorRunResult<A>>,
    ) -> (AbortHandle, ActorRef<A>) {
        let cancellation_token = CancellationToken::new();
        self.spawn_with_channel_and_token_from_join_set(
            msg_sender,
            msg_receiver,
            cancellation_token,
            join_set,
        )
    }

    fn spawn_with_channel_and_token_from_join_set(
        mut self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
        cancellation_token: CancellationToken,
        join_set: &mut JoinSet<ActorRunResult<A>>,
    ) -> (AbortHandle, ActorRef<A>) {
        let actor_ref = Ref {
            msg_sender,
            cancellation_token,
        };
        let handle = {
            let mut env = Env {
                ref_: actor_ref.clone(),
                msg_receiver,
            };
            join_set.spawn(async move {
                let exit_result = self.run_and_handle_exit(&mut env).await;
                ActorRunResult {
                    actor: self,
                    env,
                    exit_result,
                }
            })
        };
        (handle, actor_ref)
    }

    // This comment preserves the blank line above for code generation.
}

/// Provides convenience methods for running [`Actor`] instances.
/// Not intended for users.
///
/// <details>
/// <summary>This trait is not object-safe.</summary>
///
/// ```compile_fail
/// use tokio_gen_server::actor::ActorRunExt;
/// let _: Box<dyn ActorRunExt<Env = (), Msg = ()>>;
/// ```
///
/// </details>
pub trait ActorRunExt {
    type Env;
    type Msg;

    fn handle_call_or_cast(
        &mut self,
        msg: Self::Msg,
        env: &mut Self::Env,
    ) -> impl Future<Output = Result<()>>;

    fn handle_continuously(&mut self, env: &mut Self::Env) -> impl Future<Output = Result<()>>;

    fn run_and_handle_exit(&mut self, env: &mut Self::Env) -> impl Future<Output = Result<()>>;

    fn run_till_exit(&mut self, env: &mut Self::Env) -> impl Future<Output = Result<()>>;
}

impl<A> ActorRunExt for A
where
    A: Actor,
    ActorMsg<A>: Send,
{
    type Env = ActorEnv<A>;
    type Msg = ActorMsg<A>;

    async fn handle_call_or_cast(&mut self, msg: ActorMsg<A>, env: &mut ActorEnv<A>) -> Result<()> {
        match msg {
            Msg::Call(msg, reply_sender) => self.handle_call(msg, env, reply_sender).await,
            Msg::Cast(msg) => self.handle_cast(msg, env).await,
        }
    }

    async fn handle_continuously(&mut self, env: &mut ActorEnv<A>) -> Result<()> {
        let cancellation_token = env.ref_.cancellation_token.clone();
        loop {
            let maybe_msg = select! {
                biased;
                () = cancellation_token.cancelled() => return Ok(()),
                m = env.msg_receiver.recv() => m,
            };
            let msg = match maybe_msg {
                Some(m) => m,
                None => return Ok(()),
            };
            select! {
                biased;
                () = cancellation_token.cancelled() => return Ok(()),
                maybe_ok = self.handle_call_or_cast(msg, env) => maybe_ok,
            }?;
        }
    }

    async fn run_and_handle_exit(&mut self, env: &mut ActorEnv<A>) -> Result<()> {
        let run_result = self.run_till_exit(env).await;
        self.before_exit(run_result, env).await
    }

    async fn run_till_exit(&mut self, env: &mut ActorEnv<A>) -> Result<()> {
        self.init(env).await?;
        self.handle_continuously(env).await
    }
}

#[cfg(test)]
mod tests;
