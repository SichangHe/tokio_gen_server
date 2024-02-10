//! Tests are synchronized to the docstring.
use crate as tokio_gen_server;

// INSTRUCTION: Copy below code to docstring every time we change it.
use anyhow::Result;
use std::time::Duration;
use tokio::{sync::oneshot, time::timeout};
use tokio_gen_server::{Actor, Ref};

#[derive(Debug, Default)]
struct PingPongServer {
    counter: usize,
}

struct Ping;

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
    type CastMsg = Ping;
    type CallMsg = PingOrPong;
    type Reply = PongOrCount;

    async fn handle_cast(&mut self, _msg: Self::CastMsg, _env: &mut Ref<Self>) -> Result<()> {
        self.counter += 1;
        println!("Received ping #{}", self.counter);
        Ok(())
    }

    async fn handle_call(
        &mut self,
        msg: Self::CallMsg,
        _env: &mut Ref<Self>,
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
}

const DECI_SECOND: Duration = Duration::from_millis(100);

#[tokio::test]
async fn ping_pong() {
    let ping_pong_server = PingPongServer::default();
    let (handle, mut server_ref) = ping_pong_server.spawn();

    server_ref.cast(Ping).await.unwrap();
    let pong = server_ref.call(PingOrPong::Ping).await.unwrap();
    assert_eq!(pong, PongOrCount::Pong);

    let count = server_ref.call(PingOrPong::Pong).await.unwrap();
    assert_eq!(count, PongOrCount::Count(2));

    server_ref.cancel();
    timeout(DECI_SECOND, handle)
        .await
        .unwrap()
        .unwrap()
        .unwrap()
}
