//! Please see the documentation for [`Actor`].
use super::*;

// TODO: Error type.

/// A reference to an instance of [`Actor`],
/// to cast or call messages on it or cancel it.
#[derive(Debug)]
pub struct Ref<A: Actor> {
    pub msg_sender: Sender<Msg<A>>,
    pub cancellation_token: CancellationToken,
}

impl<A: Actor> Ref<A> {
    /// Cast a message to the actor and do not expect a reply.
    pub async fn cast(&mut self, msg: A::CastMsg) -> Result<(), SendError<Msg<A>>> {
        self.msg_sender.send(Msg::Cast(msg)).await
    }

    /// Same as [`Ref::cast`] but blocking.
    ///
    /// # Panics
    ///
    /// This function panics if called within an asynchronous execution context.
    pub fn blocking_cast(&mut self, msg: A::CastMsg) -> Result<(), SendError<Msg<A>>> {
        self.msg_sender.blocking_send(Msg::Cast(msg))
    }

    /// Call the actor and wait for a reply.
    ///
    /// To time out the call, use [`tokio::time::timeout`].
    pub async fn call(&mut self, msg: A::CallMsg) -> Result<A::Reply> {
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
    pub fn blocking_call(&mut self, msg: A::CallMsg) -> Result<A::Reply> {
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
        &mut self,
        msg: A::CallMsg,
        reply_sender: oneshot::Sender<A::Reply>,
    ) -> Result<(), SendError<Msg<A>>> {
        self.msg_sender.send(Msg::Call(msg, reply_sender)).await
    }

    /// Cancel the actor referred to, so it exits.
    pub fn cancel(&mut self) {
        self.cancellation_token.cancel()
    }
}

impl<A: Actor> Clone for Ref<A> {
    fn clone(&self) -> Self {
        Self {
            msg_sender: self.msg_sender.clone(),
            cancellation_token: self.cancellation_token.clone(),
        }
    }
}

/// A message sent to an actor.
#[derive(Debug)]
pub enum Msg<A: Actor> {
    Call(A::CallMsg, oneshot::Sender<A::Reply>),
    Cast(A::CastMsg),
}

#[doc = include_str!("actor_doc.md")]
pub trait Actor: Sized + Send + 'static {
    type CallMsg: Send + Sync;
    type CastMsg: Send + Sync;
    type Reply: Send;

    /// Called when the actor starts.
    fn init(&mut self, _env: &mut Ref<Self>) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }

    /// Called when the actor receives a message and does not need to reply.
    fn handle_cast(
        &mut self,
        _msg: Self::CastMsg,
        _env: &mut Ref<Self>,
    ) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }

    /// Called when the actor receives a message and needs to reply.
    ///
    /// Implementations should send the reply using the `reply_sender`,
    /// otherwise the caller may hang.
    fn handle_call(
        &mut self,
        _msg: Self::CallMsg,
        _env: &mut Ref<Self>,
        _reply_sender: oneshot::Sender<Self::Reply>,
    ) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }

    /// Called before the actor exits.
    fn before_exit(
        &mut self,
        _run_result: Result<()>,
        _env: &mut Ref<Self>,
        _msg_receiver: &mut Receiver<Msg<Self>>,
    ) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }
}

pub type ActorHandle<Msg> = JoinHandle<(Receiver<Msg>, Result<()>)>;

/// Provides convenience methods for [`Actor`].
/// Only [`ActorExt::spawn`] and its derivatives are intended to
/// be used directly.
pub trait ActorExt: Sized {
    type Ref;
    type Msg;

    fn handle_call_or_cast(
        &mut self,
        msg: Self::Msg,
        env: &mut Self::Ref,
    ) -> impl Future<Output = Result<()>> + Send;

    fn handle_continuously(
        &mut self,
        receiver: &mut Receiver<Self::Msg>,
        env: &mut Self::Ref,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Spawn the actor in a thread.
    fn spawn(self) -> (ActorHandle<Self::Msg>, Self::Ref);

    /// Same as [`ActorExt::spawn`] but with both ends of the channel given.
    /// Useful for relaying messages or reusing channels.
    fn spawn_with_channel(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
    ) -> (ActorHandle<Self::Msg>, Self::Ref);

    /// Same as [`ActorExt::spawn`] but with the given cancellation token.
    /// Useful for leveraging [`CancellationToken`] inheritance.
    fn spawn_with_token(
        self,
        cancellation_token: CancellationToken,
    ) -> (ActorHandle<Self::Msg>, Self::Ref);

    /// [`ActorExt::spawn_with_channel`] + [`ActorExt::spawn_with_token`].
    fn spawn_with_channel_and_token(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
        cancellation_token: CancellationToken,
    ) -> (ActorHandle<Self::Msg>, Self::Ref);

    fn run_and_handle_exit(
        self,
        env: Self::Ref,
        msg_receiver: Receiver<Self::Msg>,
    ) -> impl Future<Output = (Receiver<Self::Msg>, Result<()>)> + Send;

    fn run_till_exit(
        &mut self,
        env: &mut Self::Ref,
        msg_receiver: &mut Receiver<Self::Msg>,
    ) -> impl Future<Output = Result<()>> + Send;
}

impl<A: Actor> ActorExt for A {
    type Ref = Ref<A>;
    type Msg = Msg<A>;

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

    fn spawn(self) -> (ActorHandle<Self::Msg>, Self::Ref) {
        let cancellation_token = CancellationToken::new();
        self.spawn_with_token(cancellation_token)
    }

    fn spawn_with_channel(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
    ) -> (ActorHandle<Self::Msg>, Self::Ref) {
        let cancellation_token = CancellationToken::new();
        self.spawn_with_channel_and_token(msg_sender, msg_receiver, cancellation_token)
    }

    fn spawn_with_token(
        self,
        cancellation_token: CancellationToken,
    ) -> (ActorHandle<Self::Msg>, Self::Ref) {
        let (msg_sender, msg_receiver) = channel(8);
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

#[cfg(test)]
mod tests;
