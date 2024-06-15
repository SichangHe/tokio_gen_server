# Tokio GenServer

A simple Elixir/Erlang-GenServer-like actor implementation on Tokio.

Please see [the documentation] for more information.

- Simple: Define 3 messages types and at least one callback,
    and you have an actor. No macro magic. No global "system"s or "pollution".
- Powerful: Initialization and exit callbacks.
    Returns channel receiver and run result on completion.
- Blocking aCTOR (Bactor) for use with synchronous code.

NB: What is a GenServer?
A Generic Server is a persistent process that handles messages sent to it.
That's it.
The message can either be a cast (fire-and-forget)
or a call (request-response).

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
