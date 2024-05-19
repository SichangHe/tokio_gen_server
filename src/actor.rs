use super::*;

// TODO: Error type.
// TODO: Documentation.

#[derive(Debug)]
pub struct Ref<A: Actor> {
    pub msg_sender: Sender<Msg<A>>,
    pub cancellation_token: CancellationToken,
}

impl<A: Actor> Ref<A> {
    pub async fn cast(&mut self, msg: A::CastMsg) -> Result<(), SendError<Msg<A>>> {
        self.msg_sender.send(Msg::Cast(msg)).await
    }

    pub fn blocking_cast(&mut self, msg: A::CastMsg) -> Result<(), SendError<Msg<A>>> {
        self.msg_sender.blocking_send(Msg::Cast(msg))
    }

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

    pub fn blocking_call(&mut self, msg: A::CallMsg) -> Result<A::Reply> {
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.msg_sender
            .blocking_send(Msg::Call(msg, reply_sender))
            .context("Failed to send call to actor")?;
        reply_receiver
            .blocking_recv()
            .context("Failed to receive actor's reply")
    }

    pub async fn relay_call(
        &mut self,
        msg: A::CallMsg,
        reply_sender: oneshot::Sender<A::Reply>,
    ) -> Result<(), SendError<Msg<A>>> {
        self.msg_sender.send(Msg::Call(msg, reply_sender)).await
    }

    /// Cancel the actor referred to.
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

    fn init(&mut self, _env: &mut Ref<Self>) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }

    fn handle_cast(
        &mut self,
        _msg: Self::CastMsg,
        _env: &mut Ref<Self>,
    ) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }

    fn handle_call(
        &mut self,
        _msg: Self::CallMsg,
        _env: &mut Ref<Self>,
        _reply_sender: oneshot::Sender<Self::Reply>,
    ) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }

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

    fn spawn(self) -> (ActorHandle<Self::Msg>, Self::Ref);

    fn spawn_with_channel(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
    ) -> (ActorHandle<Self::Msg>, Self::Ref);

    fn spawn_with_token(
        self,
        cancellation_token: CancellationToken,
    ) -> (ActorHandle<Self::Msg>, Self::Ref);

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
