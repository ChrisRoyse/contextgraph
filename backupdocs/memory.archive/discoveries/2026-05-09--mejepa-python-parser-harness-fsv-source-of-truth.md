---
namespace: discoveries
agent_id: codex-python
timestamp: 2026-05-09T02:53:18Z
status: active
tags: mejepa, python, parser, fsv, source-of-truth
---

# ME-JEPA Python Parser Harness FSV Source Of Truth

## What Happened
The Phase-0 ME-JEPA corpus crate now validates Python mutation input with
`tree-sitter-python` before candidate scanning, uses a shared Python lexical
mask for comments/strings/docstrings/f-string replacement fields, and verifies
mutation-site byte ranges before output.

The durable source of truth for manual verification is:

- `/tmp/contextgraph-mejepa-corpus-fsv/mejepa-corpus-evidence.json`
- `/tmp/contextgraph-mejepa-corpus-fsv/*.py`
- `/tmp/contextgraph-mejepa-corpus-fsv/pyc/*.pyc`

The FSV command is:

```bash
cargo run --release -p context-graph-mejepa-corpus --example mejepa_corpus_fsv
```

## Why It Matters
This is the right verification boundary for the ME-JEPA Python harness. Return
values from `apply_mutation` are not sufficient; the generated Python files must
be written, read back, hashed, and checked by `python3 -m py_compile`.

This catches the real failure mode from the earlier stale FSV invariant: the
operator contract changed from "delete assertion bytes" to "replace assertion
bytes with `pass` to preserve Python syntax", but the evidence harness had not
been updated. The current evidence now proves `delete_test_call.py` compiles and
`compile_error.py` fails with the expected syntax error.

## Related
- `crates/context-graph-mejepa-corpus/src/python_lex.rs`
- `crates/context-graph-mejepa-corpus/src/lib.rs`
- `crates/context-graph-mejepa-corpus/examples/mejepa_corpus_fsv.rs`
- `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md`
