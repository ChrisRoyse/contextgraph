---
namespace: discoveries
created: 2026-05-09T04:10:41Z
updated: 2026-05-09T04:44:00Z
status: active
severity: high
tags: ruby, mejepa, ast-parser, dynamicjepa, fsv, fail-closed
---

# Ruby ME-JEPA AST Harness Missed Live Test Signals

## What Happened

The Ruby semantic-index path already recognized RSpec DSL calls and Minitest
`test_` methods, but the live action path still had two Ruby gaps:

- generic `path_is_test` omitted Ruby `spec/`, `test/`, `_spec.rb`, and
  `_test.rb`, so Ruby spec edits were not attributed as test-path actions and
  `expect` edits did not become `test_assertion` patch deltas;
- live action symbol names used the generic first-identifier fallback for Ruby
  calls, which can persist the receiver (`RSpec`) instead of the DSL method
  (`describe`) for `RSpec.describe`;
- missing `ruby` CLI support returned parser-only evidence with
  `toolchain_missing`, which is a fallback and not acceptable for ME-JEPA action
  control.
- manual semantic-index readback later showed anonymous Ruby keyword tokens
  (`class`, `module`) were being visited as if they were named class/module AST
  nodes, producing bogus `anonymous@...` class symbols in persisted evidence.

## Why It Matters

ME-JEPA prediction panels use `E_AST`, `E_Test`, patch-delta, and verifier-target
features. A Ruby spec edit misclassified as a generic call/action teaches the
predictor the wrong action intent, and parser-only Ruby action evidence can hide
that the real syntax checker was unavailable.

Persisting anonymous symbols from Ruby keyword tokens pollutes the symbol graph
and reverse-reference features. Ruby class/module classification must require
`node.is_named()` so only real tree-sitter Ruby `class`/`module` nodes become
symbols.

## Related

- Decision: ./memory/decisions/2026-05-09--ruby-action-semantics-require-ruby-c.md
- Report: ./memory/reports/session-2026-05-09--ruby-mejepa-ast-hardening.md
