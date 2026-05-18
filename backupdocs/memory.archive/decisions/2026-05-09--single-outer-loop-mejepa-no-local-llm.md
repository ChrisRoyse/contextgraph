---
namespace: decisions
created: 2026-05-09T06:00:00Z
updated: 2026-05-09T06:18:25Z
status: active
tags: mejepa, single-loop, local-llm-retirement, reality-loop, swe-bench
---

# Decision: single outer loop trains ME-JEPA; local inner LLM is retired

- Status: accepted
- Date: 2026-05-09
- Agent: Codex

## Decision

ContextGraph is now a single-loop system around Claude Code. The loop exists to
capture durable reality evidence for ME-JEPA-Code training and verification.

The old inner/local LLM actor is not part of the product, not part of the
runtime contract, and should not remain in the default build surface. The
ME-JEPA deliverable is a non-generative predictor/verifier over real code
changes, repository context, observed test/oracle outcomes, durable storage
rows, witness/shift logs, and failed-run records.

## Root Cause

The repository still carried two conflicting architectures:

1. A retired Claude-plus-local-Qwen/Hermes two-loop reality harness.
2. The post-pivot ME-JEPA reality predictor architecture.

That conflict caused real hardening failures: unconditional `llama-cpp-2`
dependencies pulled native llama/CUDA builds into unrelated checks, stale
hooks still tried to build `--bin reality-loop`, and docs still implied the
inner actor could generate training signal.

## Consequences

- Remove the `reality-loop` binary and local llama dependencies from the active
  build graph.
- Retain only one reality loop: Claude Code work produces real artifacts and
  reward labels for ME-JEPA.
- Retired MCP/run entrypoints must fail closed with structured errors instead
  of attempting stale binaries.
- SWE-bench remains the oracle/corpus source; Claude Code is the patch producer.
- ME-JEPA training rows must be auditable bundles with independent readback,
  not model self-reports.

## Verification Target

Source of truth for this decision is the repository state:

- `Cargo.toml` and crate manifests no longer require `llama-cpp-2`.
- `crates/context-graph-cli/src/bin/reality-loop.rs` is absent.
- Hooks/tests no longer require `cargo check -p context-graph-cli --bin reality-loop`.
- Planning docs and `.claude/loop.md` describe one outer Claude Code loop that
  feeds ME-JEPA-Code.
