---
namespace: decisions
agent_id: agent-58-harness-editor
timestamp: 2026-05-07T20:18:00Z
status: superseded
superseded_by: 2026-05-09--single-outer-loop-mejepa-no-local-llm.md
tags: locked-decision, harness-edit, ux-clarification, generic, apply-tools, candidate-patch-semantics
---

# Decision: post-apply action messages must explicitly state UPDATE-semantics + reset-to-base + REPLACE-not-stack

## Locked rule

The action-string field in the tool-result of every patch-submission tool (`apply_structured_edit`, `apply_line_window_edit`, `apply_unified_diff`) MUST tell the model THREE things explicitly:

1. **Candidate patch UPDATED** — this submission has become the current candidate.
2. **Workspace reset to base** — subsequent `open_file_window` will return BASE file content, not the model's edit. The `post_edit_window` field in this response is the authoritative view of the current candidate.
3. **REPLACE, not stack** — calling any `apply_*` tool again REPLACES the current candidate; submissions do not accumulate. The current candidate is what runs in the official Docker eval at end-of-attempt.

If any future tool surface adds another patch-submission entry point, it MUST follow this same rule.

## Why this is locked

The model is operating in a state machine the harness understands but the model cannot directly observe. The post-extract reset is a CHOICE made for safety (so subsequent rounds can apply test commands without a dirty tree), but the side-effect — `open_file_window` returning BASE instead of post-edit content — looks identical to "your edit was lost" from the model's perspective. Without explicit guidance, the model concludes its first edit failed and re-submits, often with a worse patch.

Confirmed empirically in `astropy__astropy-12907` attempt-1 (this session, see `memory/discoveries/agent-57-coordinator--workspace-reset-confuses-model-into-resubmitting.md`):
- Round-09: `apply_line_window_edit:ok` writes a reasonable (if imperfect) patch.
- Round-10: `open_file_window` returns base SHA `cac38fe974...` (matches the round-09 `observed_sha256` exactly).
- Round-11: model submits a STRICTLY WORSE patch (deletes `cright = np.zeros(...)` initialization), believing its first edit was lost.
- The attempt-level `candidate.patch` (`reality-loop.rs:1839`) is overwritten by round-11's broken patch and is what the official Docker eval runs.

## What changed (concrete edit)

Three string changes in `crates/context-graph-cli/src/bin/reality-loop.rs`:

| Line | Tool | Before | After |
|---|---|---|---|
| 6268 | `apply_structured_edit` | "compiled structured edits against real checkout, generated patch preview, then reset workspace to base commit" | "candidate patch UPDATED with this structured edit. Workspace reset to base per harness contract — your next open_file_window will return BASE FILE CONTENT (not your edit). The post_edit_window field in this response is the AUTHORITATIVE view of your current candidate. Calling any apply_* tool again REPLACES the current candidate, it does not stack. The current candidate runs in the official Docker eval at end-of-attempt." |
| 6362 | `apply_line_window_edit` | "compiled line-window edits against real checkout, generated patch preview, then reset workspace to base commit" | (same as above, with "line-window edit" instead of "structured edit") |
| 6434 | `apply_unified_diff` | "applied unified diff against real checkout, verified observed file hashes, generated normalized candidate patch, then reset workspace to base commit" | (same as above, with "unified diff" and "post_edit_window field (if present)" — apply_unified_diff doesn't always produce a post_edit_window) |

Net source diff: +3/-3 lines, 1 file. New `reality-loop.rs` SHA: `sha256:32490d1fe85832fa33112af598a733e7c2ebfb3079c7ded9622e94ff4c06dd58`.

## Why this is GENERIC

Mental check passes: the same change helps the inner actor on `psf__requests-2317`, `django__django-12286`, `sympy__sympy-13895`, and every other SWE-bench instance because the workspace-reset behavior is universal. No astropy-specific or modeling-specific knowledge baked in.

## Verification (turn 58)

| Check | Result |
|---|---|
| `cargo check` | PASS (5 pre-existing warnings in `governed_edit.rs`, unrelated) |
| `cargo test` | PASS (38 passed, 0 failed, 1 ignored) |
| `cargo build --release -p context-graph-cli` | PASS (31.78s, no new warnings) |
| `target/release/reality-loop` mtime | `2026-05-07 15:39:30 -0500` (rebuilt this turn) |
| Source-of-truth readback (3 lines) | matches spec EXACTLY |
| `mcp__cgreality__reality_shift_compare_to_my_view` | drift:`unseen_file`, no surprise content |
| `cargo fmt` | NOT run — pre-existing rustfmt drift in `harness_*` async fn signatures (lines 547-625) is unrelated to this edit; spec said "don't touch anything else" |

## Operational notes

- The new binary at `target/release/reality-loop` (mtime 15:39:30) is staged for the NEXT drip. Engine PID 82045 (current attempt-1, etime 4h+) was not killed and continues to use its already-loaded pre-edit binary.
- MCP→CLI shim (commits 4072dcf3 / d48102e6) means subsequent `mcp__cgreality__harness_*` tool calls will fork-exec the fresh binary automatically. No `/mcp reconnect` needed.
- This rule is now LOCKED: any future patch-submission tool MUST adhere to the three-part action-string contract above, or supersede this decision explicitly with rationale.

## Source of truth

- `file:/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs#L6268`
- `file:/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs#L6362`
- `file:/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs#L6434`
- `file:/home/cabdru/contextgraph/memory/discoveries/agent-57-coordinator--workspace-reset-confuses-model-into-resubmitting.md`
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/harness-transitions/round-34.json`
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/harness-transitions/round-35.json`
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/harness-transitions/round-36.json`
