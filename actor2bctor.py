import os
from typing import Final
import re

MOD_DOC: Final = (
    "//! Blocking actor. Mirrors functionalities in `actor` but blocking.\n"
)
ACTOR_SRC_NAME: Final = "src/actor.rs"
BCTOR_SRC_NAME: Final = "src/bctor.rs"


def main():
    with open(ACTOR_SRC_NAME) as f:
        actor_src = f.read()
    bctor_src = (
        actor_src.replace("Actor", "Bctor")
        .replace("actor", "bctor")
        .replace("async { Ok(()) }", "Ok(())")
        .replace("async fn ", "fn ")
    )
    bctor_src = re.sub(
        r"\n\n(:?///.*\n)*.* fn blocking.* \{(:?\n.+)+\n {4}\}", "", bctor_src
    )
    bctor_src = re.sub(
        r"impl Future<Output = (['_\w()<>, ]+)>(:? +\+ +['_\w]+)*",
        r"\1",
        bctor_src,
    )

    bctor_src = re.sub(r"receiver(:?\n *).await", "receiver.blocking_recv()", bctor_src)
    bctor_src = bctor_src.replace(".send(", ".blocking_send(").replace(".await", "")

    with open(BCTOR_SRC_NAME, "w") as f:
        f.write(MOD_DOC + bctor_src)
    os.system(f"rustfmt {BCTOR_SRC_NAME}")


main() if __name__ == "__main__" else None
