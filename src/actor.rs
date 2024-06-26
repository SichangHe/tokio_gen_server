//! Please see the documentation for [`Actor`].
use super::*;

// TODO: Error type.

/// A reference to an instance of [`Actor`],
/// to cast or call messages on it or cancel it.
#[derive(Debug)]
pub struct Ref<L, T, R> {
    pub msg_sender: Sender<Msg<L, T, R>>,
    pub cancellation_token: CancellationToken,
}

/// A reference to an instance of [`Actor`],
/// to cast or call messages on it or cancel it.
pub type ActorRef<A> = Ref<<A as Actor>::L, <A as Actor>::T, <A as Actor>::R>;

impl<L, T, R> Ref<L, T, R> {
    /// Cast a message to the actor and do not expect a reply.
    pub async fn cast(&self, msg: T) -> Result<(), SendError<Msg<L, T, R>>> {
        self.msg_sender.send(Msg::Cast(msg)).await
    }

    /// Same as [`Ref::cast`] but blocking.
    ///
    /// # Panics
    ///
    /// This function panics if called within an asynchronous execution context.
    pub fn blocking_cast(&self, msg: T) -> Result<(), SendError<Msg<L, T, R>>> {
        self.msg_sender.blocking_send(Msg::Cast(msg))
    }

    /// Call the actor and wait for a reply.
    ///
    /// To time out the call, use [`tokio::time::timeout`].
    pub async fn call(&self, msg: L) -> Result<R>
    where
        Msg<L, T, R>: Send + Sync + 'static,
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
    pub fn blocking_call(&self, msg: L) -> Result<R>
    where
        Msg<L, T, R>: Send + Sync + 'static,
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
        msg: L,
        reply_sender: oneshot::Sender<R>,
    ) -> Result<(), SendError<Msg<L, T, R>>> {
        self.msg_sender.send(Msg::Call(msg, reply_sender)).await
    }

    /// Cancel the actor referred to, so it exits.
    pub fn cancel(&self) {
        self.cancellation_token.cancel()
    }
}

impl<L, T, R> Clone for Ref<L, T, R> {
    fn clone(&self) -> Self {
        Self {
            msg_sender: self.msg_sender.clone(),
            cancellation_token: self.cancellation_token.clone(),
        }
    }
}

/// A message sent to an actor.
#[derive(Debug)]
pub enum Msg<L, T, R> {
    Call(L, oneshot::Sender<R>),
    Cast(T),
}

/// A message sent to an actor.
pub type ActorMsg<A> = Msg<<A as Actor>::L, <A as Actor>::T, <A as Actor>::R>;

#[doc = include_str!("actor_doc.md")]
pub trait Actor {
    /// Cal***L*** message type.
    type L;
    /// Cas***T*** message type.
    type T;
    /// ***R***eply message type.
    type R;

    /// Called when the actor starts.
    fn init(&mut self, _env: &mut ActorRef<Self>) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }

    /// Called when the actor receives a message and does not need to reply.
    fn handle_cast(
        &mut self,
        _msg: Self::T,
        _env: &mut ActorRef<Self>,
    ) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }

    /// Called when the actor receives a message and needs to reply.
    ///
    /// Implementations should send the reply using the `reply_sender`,
    /// otherwise the caller may hang.
    fn handle_call(
        &mut self,
        _msg: Self::L,
        _env: &mut ActorRef<Self>,
        _reply_sender: oneshot::Sender<Self::R>,
    ) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }

    /// Called before the actor exits.
    fn before_exit(
        &mut self,
        _run_result: Result<()>,
        _env: &mut ActorRef<Self>,
        _msg_receiver: &mut Receiver<ActorMsg<Self>>,
    ) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }
}

pub type ActorOutput<Msg> = (Receiver<Msg>, Result<()>);
pub type ActorHandle<Msg> = JoinHandle<ActorOutput<Msg>>;

/// Provides convenience methods for spawning [`Actor`] instances.
pub trait ActorExt {
    type Ref;
    type Msg;

    /// Spawn the actor in a task.
    fn spawn(self) -> (ActorHandle<Self::Msg>, Self::Ref);

    /// Same as [`ActorExt::spawn`] but with the given cancellation token.
    /// Useful for leveraging [`CancellationToken`] inheritance.
    fn spawn_with_token(
        self,
        cancellation_token: CancellationToken,
    ) -> (ActorHandle<Self::Msg>, Self::Ref);

    /// Same as [`ActorExt::spawn`] but with both ends of the channel given.
    /// Useful for relaying messages or reusing channels.
    fn spawn_with_channel(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
    ) -> (ActorHandle<Self::Msg>, Self::Ref);

    /// [`ActorExt::spawn_with_channel`] + [`ActorExt::spawn_with_token`].
    fn spawn_with_channel_and_token(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
        cancellation_token: CancellationToken,
    ) -> (ActorHandle<Self::Msg>, Self::Ref);

    /// Spawn the actor in a task from the given [`JoinSet`].
    fn spawn_from_join_set(
        self,
        join_set: &mut JoinSet<ActorOutput<Self::Msg>>,
    ) -> (AbortHandle, Self::Ref);

    /// Same as [`ActorExt::spawn_from_join_set`] but with
    /// the given cancellation token.
    /// Useful for leveraging [`CancellationToken`] inheritance.
    fn spawn_with_token_from_join_set(
        self,
        cancellation_token: CancellationToken,
        join_set: &mut JoinSet<ActorOutput<Self::Msg>>,
    ) -> (AbortHandle, Self::Ref);

    /// Same as [`ActorExt::spawn_from_join_set`] but with both ends of
    /// the channel given.
    /// Useful for relaying messages or reusing channels.
    fn spawn_with_channel_from_join_set(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
        join_set: &mut JoinSet<ActorOutput<Self::Msg>>,
    ) -> (AbortHandle, Self::Ref);

    /// [`ActorExt::spawn_with_channel_from_join_set`] +
    /// [`ActorExt::spawn_with_token_from_join_set`].
    fn spawn_with_channel_and_token_from_join_set(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
        cancellation_token: CancellationToken,
        join_set: &mut JoinSet<ActorOutput<Self::Msg>>,
    ) -> (AbortHandle, Self::Ref);

    // This comment preserves the blank line above for code generation.
}

/// Provides convenience methods for running [`Actor`] instances.
/// Not intended for users.
pub trait ActorRunExt {
    type Ref;
    type Msg;

    fn handle_call_or_cast(
        &mut self,
        msg: Self::Msg,
        env: &mut Self::Ref,
    ) -> impl Future<Output = Result<()>>;

    fn handle_continuously(
        &mut self,
        receiver: &mut Receiver<Self::Msg>,
        env: &mut Self::Ref,
    ) -> impl Future<Output = Result<()>>;

    fn run_and_handle_exit(
        self,
        env: Self::Ref,
        msg_receiver: Receiver<Self::Msg>,
    ) -> impl Future<Output = (Receiver<Self::Msg>, Result<()>)>;

    fn run_till_exit(
        &mut self,
        env: &mut Self::Ref,
        msg_receiver: &mut Receiver<Self::Msg>,
    ) -> impl Future<Output = Result<()>>;
}

impl<A> ActorRunExt for A
where
    A: Actor,
    ActorMsg<A>: Send,
{
    type Ref = ActorRef<A>;
    type Msg = ActorMsg<A>;

    async fn handle_call_or_cast(&mut self, msg: Self::Msg, env: &mut Self::Ref) -> Result<()> {
        match msg {
            Msg::Call(msg, reply_sender) => self.handle_call(msg, env, reply_sender).await,
            Msg::Cast(msg) => self.handle_cast(msg, env).await,
        }
    }

    async fn handle_continuously(
        &mut self,
        receiver: &mut Receiver<Self::Msg>,
        env: &mut Self::Ref,
    ) -> Result<()> {
        let cancellation_token = env.cancellation_token.clone();

        loop {
            let maybe_msg = select! {
                m = receiver.recv() => m,
                () = cancellation_token.cancelled() => return Ok(()),
            };

            let msg = match maybe_msg {
                Some(m) => m,
                None => return Ok(()),
            };

            select! {
                maybe_ok = self.handle_call_or_cast(msg, env) => maybe_ok,
                () = cancellation_token.cancelled() => return Ok(()),
            }?;
        }
    }

    async fn run_and_handle_exit(
        mut self,
        mut env: Self::Ref,
        mut msg_receiver: Receiver<Self::Msg>,
    ) -> (Receiver<Self::Msg>, Result<()>) {
        let run_result = self.run_till_exit(&mut env, &mut msg_receiver).await;
        let exit_result = self
            .before_exit(run_result, &mut env, &mut msg_receiver)
            .await;
        (msg_receiver, exit_result)
    }

    async fn run_till_exit(
        &mut self,
        env: &mut Self::Ref,
        msg_receiver: &mut Receiver<Self::Msg>,
    ) -> Result<()> {
        self.init(env).await?;
        self.handle_continuously(msg_receiver, env).await
    }
}

impl<A> ActorExt for A
where
    A: Actor + Send + 'static,
    ActorMsg<A>: Send,
{
    type Ref = ActorRef<A>;
    type Msg = ActorMsg<A>;

    fn spawn(self) -> (ActorHandle<Self::Msg>, Self::Ref) {
        let cancellation_token = CancellationToken::new();
        self.spawn_with_token(cancellation_token)
    }

    fn spawn_with_token(
        self,
        cancellation_token: CancellationToken,
    ) -> (ActorHandle<Self::Msg>, Self::Ref) {
        let (msg_sender, msg_receiver) = channel(8);
        self.spawn_with_channel_and_token(msg_sender, msg_receiver, cancellation_token)
    }

    fn spawn_with_channel(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
    ) -> (ActorHandle<Self::Msg>, Self::Ref) {
        let cancellation_token = CancellationToken::new();
        self.spawn_with_channel_and_token(msg_sender, msg_receiver, cancellation_token)
    }

    fn spawn_with_channel_and_token(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
        cancellation_token: CancellationToken,
    ) -> (ActorHandle<Self::Msg>, Self::Ref) {
        let actor_ref = Ref {
            msg_sender,
            cancellation_token,
        };
        let handle = {
            let env = actor_ref.clone();
            spawn(self.run_and_handle_exit(env, msg_receiver))
        };
        (handle, actor_ref)
    }

    fn spawn_from_join_set(
        self,
        join_set: &mut JoinSet<ActorOutput<Self::Msg>>,
    ) -> (AbortHandle, Self::Ref) {
        let cancellation_token = CancellationToken::new();
        self.spawn_with_token_from_join_set(cancellation_token, join_set)
    }

    fn spawn_with_token_from_join_set(
        self,
        cancellation_token: CancellationToken,
        join_set: &mut JoinSet<ActorOutput<Self::Msg>>,
    ) -> (AbortHandle, Self::Ref) {
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
        join_set: &mut JoinSet<ActorOutput<Self::Msg>>,
    ) -> (AbortHandle, Self::Ref) {
        let cancellation_token = CancellationToken::new();
        self.spawn_with_channel_and_token_from_join_set(
            msg_sender,
            msg_receiver,
            cancellation_token,
            join_set,
        )
    }

    fn spawn_with_channel_and_token_from_join_set(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
        cancellation_token: CancellationToken,
        join_set: &mut JoinSet<ActorOutput<Self::Msg>>,
    ) -> (AbortHandle, Self::Ref) {
        let actor_ref = Ref {
            msg_sender,
            cancellation_token,
        };
        let handle = {
            let env = actor_ref.clone();
            join_set.spawn(self.run_and_handle_exit(env, msg_receiver))
        };
        (handle, actor_ref)
    }

    // This comment preserves the blank line above for code generation.
}

#[cfg(test)]
mod tests;
