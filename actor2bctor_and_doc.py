#!/usr/bin/env python
import os
from typing import Final
import re

NOTICE = "DO NOT modify manually! Generate with `actor2bctor_and_doc.py`."


def substitute_for_sync(text: str) -> str:
    # Exact replacements.
    text = (
        text.replace(
            "use super::*", "use std::thread::{spawn, JoinHandle}; use super::*"
        )
        .replace(
            """select! {
                m = env.msg_receiver.recv() => m,
                () = cancellation_token.cancelled() => return Ok(()),
            }""",
            "env.msg_receiver.blocking_recv()",
        )
        .replace(
            """select! {
                maybe_ok = self.handle_call_or_cast(msg, env) => maybe_ok,
                () = cancellation_token.cancelled() => return Ok(()),
            }?""",
            """match msg {
                Msg::Exit => return Ok(()) ,
                _ => self.handle_call_or_cast(msg, env)?,
            }""",
        )
        .replace("enum Msg<Call, Cast, Reply> {", "enum Msg<Call, Cast, Reply> { Exit,")
        .replace(
            """match msg {
            Msg""",
            """match msg {
            Msg::Exit => unreachable!("Exit signals should be handled before handling `handle_call_or_cast`."),
            Msg""",
        )
        .replace(
            "self.cancellation_token.cancel()",
            "_ = self.msg_sender.blocking_send(Msg::Exit)",
        )
        .replace(
            """let cancellation_token = CancellationToken::new();
        self.spawn_with_token(cancellation_token)""",
            """let (msg_sender, msg_receiver) = channel(8);
        self.spawn_with_channel(msg_sender, msg_receiver)""",
        )
        .replace(
            """

    fn spawn_with_channel(
        self,
""",
            """

    fn spawn_with_channel(
        mut self,
""",
        )
        .replace(
            """let cancellation_token = CancellationToken::new();
        self.spawn_with_channel_and_token(msg_sender, msg_receiver, cancellation_token)""",
            """let bctor_ref = Ref { msg_sender };
        let handle = {
            let mut env = Env {
                ref_: bctor_ref.clone(),
                msg_receiver,
            };
            spawn(move || {
                let exit_result = self.run_and_handle_exit(&mut env);
                BctorRunResult {
                    bctor: self,
                    env,
                    exit_result,
                }
            })
        };
        (handle, bctor_ref)""",
        )
        .replace(
            """timeout(DECI_SECOND, server_ref.call(PingOrPong::Ping)).await {
        Ok(Err(_)) |""",
            "server_ref.call(PingOrPong::Ping) {",
        )
        .replace("const DECI_SECOND: Duration = Duration::from_millis(100);", "")
        .replace(" task", " thread")
        .replace(
            """/// <summary>This trait is not object-safe.</summary>
///
/// ```compile_fail
/// use tokio_gen_server::actor::ActorRunExt;
""",
            """/// <summary>This trait is object-safe.</summary>
///
/// ```
/// use tokio_gen_server::bctor::BctorRunExt;
""",
        )
    )

    # Regex replacements.
    text = re.sub(r"\n\n(:?///.*\n)*.* fn blocking.* \{(:?\n.+)+\n {4}\}", "", text)
    text = re.sub(
        r"impl Future<Output = (['_\w()<>,: ]+)>(:? +\+ +['_\w]+)*",
        r"\1",
        text,
    )
    text = re.sub(r"receiver[\s\n]*\.await", "receiver.blocking_recv()", text)
    text = re.sub(r"\.send\((.*)\)[\s\n]*\.await", r".blocking_send(\1)", text)
    text = re.sub(
        r"timeout\([_\w]+, handle\)[\n\s]*\.await\?\?",
        "handle.join().unwrap()",
        text,
    )
    text = re.sub(
        r"\n(:?\s*///.*\n)*.*fn .*(token|join_set).*(?:\n.*[)\S])*\n", "\n", text
    )
    text = re.sub(r"(?:\n\s*/// .*)*\n.*cancellation.*\n", "\n", text)
    text = re.sub(
        r"\n\s*/// # Snippet for copying(?:\n\s*/// .*)+?\n\s*/// ```", "", text
    )
    text = re.sub(r"async { (.*) }", r"\1", text)

    # Wildcard replacements.
    text = (
        text.replace("Actor", "Bctor")
        .replace("actor", "bctor")
        .replace("async fn ", "fn ")
        .replace(".recv(", ".blocking_recv(")
        .replace(".await", "")
        .replace("#[tokio::test]", "#[test]")
    )

    return text


def transform_file(input_filename: str, output_filename: str, header: str = ""):
    with open(input_filename) as f:
        input = f.read()
    output = substitute_for_sync(input)

    with open(output_filename, "w") as f:
        _ = f.write(header + output)
    _ = os.system(f"rustfmt {output_filename}")


ACTOR_SRC_NAME: Final = "src/actor.rs"
BCTOR_SRC_NAME: Final = "src/bctor.rs"
BCTOR_HEADER: Final = f"// {NOTICE}"
MOD_DOC: Final = (
    BCTOR_HEADER
    + """
//! Blocking aCTOR. Mirrors functionalities in `actor` but blocking.
//!
//! Unlike Actors, Bctors are spawn using [`spawn`] from [`std`] and
//! cannot be cancelled during the handling of each message.
//! Bctors are supposed to be long-lived.
//!
"""
)

ACTOR_TEST_NAME: Final = "src/actor/tests.rs"
BCTOR_TEST_NAME: Final = "src/bctor/tests.rs"


def actor2bctor() -> None:
    transform_file(ACTOR_SRC_NAME, BCTOR_SRC_NAME, MOD_DOC)
    transform_file(ACTOR_TEST_NAME, BCTOR_TEST_NAME, BCTOR_HEADER + "\n")
    _ = os.system(
        "cargo clippy --all-targets --fix --allow-dirty --allow-staged --workspace"
    )
    _ = os.system("cargo fmt")


def concate_and_paste(
    input_filename: str,
    output_filename: str,
    n_cutoff_line: int = 0,
    header: str = "",
    footer: str = "",
):
    with open(input_filename) as f:
        input_lines = f.readlines()
    input_trimmed = "".join(input_lines[n_cutoff_line:])
    with open(output_filename, "w") as f:
        _ = f.write(header + input_trimmed + footer)


DOC_HEADER: Final = f"<!-- {NOTICE} -->"
ACTOR_DOC_NAME: Final = "src/actor_doc.md"
ACTOR_DOC_HEADER: Final = f"""{DOC_HEADER}
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
"""
ACTOR_DOC_FOOTER: Final = """
```

</details>

---

<details>
<summary>This trait is not object-safe.</summary>

```compile_fail
use tokio_gen_server::prelude::*;
let _: Box<dyn Actor<Call = (), Cast = (), Reply = ()>>;
```

</details>
""".lstrip()
BCTOR_DOC_NAME: Final = "src/bctor_doc.md"
BCTOR_DOC_HEADER: Final = f"""{DOC_HEADER}
# An Elixir/Erlang-GenServer-like Blocking aCTOR

`Bctor` mirrors the functionality of [`Actor`], but blocking.
Please see [`Actor`]'s documentation for the usage.

Tokio channels are used for compatibility.

## Example

<details>

```rust
"""
BCTOR_DOC_FOOTER: Final = """
```

</details>

---

<details>
<summary>This trait is object-safe.</summary>

```
use tokio_gen_server::prelude::*;
let _: Box<dyn Bctor<Call = (), Cast = (), Reply = ()>>;
```

</details>
""".lstrip()


def gen_docs() -> None:
    concate_and_paste(
        ACTOR_TEST_NAME, ACTOR_DOC_NAME, 4, ACTOR_DOC_HEADER, ACTOR_DOC_FOOTER
    )
    concate_and_paste(
        BCTOR_TEST_NAME, BCTOR_DOC_NAME, 5, BCTOR_DOC_HEADER, BCTOR_DOC_FOOTER
    )


def main() -> None:
    actor2bctor()
    gen_docs()


main() if __name__ == "__main__" else None
