---
namespace: reports
agent_id: codex-ruby
timestamp: 2026-05-09T05:04:00Z
status: complete
tags: ruby,mejepa,dynamicjepa,ast,semantic-index,fsv,fail-closed
---

# Ruby ME-JEPA AST Harness Hardening Report

## Scope

Focused only on Ruby AST parser/action hardening for the ME-JEPA DynamicJEPA
prediction system. Concurrent non-Ruby workspace changes were left untouched.

## Root Cause Summary

- Ruby live action attribution used generic test-path detection, so `spec/`,
  `test/`, `_spec.rb`, and `_test.rb` edits were not classified as test-path
  actions.
- Ruby live action call symbols used the generic first identifier fallback, so
  receiver-qualified DSL calls like `RSpec.describe` could persist `RSpec`
  instead of the actual test DSL method `describe`.
- Missing Ruby CLI support in live action semantics was downgraded to
  parser-only `toolchain_missing` evidence instead of failing closed.
- Manual semantic-index readback exposed anonymous Ruby keyword tokens:
  tree-sitter anonymous `class`/`module` keyword children were classified as
  class/module symbols, producing bogus `anonymous@...` entries.

## Fixes

- Ruby action node naming now extracts call names from Ruby `method` fields
  before falling back to generic naming.
- Ruby test-path detection includes `spec/`, `test/`, `_spec.rb`, and
  `_test.rb`, which drives `E_Test`, test attribution, and
  `test_assertion` patch-delta features.
- Ruby live action semantics require the real Ruby CLI and `ruby -c`; missing
  Ruby returns `RUST_REALITY_DYNAMICJEPA_RUBY_TOOLCHAIN_MISSING` with the
  candidate snapshot file as source of truth.
- Ruby class/module classification in both semantic-index and live action paths
  now requires `node.is_named()`, preventing anonymous keyword tokens from
  becoming persisted symbols.

## Research Applied

- tree-sitter documents static node types and the distinction between named
  AST nodes and anonymous tokens; this supports requiring `node.is_named()` for
  persisted Ruby class/module symbols.
- Ruby's official CLI options document `-c` as syntax-check mode, which is the
  correct non-executing compiler check for Ruby source snapshots.
- RSpec and Minitest docs confirm the real Ruby test shapes used in fixtures:
  RSpec `describe`/`it` DSL calls and Minitest `test_` methods.

## Source Of Truth

Final CLI happy-path source of truth:

- `/tmp/contextgraph-ruby-ast-fsv/final-cli/happy/semantic-index.json`
- `/tmp/contextgraph-ruby-ast-fsv/final-cli/happy/readback-summary.json`
- `/tmp/contextgraph-ruby-ast-fsv/final-cli/happy/semantic-index.json.evidence/`

Readback facts:

- `languages`: `["ruby"]`
- `source_file_count`: `3`
- symbols: `Calculator`, `add`, `describe`, `it`, `CalculatorTest`,
  `test_adds`, `helper_value`
- test symbols: `describe`, `it`, `test_adds`
- `call_count`: `6`
- `no_anonymous_symbols`: `true`
- Ruby syntax evidence: all three happy-path Ruby files emitted `Syntax OK`

Final action/semantic replay logs:

- `/tmp/contextgraph-ruby-ast-fsv/final/ruby-semantic-tests.log`
- `/tmp/contextgraph-ruby-ast-fsv/final/ruby-action-tests.log`
- `/tmp/contextgraph-ruby-ast-fsv/final/ruby-action-missing-toolchain.log`

Manual edge source-of-truth:

- Invalid syntax:
  `/tmp/contextgraph-ruby-ast-fsv/final-cli/edge-invalid-syntax/`
  - before: one Ruby file, no output/evidence
  - after: status `1`, output absent, evidence dir present
  - source unchanged after separate readback
  - stderr includes Ruby `SyntaxError`
- Missing Ruby CLI:
  `/tmp/contextgraph-ruby-ast-fsv/final-cli/edge-missing-ruby/`
  - before: one Ruby file, no output/evidence
  - after: status `1`, output absent, evidence dir present
  - source unchanged after separate readback
  - stderr includes `missing ["ruby"]`
- Empty Ruby repo:
  `/tmp/contextgraph-ruby-ast-fsv/final-cli/edge-empty-repo/`
  - before: zero Ruby files, no output/evidence
  - after: status `1`, output absent, evidence dir present
  - stderr includes `requested language ruby was not present`

## Verification Commands

- `cargo fmt --all --check`
- `git diff --check -- crates/context-graph-cli/src/bin/reality-loop.rs crates/context-graph-cli/src/commands/dynamicjepa.rs docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md vendor/llama-cpp-sys-2-0.1.145/build.rs`
- `target/debug/deps/context_graph_cli-425ba30a0168ed8d ruby_semantic --nocapture --test-threads=1`
- `target/debug/deps/reality_loop-fd57860eb98cbc76 ruby --nocapture --test-threads=1`
- `env PATH=/tmp/contextgraph-ruby-ast-fsv/final/no-ruby-path/bin target/debug/deps/reality_loop-fd57860eb98cbc76 dynamicjepa_ast_action_semantics_parse_real_ruby_spec_edit_with_ruby_c --nocapture --test-threads=1`
- `target/debug/context-graph-cli dynamicjepa build-semantic-index --repo /tmp/contextgraph-ruby-ast-fsv/final-cli/happy/repo --output /tmp/contextgraph-ruby-ast-fsv/final-cli/happy/semantic-index.json --language ruby --max-files 20 --json`
- `target/debug/context-graph-cli dynamicjepa build-semantic-index --repo /tmp/contextgraph-ruby-ast-fsv/final-cli/edge-invalid-syntax/repo --output /tmp/contextgraph-ruby-ast-fsv/final-cli/edge-invalid-syntax/semantic-index.json --language ruby --max-files 20 --json`
- `env PATH=/tmp/contextgraph-ruby-ast-fsv/final-cli/edge-missing-ruby/path/bin target/debug/context-graph-cli dynamicjepa build-semantic-index --repo /tmp/contextgraph-ruby-ast-fsv/final-cli/edge-missing-ruby/repo --output /tmp/contextgraph-ruby-ast-fsv/final-cli/edge-missing-ruby/semantic-index.json --language ruby --max-files 20 --json`
- `target/debug/context-graph-cli dynamicjepa build-semantic-index --repo /tmp/contextgraph-ruby-ast-fsv/final-cli/edge-empty-repo/repo --output /tmp/contextgraph-ruby-ast-fsv/final-cli/edge-empty-repo/semantic-index.json --language ruby --max-files 20 --json`

## Notes

The provided sudo credential was not used or stored. A post-hoc Cargo rebuild
attempt timed out while compiling unrelated native `llama-cpp-sys` dependencies
after concurrent vendor changes; final verification used already-built test and
CLI binaries plus direct source-of-truth readbacks.
