#!/usr/bin/env python
import os
from typing import Final
import re


def substitute_for_sync(text: str) -> str:
    text = re.sub(r"\n\n(:?///.*\n)*.* fn blocking.* \{(:?\n.+)+\n {4}\}", "", text)
    text = re.sub(
        r"impl Future<Output = (['_\w()<>,: ]+)>(:? +\+ +['_\w]+)*",
        r"\1",
        text,
    )
    text = re.sub(
        r"""
select! \{
\s*(?:[_\w]+) = (.*) => (?:[_\w]+),
\s*\(\) = cancellation_token.cancelled\(\) => (.*),
\s*\}
""".strip().replace("\n", r"\n"),
        r"""match cancelled.load(Relaxed) {
        false => \1,
        true => \2,
    }""",
        text,
    )
    text = re.sub(r"receiver[\s\n]*\.await", "receiver.blocking_recv()", text)
    text = re.sub(r"\.send\((.*)\)[\s\n]*\.await", r".blocking_send(\1)", text)
    text = re.sub(
        r"timeout\([_\w]+, handle\)[\n\s]*\.await\?\?",
        "handle.join().unwrap()",
        text,
    )

    text = (
        text.replace("Actor", "Bctor")
        .replace("actor", "bctor")
        .replace("cancellation_token.cancelled()", "cancelled.load(Relaxed)")
        .replace("cancellation_token.cancel()", "cancelled.store(true, Relaxed)")
        .replace("cancellation_token", "cancelled")
        .replace("CancellationToken::new()", "Arc::new(AtomicBool::new(false))")
        .replace("CancellationToken", "Arc<AtomicBool>")
        .replace("async { Ok(()) }", "Ok(())")
        .replace("async fn ", "fn ")
        .replace("    spawn(", "    spawn(|| ")
        .replace(".recv(", ".blocking_recv(")
        .replace(".await", "")
        .replace("#[tokio::test]", "#[test]")
        .replace(
            """
timeout(DECI_SECOND, server_ref.call(PingOrPong::Ping)) {
        Ok(Err(_)) |
            """.strip(),
            "server_ref.call(PingOrPong::Ping) {",
        )
    )

    return text


def transform_file(input_filename: str, output_filename: str, header: str = ""):
    with open(input_filename) as f:
        input = f.read()
    output = substitute_for_sync(input)

    with open(output_filename, "w") as f:
        f.write(header + output)
    os.system(f"rustfmt {output_filename}")


ACTOR_SRC_NAME: Final = "src/actor.rs"
BCTOR_SRC_NAME: Final = "src/bctor.rs"
MOD_DOC: Final = """
//! Blocking actor. Mirrors functionalities in `actor` but blocking.
use std::thread::{spawn, JoinHandle};
"""

ACTOR_TEST_NAME: Final = "src/actor/tests.rs"
BCTOR_TEST_NAME: Final = "src/bctor/tests.rs"


def main():
    transform_file(ACTOR_SRC_NAME, BCTOR_SRC_NAME, MOD_DOC)
    transform_file(ACTOR_TEST_NAME, BCTOR_TEST_NAME)
    os.system("cargo clippy --fix --allow-dirty --allow-staged --workspace")
    os.system("cargo fmt")


main() if __name__ == "__main__" else None
