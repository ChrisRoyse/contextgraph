---
namespace: decisions
agent_id: agent-79-harness-editor
timestamp: 2026-05-08T04:30:00Z
status: active
tags: locked-decision, harness-edit, prompt-compression, tool-result-cap, generic, supersedes-aspect-of-2682
---

# Decision: lower TOOL_RESULT_HISTORY_CAP_BYTES from 16 KB to 4 KB

## Locked rule

`TOOL_RESULT_HISTORY_CAP_BYTES = 4 * 1024 = 4096 bytes` (~1000 tokens). Each tool result that exceeds this byte count gets truncated in the conversation history with an explicit marker. The full result is always preserved at `<tool_dir>/tool-result.json` for replay/debugging.

## Why locked

Operator-flagged: prompts grow to 200K+ tokens by round 15+, killing per-round wallclock. Turn-78 first MoE drip confirmed:
- Round 3 prompt: 40,514 tokens
- Round 3 generated: 72 tokens
- Round 3 wallclock: 11 min — **almost entirely PROMPT PREFILL, not generation**

The MoE Qwen3-Coder model emits short responses (no `<think>` tags), so the bloat source is **accumulated tool results in conversation history**. Each round adds ~16 KB to the prompt. By round 15+, results contribute 200K+ tokens.

Lowering the cap to 4 KB cuts per-round bloat by 4×: a round-15 prompt drops from ~240 KB tool results to ~60 KB, saving ~45K tokens of prefill cost per round and significantly improving wallclock at later rounds.

This SUPERSEDES the 16 KB default established earlier (line 2682, dated comment block 2670-2681).

## What changed (concrete edit)

`crates/context-graph-cli/src/bin/reality-loop.rs:2666-2685`:
- Doc-comment updated to describe 32K-token budget instead of 100K-token budget
- "16 KB ≈ 4000 tokens" → "4 KB ≈ 1000 tokens"
- "open_file_window of ~250 source lines" → "open_file_window of ~60 source lines"
- "directory listing of ~200 files" → "directory listing of ~50 files"
- Added provenance note: "Lowered from 16 KB on 2026-05-08 (turn 79)..."
- `const TOOL_RESULT_HISTORY_CAP_BYTES: usize = 16 * 1024;` → `4 * 1024`

New `reality-loop.rs` SHA: `sha256:8bbbe5be02cac0235edf71796ae6b1b210b366d16d4d97401ef9bdf7053a63c2`. Net diff: 1 byte/value change + comment refresh.

## Verification (turn 79)

| Check | Result |
|---|---|
| `cargo check -p context-graph-cli` | PASS (0 errors, 5 pre-existing dead-code warnings in `governed_edit.rs`) |
| `cargo test -p context-graph-cli --lib` | 229 passed, 2 failed (pre-existing: `governed_path_policy_*` tests for the gate that was removed 2026-05-07; same failures on main HEAD; UNRELATED to this edit) |
| `cargo build --release -p context-graph-cli` | PASS (1m21s, no new warnings) |
| `target/release/reality-loop` mtime | rebuilt this edit |
| Source-of-truth readback | matches spec exactly |

## Why this is GENERIC

Mental check: would this same change help on `psf__requests-2317`, `django__django-12286`, `sympy__sympy-13895`? **YES** — tool-result cap applies to every SWE-bench task. Trade-off (smaller per-result snippet) is uniform. Model can re-call open_file_window with tighter scope if needed; full result always at `<tool_dir>/tool-result.json`.

## Trade-off + mitigation

Smaller cap means file slices in history get cut to ~60 lines (vs 250 prior). If model needs more, it must call `open_file_window` again with tighter `start_line`/`end_line`. The truncation marker tells the model exactly how to recover. Cost: occasional extra tool call. Benefit: massively faster prefill, more rounds within wallclock budget.

For longer file analysis, model can use `search_repo` (terser results) to find specific patterns, then targeted `open_file_window` for narrow ranges.

## Operational notes

- Engine PID 870206 (still running, etime 37+ min) uses the OLD binary with 16 KB cap. New cap takes effect on next drip.
- Pre-existing 2 test failures (path-policy gate) are tracked separately; not introduced by this edit.

## Source of truth

- `file:/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs#L2666-L2685`
- `file:/tmp/contextgraph-rust-reality-loop/rust_reality_1778211124503226686_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/harness-transitions/round-02.json`
- `file:./memory/reports/agent-78-coordinator--turn-78-final-report.md` (round-3 prompt_tokens=40,514 empirical observation)
