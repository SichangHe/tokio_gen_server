//! Blocking actor. Mirrors functionalities in `actor` but blocking.
use super::*;

// TODO: Error type.
// TODO: Documentation.

#[derive(Debug)]
pub struct Ref<A: Bctor> {
    pub msg_sender: Sender<Msg<A>>,
    pub cancellation_token: CancellationToken,
}

impl<A: Bctor> Ref<A> {
    pub fn cast(&mut self, msg: A::CastMsg) -> Result<(), SendError<Msg<A>>> {
        self.msg_sender.blocking_send(Msg::Cast(msg))
    }

    pub fn call(&mut self, msg: A::CallMsg) -> Result<A::Reply> {
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

    pub fn relay_call(
        &mut self,
        msg: A::CallMsg,
        reply_sender: oneshot::Sender<A::Reply>,
    ) -> Result<(), SendError<Msg<A>>> {
        self.msg_sender.blocking_send(Msg::Call(msg, reply_sender))
    }

    /// Cancel the bctor referred to.
    pub fn cancel(&mut self) {
        self.cancellation_token.cancel()
    }
}

impl<A: Bctor> Clone for Ref<A> {
    fn clone(&self) -> Self {
        Self {
            msg_sender: self.msg_sender.clone(),
            cancellation_token: self.cancellation_token.clone(),
        }
    }
}

#[derive(Debug)]
pub enum Msg<A: Bctor> {
    Call(A::CallMsg, oneshot::Sender<A::Reply>),
    Cast(A::CastMsg),
}

#[doc = include_str!("bctor_doc.md")]
pub trait Bctor: Sized + Send + 'static {
    type CallMsg: Send + Sync;
    type CastMsg: Send + Sync;
    type Reply: Send;

    fn init(&mut self, _env: &mut Ref<Self>) -> Result<()> {
        Ok(())
    }

    fn handle_cast(&mut self, _msg: Self::CastMsg, _env: &mut Ref<Self>) -> Result<()> {
        Ok(())
    }

    fn handle_call(
        &mut self,
        _msg: Self::CallMsg,
        _env: &mut Ref<Self>,
        _reply_sender: oneshot::Sender<Self::Reply>,
    ) -> Result<()> {
        Ok(())
    }

    fn before_exit(
        &mut self,
        _run_result: Result<()>,
        _env: &mut Ref<Self>,
        _msg_receiver: &mut Receiver<Msg<Self>>,
    ) -> Result<()> {
        Ok(())
    }
}

pub type BctorHandle<Msg> = JoinHandle<(Receiver<Msg>, Result<()>)>;

/// Provides convenience methods for [`Bctor`].
pub trait BctorExt: Sized {
    type Ref;
    type Msg;

    fn handle_call_or_cast(&mut self, msg: Self::Msg, env: &mut Self::Ref) -> Result<()>;

    fn handle_continuously(
        &mut self,
        receiver: &mut Receiver<Self::Msg>,
        env: &mut Self::Ref,
    ) -> Result<()>;

    fn spawn(self) -> (BctorHandle<Self::Msg>, Self::Ref);

    fn spawn_with_channel(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
    ) -> (BctorHandle<Self::Msg>, Self::Ref);

    fn spawn_with_token(
        self,
        cancellation_token: CancellationToken,
    ) -> (BctorHandle<Self::Msg>, Self::Ref);

    fn spawn_with_channel_and_token(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
        cancellation_token: CancellationToken,
    ) -> (BctorHandle<Self::Msg>, Self::Ref);

    fn run_and_handle_exit(
        self,
        env: Self::Ref,
        msg_receiver: Receiver<Self::Msg>,
    ) -> impl Future<Output = (Receiver<Self::Msg>, Result<()>)> + Send;

    fn run_till_exit(
        &mut self,
        env: &mut Self::Ref,
        msg_receiver: &mut Receiver<Self::Msg>,
    ) -> Result<()>;
}

impl<A: Bctor> BctorExt for A {
    type Ref = Ref<A>;
    type Msg = Msg<A>;

    fn handle_call_or_cast(&mut self, msg: Self::Msg, env: &mut Self::Ref) -> Result<()> {
        match msg {
            Msg::Call(msg, reply_sender) => self.handle_call(msg, env, reply_sender),
            Msg::Cast(msg) => self.handle_cast(msg, env),
        }
    }

    fn handle_continuously(
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

    fn spawn(self) -> (BctorHandle<Self::Msg>, Self::Ref) {
        let cancellation_token = CancellationToken::new();
        self.spawn_with_token(cancellation_token)
    }

    fn spawn_with_channel(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
    ) -> (BctorHandle<Self::Msg>, Self::Ref) {
        let cancellation_token = CancellationToken::new();
        self.spawn_with_channel_and_token(msg_sender, msg_receiver, cancellation_token)
    }

    fn spawn_with_token(
        self,
        cancellation_token: CancellationToken,
    ) -> (BctorHandle<Self::Msg>, Self::Ref) {
        let (msg_sender, msg_receiver) = channel(8);
        self.spawn_with_channel_and_token(msg_sender, msg_receiver, cancellation_token)
    }

    fn spawn_with_channel_and_token(
        self,
        msg_sender: Sender<Self::Msg>,
        msg_receiver: Receiver<Self::Msg>,
        cancellation_token: CancellationToken,
    ) -> (BctorHandle<Self::Msg>, Self::Ref) {
        let bctor_ref = Ref {
            msg_sender,
            cancellation_token,
        };
        let handle = {
            let env = bctor_ref.clone();
            spawn(self.run_and_handle_exit(env, msg_receiver))
        };

        (handle, bctor_ref)
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
