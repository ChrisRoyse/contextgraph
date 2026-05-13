---
namespace: reports
created: 2026-05-09T05:49:19Z
updated: 2026-05-09T05:49:19Z
status: active
tags: ast, tree-sitter, samplecode, fsv, ruby, context-graph-cli, hardening
---

# AST System Sample Corpus Hardening FSV

## What Happened

Reviewed the current ContextGraph tree-sitter AST implementation and hardened `/home/cabdru/contextgraph/samplecode` into a real multi-language AST verification corpus. The standalone sample harness now exercises every supported grammar tag used by the AST system:

- python
- rust
- javascript
- typescript
- tsx
- go
- java
- c
- cpp
- csharp
- ruby
- php

The sample corpus was expanded with harder constructs per language, including TSX JSX, Go test/benchmark/fuzz/example entrypoints, Java records/interfaces, C/C++ macros/templates/namespaces, C# delegates/operators/events/indexers, Ruby Minitest/RSpec-style calls, and PHP attributes/test-style calls.

The sample harness writes its source-of-truth evidence to:

```text
/home/cabdru/contextgraph/samplecode/target/ast-sample-evidence.json
```

It immediately reads the evidence file back and fails if the readback diverges from the in-memory evidence.

## Full State Verification

Manual FSV source of truth:

```text
/home/cabdru/contextgraph/samplecode/target/ast-sample-evidence.json
```

Durable manual FSV log:

```text
/tmp/contextgraph-ast-system-fsv/manual-fsv.log
/tmp/contextgraph-ast-system-fsv/ast-sample-evidence.json
```

Artifact hashes:

```text
d4c5befb4290e5b604941f151b117a41f66c81e014d06a3162e4ba9351a7d2c1  /tmp/contextgraph-ast-system-fsv/manual-fsv.log
b462e5c1b3c6b67ebb553705a31b2dd352a49a797520c499f6fa1fefabadf25e  /tmp/contextgraph-ast-system-fsv/ast-sample-evidence.json
```

Evidence summary:

```json
{
  "sample_count": 13,
  "pass_count": 13,
  "edge_case_count": 3,
  "edge_pass_count": 3,
  "failures": [],
  "edge_failures": []
}
```

Manual edge cases printed before/after state:

- `empty_tsx_refused`: whitespace-only TSX input is refused instead of silently accepted as meaningful AST data.
- `invalid_python_recovery`: malformed Python source produces a parser recovery diagnostic with a `MISSING ")"` node.
- `invalid_csharp_recovery`: malformed C# source produces a parser recovery diagnostic with a `MISSING ")"` node.

Real language tool checks run against the actual sample files:

- `python3 -m py_compile sample.py`
- `cargo check --manifest-path Cargo.toml`
- `./node_modules/.bin/tsc --noEmit --pretty false`
- `go test ./...`
- `javac Sample.java`
- `gcc -std=c11 -Wall -Wextra -fsyntax-only sample.c`
- `g++ -std=c++17 -Wall -Wextra -fsyntax-only sample.cpp`
- `dotnet build SampleCode.csproj --nologo`
- `ruby -c sample.rb`
- `php -l sample.php`

All passed.

## Root Cause Notes

The concrete AST classification issue found during this pass was Ruby import classification. `tree-sitter-ruby` represents normal `require` and `require_relative` statements as `call` nodes with callee names, not as dedicated `require` / `require_relative` node kinds. The current `dynamicjepa` code and the tracked `reality-loop.rs` in `HEAD` both contain callee-name checks for this shape:

- `semantic_ruby_call_is_require`
- `ast_ruby_call_is_require`

The working tree currently stages deletion of `crates/context-graph-cli/src/bin/reality-loop.rs`, so live-action AST verification against that file cannot be completed without deciding whether that deletion is intentional. I did not restore it.

## Blocker

Repo-level `context-graph-cli` Cargo tests are blocked by staged/pre-existing vendored llama source deletions. The focused AST test failed before Rust compilation inside `llama-cpp-sys-2` because required ggml CUDA source files are absent. Logged separately:

```text
memory/blockers/2026-05-09--context-graph-cli-tests-blocked-by-vendored-llama-deletions.md
```

## Why It Matters

The sample harness is now a repeatable AST compatibility gate: supported grammars must parse real code, cover required node kinds, fail closed on syntax recovery, and leave a durable evidence file. This gives future AST changes a concrete source of truth instead of relying on parser return values alone.

## Related

- See also: ./memory/discoveries/2026-05-09--ruby-require-is-call-node.md
- See also: ./memory/discoveries/2026-05-09--tree-sitter-abi-runtime-and-mejepa-fsv-hardening.md
- See also: ./memory/blockers/2026-05-09--context-graph-cli-tests-blocked-by-vendored-llama-deletions.md
- Tags: ast, tree-sitter, samplecode, fsv, ruby, context-graph-cli, hardening
