# Tokio GenServer

A simple Elixir/Erlang-GenServer-like actor implementation on Tokio.

Please see [the documentation](https://docs.rs/tokio_gen_server/latest/tokio_gen_server/) for more information.

- Simple: Define 3 messages types and at least one callback, and you have an actor. No macro magic. No global "system"s.
- Powerful: Initialization and exit callbacks. Returns channel receiver and run result on completion.
- Blocking aCTOR (Bactor) for use with synchronous code.

NB: What is a GenServer?
A Generic Server is a persistent process that handles messages sent to it. That's it. The message can either be a cast (fire-and-forget) or a call (request-response).
