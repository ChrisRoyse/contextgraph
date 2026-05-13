---
namespace: decisions
agent_id: agent-73-harness-editor
timestamp: 2026-05-08T01:15:00Z
status: superseded
superseded_by: 2026-05-09--single-outer-loop-mejepa-no-local-llm.md
tags: locked-decision, harness-edit, wallclock-budget, default-tuning, generic, supersedes-aspect-of-agent-47
---

# Decision: raise CGREALITY_PER_ATTEMPT_DEADLINE_SECS default from 2700 (45 min) to 7200 (120 min)

## Locked rule

The default per-attempt wallclock budget is **7200 seconds (120 minutes)**. Override via `CGREALITY_PER_ATTEMPT_DEADLINE_SECS` env var. The 7200 default is sized for the observed hardware/model combo (RTX 5090 + Qwen3.6-27B Q6_K_XL at 10-25 min/round), giving 5-12 rounds — enough for a complete trajectory of: read_problem_reality + open_file_window(s) + apply_*_edit + run_targeted_test before clean abort.

This SUPERSEDES the 2700 default established in `decisions/agent-47-harness-editor--per-attempt-wallclock-budget.md`. The 2700 cap fired correctly (validating turn-47's structural intervention) but was empirically too tight: only 4 rounds completed on the validation drip, model never reached the apply phase.

## Why locked

Single empirical data point (turn-72 drip: 4 rounds in 56 min, no patch submitted) shows the 2700 cap is below the minimum useful trajectory length on this hardware. The previous 7-hour attempt that produced the canonical correct fix used ~14-26 min/round, hitting MAX_ROUNDS=20 not the wallclock cap. So the wallclock budget needs to span 5-12 rounds at the 10-25 min/round range = 50-300 min. Picking 120 min as a balanced midpoint — generous enough to allow a full trajectory, tight enough to abort clearly-stuck attempts.

## What changed (concrete edit)

In `crates/context-graph-cli/src/bin/reality-loop.rs`:

| Line | Change |
|---|---|
| 4272-4275 | Comment block: "Default 2700s (45 min)" → "Default 7200s (120 min)" |
| 4279 | `.unwrap_or(2700)` → `.unwrap_or(7200)` + updated inline rationale |
| 4366 | forced_stop remediation: "default 2700 = 45 min" → "default 7200 = 120 min" |
| 13178 | test doc-comment: "2700s (45 min)" → "7200s (120 min)" with updated rationale |
| 13186-13192 | unset-env-var test: 2700 → 7200 (assert + fallback) |
| 13196-13199 | env-var-override test fallback: 2700 → 7200 |

Two intentional historical mentions of "2700" remain in comments (4279 and 13178) noting "raised from 2700".

New `reality-loop.rs` SHA: `sha256:5db25febf4bb71eec237e982b32c259067ea20cbb94bc5b53b1282294a118c1f`. Net diff +6/-6.

## Verification (turn 73)

| Check | Result |
|---|---|
| `cargo check -p context-graph-cli` | PASS (1.7s, 0 errors) |
| `cargo test -p context-graph-cli` | PASS (38 passed, 0 failed, 1 ignored — `attempt_budget_env_parses_with_sensible_default_edge_fsv` passes with new 7200 default) |
| `cargo build --release -p context-graph-cli` | PASS (24.7s, no new warnings) |
| `target/release/reality-loop` mtime | `2026-05-07 20:15:22 -0500` |
| Source-of-truth readback (lines 4279, 4366) | matches spec exactly |
| `cargo fmt` | NOT run — pre-existing rustfmt drift in unrelated `harness_*` async fn signatures |

## Why this is GENERIC

Mental check: would this same change help on `psf__requests-2317`, `django__django-12286`, `sympy__sympy-13895`? **YES** — same hardware, same model, same generation rate. The wallclock budget is per-attempt, not per-task. No task-specific knowledge baked in.

## Operational notes

- The new binary at `target/release/reality-loop` (mtime 20:15:22) is staged for the NEXT drip via the MCP→CLI shim.
- This default is for HARDWARE that yields 10-25 min/round. If a different model or different hardware produces faster generation (say 2-5 min/round), this default is overly generous. Override via env var as appropriate.
- The previous attempt that took 7+ hours hit MAX_ROUNDS_PER_ATTEMPT_HARD_CAP=20, not the wallclock cap. So even with 7200 budget, the round-count cap is the FIRST line of defense for runaway attempts.

## Source of truth

- `file:/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs#L4279`
- `file:/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs#L4366`
- `file:/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs#L13178-L13211`
- `file:/home/cabdru/contextgraph/memory/discoveries/agent-72-coordinator--wallclock-cap-too-tight-for-this-hardware.md`
- `file:/tmp/contextgraph-rust-reality-loop/rust_reality_1778198277123757517_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/harness-transitions/round-02.json`
- `file:/tmp/contextgraph-rust-reality-loop/rust_reality_1778198277123757517_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/harness-transitions/round-03.json`
- `file:/tmp/contextgraph-rust-reality-loop/rust_reality_1778198277123757517_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/harness-transitions/round-04.json`
