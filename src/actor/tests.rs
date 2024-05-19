//! Tests are synchronized to the docstring.
use crate as tokio_gen_server;

// INSTRUCTION: When changed, run `gen_docs.py` to update `actor_doc.md`.
use anyhow::{bail, Result};
use std::time::Duration;
use tokio::{
    sync::{mpsc::Receiver, oneshot},
    time::timeout,
};
use tokio_gen_server::prelude::*;

#[derive(Debug, Default)]
struct PingPongServer {
    counter: usize,
}

#[derive(Debug)]
enum PingOrBang {
    Ping,
    Bang,
}

#[derive(Debug)]
enum PingOrPong {
    Ping,
    Pong,
}

#[derive(Debug, Eq, PartialEq)]
enum PongOrCount {
    Pong,
    Count(usize),
}

impl Actor for PingPongServer {
    type CastMsg = PingOrBang;
    type CallMsg = PingOrPong;
    type Reply = PongOrCount;

    async fn init(&mut self, _env: &mut ActorRef<Self>) -> Result<()> {
        println!("PingPongServer starting.");
        Ok(())
    }

    async fn handle_cast(&mut self, msg: Self::CastMsg, _env: &mut ActorRef<Self>) -> Result<()> {
        if matches!(msg, PingOrBang::Bang) {
            bail!("Received Bang! Blowing up.");
        }
        self.counter += 1;
        println!("Received ping #{}", self.counter);
        Ok(())
    }

    async fn handle_call(
        &mut self,
        msg: Self::CallMsg,
        _env: &mut ActorRef<Self>,
        reply_sender: oneshot::Sender<Self::Reply>,
    ) -> Result<()> {
        match msg {
            PingOrPong::Ping => {
                self.counter += 1;
                println!("Received ping #{} as a call", self.counter);
                reply_sender.send(PongOrCount::Pong).unwrap();
            }
            PingOrPong::Pong => reply_sender.send(PongOrCount::Count(self.counter)).unwrap(),
        }
        Ok(())
    }

    async fn before_exit(
        &mut self,
        run_result: Result<()>,
        _env: &mut ActorRef<Self>,
        msg_receiver: &mut Receiver<ActorMsg<Self>>,
    ) -> Result<()> {
        msg_receiver.close();
        let result_msg = match &run_result {
            Ok(()) => "successfully".into(),
            Err(why) => {
                let mut messages = Vec::new();
                while let Ok(msg) = msg_receiver.try_recv() {
                    messages.push(msg);
                }
                format!("with error `{why:?}` and disregarded messages `{messages:?}`, ")
            }
        };
        println!(
            "PingPongServer exiting {result_msg} with {} pings received.",
            self.counter
        );
        run_result.map_err(|_| anyhow::Error::msg(result_msg))
    }
}

const DECI_SECOND: Duration = Duration::from_millis(100);

#[tokio::test]
async fn ping_pong() -> Result<()> {
    let ping_pong_server = PingPongServer::default();
    let (handle, mut server_ref) = ping_pong_server.spawn();

    server_ref.cast(PingOrBang::Ping).await?;
    let pong = server_ref.call(PingOrPong::Ping).await?;
    assert_eq!(pong, PongOrCount::Pong);

    let count = server_ref.call(PingOrPong::Pong).await?;
    assert_eq!(count, PongOrCount::Count(2));

    server_ref.cancel();
    timeout(DECI_SECOND, handle).await??.1?;

    Ok(())
}

#[tokio::test]
async fn ping_pong_bang() -> Result<()> {
    let ping_pong_server = PingPongServer::default();
    let (handle, mut server_ref) = ping_pong_server.spawn();

    server_ref.cast(PingOrBang::Bang).await?;
    match timeout(DECI_SECOND, server_ref.call(PingOrPong::Ping)).await {
        Ok(Err(_)) | Err(_) => {}
        Ok(reply) => panic!("Ping Ping Server should have crashed, but got `{reply:?}`."),
    }

    let err: String = timeout(DECI_SECOND, handle)
        .await??
        .1
        .unwrap_err()
        .downcast()?;
    assert_eq!(err, "with error `Received Bang! Blowing up.` and disregarded messages `[Call(Ping, Sender { inner: Some(Inner { state: State { is_complete: false, is_closed: false, is_rx_task_set: true, is_tx_task_set: false } }) })]`, ");

    Ok(())
}
