#!/usr/bin/env python
from typing import Final
from actor2bctor import ACTOR_TEST_NAME, BCTOR_TEST_NAME


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
        f.write(header + input_trimmed + footer)


ACTOR_DOC_NAME: Final = "src/actor_doc.md"
ACTOR_DOC_HEADER: Final = """
# An Elixir/Erlang-GenServer-like actor

## Example

```rust
""".lstrip()
BCTOR_DOC_NAME: Final = "src/actor_doc.md"
BCTOR_DOC_HEADER: Final = """
# An Elixir/Erlang-GenServer-like Blocking aCTOR

`bctor` mirrors the functionality of `actor`, but blocking.
Tokio channels are used for compatibility.

## Example

```rust
""".lstrip()
FOOTER: Final = """
```
""".lstrip()


def main():
    concate_and_paste(ACTOR_TEST_NAME, ACTOR_DOC_NAME, 4, ACTOR_DOC_HEADER, FOOTER)
    concate_and_paste(BCTOR_TEST_NAME, BCTOR_DOC_NAME, 4, BCTOR_DOC_HEADER, FOOTER)


main() if __name__ == "__main__" else None
