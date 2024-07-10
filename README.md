# Tokio GenServer

A simple Elixir/Erlang-GenServer-like actor implementation on Tokio.

Please see [the documentation] for information on usage and examples.

- Simple: Define 3 messages types and at least one callback,
    and you have an actor. No macro magic. No global "system"s or "pollution".
- Powerful: Initialization and exit callbacks.
    On completion, returns the actor itself, the whole running environment,
    and run result.
- Lightweight: Little code. No boxing except the ones introduced by Tokio.
- Blocking aCTOR (`Bctor`) for use with synchronous code.

NB: What is a GenServer?
A Generic Server is a persistent process that handles messages sent to it.
That's it.
The message can either be a cast (fire-and-forget)
or a call (request-response).

## Major difference from Erlang

- **Do not panic** (crash). Return `anyhow::Error` instead.
- Tokio scheduling is **cooperative**, not preemptive.
    Your long-running async code should call `yield_now` in the middle so
    they can be interrupted.
- Blanket **supervision** implementations are **not** very **useful**.
    Instead, let your actors spawn children actors,
    and let children send messages to their parents using `before_exit` to
    handle children exiting.

## Alternatives

[Hydra](https://github.com/dtzxporter/hydra):

- 👍 Implements "proper" "process"; interfaces closer to Erlang.
- 👍 Type-erasure on "child", "pid".
- 👍 Supervisors.
- 👍 Multi-node.
- ❓ Catches unwind (also bad because of overhead?).
- ❓ Uses flume channels: may be faster, but more dependencies.
- 👎 Requires creating "applications" and "pollutes" the codebase.
- 👎 Call, cast, and reply message types are not distinguished.
- 👎 Uses magic global variables to manage "processes".
- 👎 No blocking.
- 👎 Requires messages to be serializable.
- 👎 Most traits are not object-safe.

[the documentation]: https://docs.rs/tokio_gen_server/latest/tokio_gen_server/
