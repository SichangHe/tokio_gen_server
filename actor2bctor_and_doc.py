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
                m = receiver.recv() => m,
                () = cancellation_token.cancelled() => return Ok(()),
            }""",
            "receiver.blocking_recv()",
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
        .replace("enum Msg<L, T, R> {", "enum Msg<L, T, R> { Exit,")
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
            """let cancellation_token = CancellationToken::new();
        self.spawn_with_channel_and_token(msg_sender, msg_receiver, cancellation_token)""",
            """let bctor_ref = Ref { msg_sender };
        let handle = {
            let env = bctor_ref.clone();
            spawn(|| self.run_and_handle_exit(env, msg_receiver))
        };
        (handle, bctor_ref)""",
        )
        .replace(
            """timeout(DECI_SECOND, server_ref.call(PingOrPong::Ping)).await {
        Ok(Err(_)) |""",
            "server_ref.call(PingOrPong::Ping) {",
        )
        .replace("const DECI_SECOND: Duration = Duration::from_millis(100);", "")
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
    text = re.sub(r"\n(:?\s*///.*\n)*.*fn .*token.*(?:\n.*[)\S])*\n", "\n", text)
    text = re.sub(r"\n.*cancellation.*\n", "\n", text)

    # Wildcard replacements.
    text = (
        text.replace("Actor", "Bctor")
        .replace("actor", "bctor")
        .replace("async { Ok(()) }", "Ok(())")
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
//! Blocking actor. Mirrors functionalities in `actor` but blocking.
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

## Example

```rust
"""
BCTOR_DOC_NAME: Final = "src/bctor_doc.md"
BCTOR_DOC_HEADER: Final = f"""{DOC_HEADER}
# An Elixir/Erlang-GenServer-like Blocking aCTOR

`bctor` mirrors the functionality of [`actor`], but blocking.
Tokio channels are used for compatibility.

## Example

```rust
"""
FOOTER: Final = """
```
""".lstrip()


def gen_docs() -> None:
    concate_and_paste(ACTOR_TEST_NAME, ACTOR_DOC_NAME, 4, ACTOR_DOC_HEADER, FOOTER)
    concate_and_paste(BCTOR_TEST_NAME, BCTOR_DOC_NAME, 5, BCTOR_DOC_HEADER, FOOTER)


def main() -> None:
    actor2bctor()
    gen_docs()


main() if __name__ == "__main__" else None
