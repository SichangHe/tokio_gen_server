<!-- DO NOT modify manually! Generate with `actor2bctor_and_doc.py`. -->
# An Elixir/Erlang-GenServer-like Blocking aCTOR

`Bctor` mirrors the functionality of [`Actor`], but blocking.
Please see [`Actor`]'s documentation for the usage.

Tokio channels are used for compatibility.

## Example

<details>

```rust
use anyhow::{bail, Result};
use tokio::sync::oneshot;
use tokio_gen_server::prelude::*;

// Define the bctor.
#[derive(Debug, Default)]
struct PingPongServer {
    counter: usize,
}
impl Bctor for PingPongServer {
    // Message types.
    type Cast = PingOrBang;
    type Call = PingOrPong;
    type Reply = PongOrCount;

    // All the methods are optional. The default implementations does nothing.

    // `init` is called when the bctor starts.
    fn init(&mut self, _env: &mut BctorEnv<Self>) -> Result<()> {
        println!("PingPongServer starting.");
        Ok(())
    }

    // `handle_cast` is called when the bctor receives a message and
    // does not need to reply.
    fn handle_cast(&mut self, msg: Self::Cast, _env: &mut BctorEnv<Self>) -> Result<()> {
        if matches!(msg, PingOrBang::Bang) {
            bail!("Received Bang! Blowing up.");
        }
        self.counter += 1;
        println!("Received ping #{}", self.counter);
        Ok(())
    }

    // `handle_call` is called when the bctor receives a message and
    // needs to reply.
    fn handle_call(
        &mut self,
        msg: Self::Call,
        _env: &mut BctorEnv<Self>,
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

    // `before_exit` is called before the bctor exits.
    fn before_exit(&mut self, run_result: Result<()>, env: &mut BctorEnv<Self>) -> Result<()> {
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

// Now let's look at an example where we use the bctor normally.
#[test]
fn ping_pong() -> Result<()> {
    // Call `spawn` on the bctor to start it.
    let ping_pong_server = PingPongServer::default();
    let (handle, server_ref) = ping_pong_server.spawn();

    // Cast a message to the bctor and do not expect a reply.
    server_ref.cast(PingOrBang::Ping)?;

    // Call the bctor and wait for the reply.
    let pong = server_ref.call(PingOrPong::Ping)?;
    assert_eq!(pong, PongOrCount::Pong);
    let count = server_ref.call(PingOrPong::Pong)?;
    assert_eq!(count, PongOrCount::Count(2));

    // Cancel the bctor.
    server_ref.cancel();

    // The handle returns the bctor itself, its environment, and the run result.
    let BctorRunResult {
        bctor: PingPongServer { counter: 2 },
        env: _,
        exit_result: Ok(()),
    } = handle.join().unwrap()
    else {
        panic!("Should exit normally.")
    };

    Ok(())
}

// Let's also see how the bctor crashes.
#[test]
fn ping_pong_bang() -> Result<()> {
    let ping_pong_server = PingPongServer::default();
    let (handle, server_ref) = ping_pong_server.spawn();

    // Cast the `Bang` message to crash the bctor.
    server_ref.cast(PingOrBang::Bang)?;

    // Call the bctor and expect the call to fail.
    match server_ref.call(PingOrPong::Ping) {
        Err(_) => {}
        Ok(reply) => panic!("Ping Ping Server should have crashed, but got `{reply:?}`."),
    }

    // The receiver end of the channel can be reused after the crash.
    let BctorRunResult {
        bctor: PingPongServer { counter: 0 },
        env: _,
        exit_result: Err(why),
    } = handle.join().unwrap()
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
```

</details>
