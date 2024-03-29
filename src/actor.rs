use super::*;

// TODO: Error type.
// TODO: Documentation.

#[derive(Debug)]
pub struct Ref<A: Actor> {
    pub msg_sender: Sender<Msg<A>>,
    pub cancellation_token: CancellationToken,
}

impl<A: Actor> Ref<A> {
    pub fn cast(
        &mut self,
        msg: A::CastMsg,
    ) -> impl Future<Output = Result<(), SendError<Msg<A>>>> + '_ {
        self.msg_sender.send(Msg::Cast(msg))
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

    pub fn relay_call(
        &mut self,
        msg: A::CallMsg,
        reply_sender: oneshot::Sender<A::Reply>,
    ) -> impl Future<Output = Result<(), SendError<Msg<A>>>> + '_ {
        self.msg_sender.send(Msg::Call(msg, reply_sender))
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

pub enum Msg<A: Actor> {
    Call(A::CallMsg, oneshot::Sender<A::Reply>),
    Cast(A::CastMsg),
}

/// An Elixir/Erlang-GenServer-like actor.
///
/// ## Example
///
/// ```rust
/// use anyhow::Result;
/// use std::time::Duration;
/// use tokio::{sync::oneshot, time::timeout};
/// use tokio_gen_server::*;
///
/// #[derive(Debug, Default)]
/// struct PingPongServer {
///     counter: usize,
/// }
///
/// struct Ping;
///
/// enum PingOrPong {
///     Ping,
///     Pong,
/// }
///
/// #[derive(Debug, Eq, PartialEq)]
/// enum PongOrCount {
///     Pong,
///     Count(usize),
/// }
///
/// impl Actor for PingPongServer {
///     type CastMsg = Ping;
///     type CallMsg = PingOrPong;
///     type Reply = PongOrCount;
///
///     async fn init(&mut self, _env: &mut Ref<Self>) -> Result<()> {
///         println!("PingPongServer starting.");
///         Ok(())
///     }
///
///     async fn handle_cast(&mut self, _msg: Self::CastMsg, _env: &mut Ref<Self>) -> Result<()> {
///         self.counter += 1;
///         println!("Received ping #{}", self.counter);
///         Ok(())
///     }
///
///     async fn handle_call(
///         &mut self,
///         msg: Self::CallMsg,
///         _env: &mut Ref<Self>,
///         reply_sender: oneshot::Sender<Self::Reply>,
///     ) -> Result<()> {
///         match msg {
///             PingOrPong::Ping => {
///                 self.counter += 1;
///                 println!("Received ping #{} as a call", self.counter);
///                 reply_sender.send(PongOrCount::Pong).unwrap();
///             }
///             PingOrPong::Pong => reply_sender.send(PongOrCount::Count(self.counter)).unwrap(),
///         }
///         Ok(())
///     }
///
///     async fn before_exit(&mut self, _env: &mut Ref<Self>) -> Result<()> {
///         println!(
///             "PingPongServer exiting with {} pings received.",
///             self.counter
///         );
///         Ok(())
///     }
/// }
///
/// const DECI_SECOND: Duration = Duration::from_millis(100);
///
/// #[tokio::test]
/// async fn ping_pong() -> Result<()> {
///     let ping_pong_server = PingPongServer::default();
///     let (handle, mut server_ref) = ping_pong_server.spawn();
///
///     server_ref.cast(Ping).await?;
///     let pong = server_ref.call(PingOrPong::Ping).await?;
///     assert_eq!(pong, PongOrCount::Pong);
///
///     let count = server_ref.call(PingOrPong::Pong).await?;
///     assert_eq!(count, PongOrCount::Count(2));
///
///     server_ref.cancel();
///     timeout(DECI_SECOND, handle).await???;
///
///     Ok(())
/// }
/// ```
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

    fn before_exit(&mut self, _env: &mut Ref<Self>) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }
}

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

    fn spawn(self) -> (JoinHandle<Result<()>>, Self::Ref);

    fn spawn_with_channel(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
    ) -> (JoinHandle<Result<()>>, Self::Ref);

    fn spawn_with_token(
        self,
        cancellation_token: CancellationToken,
    ) -> (JoinHandle<Result<()>>, Self::Ref);

    fn spawn_with_channel_and_token(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
        cancellation_token: CancellationToken,
    ) -> (JoinHandle<Result<()>>, Self::Ref);
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

    fn spawn(self) -> (JoinHandle<Result<()>>, Self::Ref) {
        let cancellation_token = CancellationToken::new();
        self.spawn_with_token(cancellation_token)
    }

    fn spawn_with_channel(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
    ) -> (JoinHandle<Result<()>>, Self::Ref) {
        let cancellation_token = CancellationToken::new();
        self.spawn_with_channel_and_token(msg_sender, msg_receiver, cancellation_token)
    }

    fn spawn_with_token(
        self,
        cancellation_token: CancellationToken,
    ) -> (JoinHandle<Result<()>>, Self::Ref) {
        let (msg_sender, msg_receiver) = channel(8);
        self.spawn_with_channel_and_token(msg_sender, msg_receiver, cancellation_token)
    }

    fn spawn_with_channel_and_token(
        mut self,
        msg_sender: Sender<Self::Msg>,
        mut msg_receiver: Receiver<Self::Msg>,
        cancellation_token: CancellationToken,
    ) -> (JoinHandle<Result<()>>, Self::Ref) {
        let actor_ref = Ref {
            msg_sender,
            cancellation_token,
        };
        let handle = {
            let mut env = actor_ref.clone();
            spawn(async move {
                self.init(&mut env).await?;
                self.handle_continuously(&mut msg_receiver, &mut env)
                    .await?;
                self.before_exit(&mut env).await
            })
        };

        (handle, actor_ref)
    }
}
