---
namespace: decisions
created: 2026-05-09T04:10:41Z
updated: 2026-05-09T04:10:41Z
status: active
severity: high
tags: ruby, mejepa, dynamicjepa, toolchain, fail-closed
---

# Ruby Action Semantics Require `ruby -c`

## Decision

Ruby ME-JEPA live action semantics must fail closed unless the Ruby CLI is
available and the candidate snapshot can be checked with `ruby -c`.

## Why

Tree-sitter supplies parser-backed structure, but Ruby action control needs an
independent syntax source of truth for the candidate after-source. The official
Ruby CLI provides `-c` specifically to check syntax without executing the
program. Downgrading missing Ruby tooling to parser-only evidence hides a real
instrument failure.

## Required Behavior

- Missing Ruby CLI returns `RUST_REALITY_DYNAMICJEPA_RUBY_TOOLCHAIN_MISSING`.
- The error includes the candidate snapshot file as source of truth.
- The governed repository source remains unchanged; only the compiler snapshot
  receives the candidate after-source.
- Ruby semantic-index and live action tests must write and re-read evidence JSON
  plus source files.

## Related

- Discovery: ./memory/discoveries/2026-05-09--ruby-mejepa-ast-test-path-and-dsl-symbols.md
- Report: ./memory/reports/session-2026-05-09--ruby-mejepa-ast-hardening.md
