---
namespace: journal
agent_id: agent-47-readback
timestamp: 2026-05-07T17:56:17Z
status: active
tags: readback, verification, turn-47, wallclock-budget
---

# Turn 47 readback — per-attempt wallclock budget — PASS

> Authored by the parent optimizer on behalf of `cc-reality-source-of-truth-readback`. The readback subagent is intentionally read-only and has no Write tool, so it returned its findings to the parent for SMAP persistence. Content below is verbatim from the readback's verdict.

## Verdict

PASS — all six verification points hold.

## SHA chain (rounds 13 → 14 → 15 → 16)

| Round | before_sha | after_sha | bytes |
|-------|-----------|-----------|-------|
| 13 | `8cae96a6...4863a328` | `ab421342...0316c51d` | 533430 |
| 14 | `ab421342...0316c51d` | `ddbec145...1233f864` | 534816 |
| 15 | `ddbec145...1233f864` | `5b0b081a...78287808` | 536552 |
| 16 (record) | `8cae96a6...4863a328` | `5b0b081a...78287808` | n/a |

- Round 13 `after` == Round 14 `before` — MATCH.
- Round 14 `after` == Round 15 `before` — MATCH.
- Round 15 `after` == Round 16 `after` — MATCH.
- Round 16 reports `lines_added: 76, lines_removed: 0` and `cargo_check.exit_code: 0` — matches editor's claim.

## Working-tree state

`git status --porcelain` returns ONLY `M crates/context-graph-cli/src/bin/reality-loop.rs`. No untracked, no staged, no other modifications.

## Current file SHA

On-disk sha256 of `crates/context-graph-cli/src/bin/reality-loop.rs` = `5b0b081a41623e59770bacae2e3e35dffea747ccd9d14a4cfbc42fd678287808` — MATCHES round-16 `after_sha`. Size 536552 bytes.

## Diff body — three regions, +76 / -0

`git diff` confirms 3 hunks, +76 insertions, 0 deletions, all in `crates/context-graph-cli/src/bin/reality-loop.rs`:

- **Region (a)** at hunk header `@@ -4263,6 +4263,21 @@ fn inner_coding_harness_chat_json`: +15 lines adding `attempt_start = Instant::now()`, `CGREALITY_PER_ATTEMPT_DEADLINE_SECS` env parse with `.unwrap_or(2700)`, and `let attempt_deadline = Duration::from_secs(attempt_deadline_secs);`.
- **Region (b)** at hunk header `@@ -4335,6 +4350,31 @@`: +25 lines adding the post-check sibling block — `let elapsed = attempt_start.elapsed();`, `if elapsed >= attempt_deadline`, forced_stop tuple with `RUST_REALITY_ATTEMPT_BUDGET_EXCEEDED`, transcript_entries push including `elapsed_secs` + `attempt_deadline_secs`, terminating `break;`.
- **Region (c)** at hunk header `@@ -13133,4 +13173,40 @@`: +36 lines adding `#[test] fn attempt_budget_env_parses_with_sensible_default_edge_fsv` covering unset → 2700, valid "600" → 600, garbage → 2700 fallback.

15 + 25 + 36 = 76 insertions — MATCHES claim.

## Existing `MAX_ROUNDS_PER_ATTEMPT_HARD_CAP` block preserved

Read of lines 4333-4352 confirms the existing block is intact and unchanged: `if round > MAX_ROUNDS_PER_ATTEMPT_HARD_CAP { forced_stop = Some(("RUST_REALITY_MAX_ROUNDS_EXCEEDED"...))...break; }`. The new wallclock check at lines 4353-4377 is a sibling immediately following, NOT a replacement.

## No edits outside the three regions

`git diff --stat` shows `1 file changed, 76 insertions(+)` and only the three documented hunks. No other files touched, no other regions of `reality-loop.rs` modified.

## Source-of-truth pointers

- `file:/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs`
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/harness-transitions/round-13.json`
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/harness-transitions/round-14.json`
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/harness-transitions/round-15.json`
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/harness-transitions/round-16.json`
