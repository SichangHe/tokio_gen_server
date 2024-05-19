# An Elixir/Erlang-GenServer-like bctor

## Example

```rust
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

impl Bctor for PingPongServer {
    type CastMsg = PingOrBang;
    type CallMsg = PingOrPong;
    type Reply = PongOrCount;

    fn init(&mut self, _env: &mut Ref<Self>) -> Result<()> {
        println!("PingPongServer starting.");
        Ok(())
    }

    fn handle_cast(&mut self, msg: Self::CastMsg, _env: &mut Ref<Self>) -> Result<()> {
        if matches!(msg, PingOrBang::Bang) {
            bail!("Received Bang! Blowing up.");
        }
        self.counter += 1;
        println!("Received ping #{}", self.counter);
        Ok(())
    }

    fn handle_call(
        &mut self,
        msg: Self::CallMsg,
        _env: &mut Ref<Self>,
        reply_sender: oneshot::Sender<Self::Reply>,
    ) -> Result<()> {
        match msg {
            PingOrPong::Ping => {
                self.counter += 1;
                println!("Received ping #{} as a call", self.counter);
                reply_sender.blocking_send(PongOrCount::Pong).unwrap();
            }
            PingOrPong::Pong => reply_sender.blocking_send(PongOrCount::Count(self.counter)).unwrap(),
        }
        Ok(())
    }

    fn before_exit(
        &mut self,
        run_result: Result<()>,
        _env: &mut Ref<Self>,
        msg_receiver: &mut Receiver<Msg<Self>>,
    ) -> Result<()> {
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
fn ping_pong() -> Result<()> {
    let ping_pong_server = PingPongServer::default();
    let (handle, mut server_ref) = ping_pong_server.spawn();

    server_ref.cast(PingOrBang::Ping)?;
    let pong = server_ref.call(PingOrPong::Ping)?;
    assert_eq!(pong, PongOrCount::Pong);

    let count = server_ref.call(PingOrPong::Pong)?;
    assert_eq!(count, PongOrCount::Count(2));

    server_ref.cancel();
    timeout(DECI_SECOND, handle)??.1?;

    Ok(())
}

#[tokio::test]
fn ping_pong_bang() -> Result<()> {
    let ping_pong_server = PingPongServer::default();
    let (handle, mut server_ref) = ping_pong_server.spawn();

    server_ref.cast(PingOrBang::Bang)?;
    match timeout(DECI_SECOND, server_ref.call(PingOrPong::Ping)) {
        Ok(Err(_)) | Err(_) => {}
        Ok(reply) => panic!("Ping Ping Server should have crashed, but got `{reply:?}`."),
    }

    let err: String = timeout(DECI_SECOND, handle)
        ??
        .1
        .unwrap_err()
        .downcast()?;
    assert_eq!(err, "with error `Received Bang! Blowing up.` and disregarded messages `[Call(Ping, Sender { inner: Some(Inner { state: State { is_complete: false, is_closed: false, is_rx_task_set: true, is_tx_task_set: false } }) })]`, ");

    Ok(())
}
```
