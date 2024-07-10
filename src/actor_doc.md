<!-- DO NOT modify manually! Generate with `actor2bctor_and_doc.py`. -->
# An Elixir/Erlang-GenServer-like actor

Define 3 message types and at least one callback handler on your struct to
make it an actor.

A GenServer-like actor simply receives messages and acts upon them.
A message is either a "call" (request-reply) or a "cast" (fire-and-forget).
Upon a "call" message, we call your [`Actor::handle_call`];
upon a "cast" message, we call your [`Actor::handle_cast`].
Upon cancellation or error, we call your [`Actor::before_exit`],
so you can gracefully shut down.

## Usage

1. Determine your message types.
    If your actor do not expect any "cast", set `Cast` to `()`;
    if your actor do not expect any "call",
    set both `Call` and `Reply` to `()`;
1. Implement `handle_call` and/or `handle_cast` for your actor.
1. Implement `init` and `before_exit` if needed.
1. Spawn your actor with [`ActorExt::spawn`]
    or other similar methods and get the [`ActorRef`].
1. Use [`ActorRef`] to send messages to your actor.

## Example

<details>

```rust
use anyhow::{bail, Result};
use std::time::Duration;
use tokio::{sync::oneshot, time::timeout};
use tokio_gen_server::prelude::*;

// Define the actor.
#[derive(Debug, Default)]
struct PingPongServer {
    counter: usize,
}
impl Actor for PingPongServer {
    // Message types.
    type Cast = PingOrBang;
    type Call = PingOrPong;
    type Reply = PongOrCount;

    // All the methods are optional. The default implementations does nothing.

    // `init` is called when the actor starts.
    async fn init(&mut self, _env: &mut ActorEnv<Self>) -> Result<()> {
        println!("PingPongServer starting.");
        Ok(())
    }

    // `handle_cast` is called when the actor receives a message and
    // does not need to reply.
    async fn handle_cast(&mut self, msg: Self::Cast, _env: &mut ActorEnv<Self>) -> Result<()> {
        if matches!(msg, PingOrBang::Bang) {
            bail!("Received Bang! Blowing up.");
        }
        self.counter += 1;
        println!("Received ping #{}", self.counter);
        Ok(())
    }

    // `handle_call` is called when the actor receives a message and
    // needs to reply.
    async fn handle_call(
        &mut self,
        msg: Self::Call,
        _env: &mut ActorEnv<Self>,
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

    // `before_exit` is called before the actor exits.
    async fn before_exit(
        &mut self,
        run_result: Result<()>,
        env: &mut ActorEnv<Self>,
    ) -> Result<()> {
        env.msg_receiver.close();
        let result_msg = match &run_result {
            Ok(()) => "successfully".into(),
            Err(why) => {
                let mut messages = Vec::new();
                while let Ok(msg) = env.msg_receiver.try_recv() {
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

// Now let's look at an example where we use the actor normally.
#[tokio::test]
async fn ping_pong() -> Result<()> {
    // Call `spawn` on the actor to start it.
    let ping_pong_server = PingPongServer::default();
    let (handle, server_ref) = ping_pong_server.spawn();

    // Cast a message to the actor and do not expect a reply.
    server_ref.cast(PingOrBang::Ping).await?;

    // Call the actor and wait for the reply.
    let pong = server_ref.call(PingOrPong::Ping).await?;
    assert_eq!(pong, PongOrCount::Pong);
    let count = server_ref.call(PingOrPong::Pong).await?;
    assert_eq!(count, PongOrCount::Count(2));

    // Cancel the actor.
    server_ref.cancel();

    // The handle returns the actor itself, its environment, and the run result.
    let ActorRunResult {
        actor: PingPongServer { counter: 2 },
        env: _,
        exit_result: Ok(()),
    } = timeout(DECI_SECOND, handle).await??
    else {
        panic!("Should exit normally.")
    };

    Ok(())
}

// Let's also see how the actor crashes.
#[tokio::test]
async fn ping_pong_bang() -> Result<()> {
    let ping_pong_server = PingPongServer::default();
    let (handle, server_ref) = ping_pong_server.spawn();

    // Cast the `Bang` message to crash the actor.
    server_ref.cast(PingOrBang::Bang).await?;

    // Call the actor and expect the call to fail.
    match timeout(DECI_SECOND, server_ref.call(PingOrPong::Ping)).await {
        Ok(Err(_)) | Err(_) => {}
        Ok(reply) => panic!("Ping Ping Server should have crashed, but got `{reply:?}`."),
    }

    // The receiver end of the channel can be reused after the crash.
    let ActorRunResult {
        actor: PingPongServer { counter: 0 },
        env: _,
        exit_result: Err(why),
    } = timeout(DECI_SECOND, handle).await??
    else {
        panic!("Should exit with error.")
    };

    // The exit result tells why it crashed.
    let err: String = why.downcast()?;
    assert_eq!(err, "with error `Received Bang! Blowing up.` and disregarded messages `[Call(Ping, Sender { inner: Some(Inner { state: State { is_complete: false, is_closed: false, is_rx_task_set: true, is_tx_task_set: false } }) })]`, ");

    Ok(())
}

// Data structure definitions.
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
const DECI_SECOND: Duration = Duration::from_millis(100);
```

</details>
