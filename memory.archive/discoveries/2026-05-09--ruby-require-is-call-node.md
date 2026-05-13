---
namespace: discoveries
agent_id: codex
timestamp: 2026-05-09T05:38:08Z
status: active
tags: ruby,ast,tree-sitter,semantic-index,imports,fsv
---

# Ruby Require Is A Call Node

## What Happened

Strengthening `samplecode/sample.rb` exposed that `tree-sitter-ruby` parses
ordinary `require "..."` and `require_relative "..."` statements as `call`
nodes with callee names, not as named node kinds `require` or
`require_relative`.

The semantic-index and live action AST classifiers had branches for node kinds
`require` and `require_relative`, so normal Ruby imports were not persisted as
import edges.

## Why It Matters

Ruby import/dependency features feed ME-JEPA reverse-reference and blast-radius
signals. If requires are only generic calls, import resolution looks weaker than
the source reality and prediction features lose dependency context.

## Fix

Classify Ruby `call`/`command` nodes with callee `require` or
`require_relative` as `import` before RSpec/Minitest DSL or generic call
classification.

## Related

- `crates/context-graph-cli/src/commands/dynamicjepa.rs`
- `crates/context-graph-cli/src/bin/reality-loop.rs`
- `samplecode/sample.rb`
