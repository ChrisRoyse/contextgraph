---
namespace: decisions
agent_id: agent-42-harness-editor
timestamp: 2026-05-07T16:00:00Z
status: active
tags: nvidia-smi, gpu-vram, defensive-parsing, generic-fix, locked-decision
---

# Decision: tolerate `[N/A]` sentinel in nvidia-smi compute-apps `used_memory`

> **Provenance**: this decision file was authored by the parent optimizer (turn 42 coordinator) on behalf of `cc-reality-harness-editor`, because the harness-editor subagent's tool surface does not include a "create new file" primitive (`harness_apply_line_window_edit` requires an existing path). The editor performed the actual code change and ran verification; this file is the SMAP narrative the editor would have written if it could.

## Symptom

Turn 42's `mcp__cgreality__reality_run_attempt {task_manifest: "tasks/swebench-lite/astropy__astropy-12907.json", model: "qwen3.6:27b", official_eval: true}` failed mid-engine-spawn with:

```
RUST_REALITY_GPU_COMPUTE_APPS_MEMORY_PARSE_FAILED
"nvidia-smi compute-apps used_memory was not numeric"
"line": "GPU-de2d5475-3447-83c3-1539-876a7257ae8a, 31923, [Not Found], [N/A]"
"value": "[N/A]"
```

The engine had already loaded the GGUF (65/65 layers, 23 GB on CUDA0) before the GPU-budget assertion fired. The parser at `crates/context-graph-cli/src/bin/reality-loop.rs:10895` (in `fn parse_gpu_compute_process_rows`) called `parts[3].parse::<u64>()` and returned an error for `[N/A]`.

## Root cause

`nvidia-smi --query-compute-apps=gpu_uuid,pid,process_name,used_memory --format=csv` outputs `[N/A]` for the `used_memory` column when the driver cannot introspect the process's VRAM usage. This is a known driver behavior, especially common on WSL2 and for processes the driver lacks proper visibility into. The corresponding `process_name` typically resolves to `[Not Found]`. Both are documented sentinel values, not garbage.

The original parser was correctly strict — "fail loud on non-numeric memory" — but did not exempt the documented sentinel. Result: every engine attempt on this host fails at GPU snapshot time, regardless of task or model.

## Decision

In `parse_gpu_compute_process_rows`, before the strict numeric parse of `parts[3]`, add a single early-skip:

```rust
if parts[3] == "[N/A]" {
    continue;
}
```

(See `crates/context-graph-cli/src/bin/reality-loop.rs:10895-10903` for the inserted block, with comment.)

This is **deliberately narrow**: only the bracketed sentinel `[N/A]` is exempted. Any other non-numeric string still hits the existing `RUST_REALITY_GPU_COMPUTE_APPS_MEMORY_PARSE_FAILED` path. The old test `gpu_compute_process_rows_reject_non_numeric_memory_edge_fsv` (which uses `"N/A"` without brackets) still passes — its input does not match the sentinel and so still fails loud. Strictness preserved everywhere except the documented sentinel.

A new test `gpu_compute_process_rows_skip_bracketed_na_memory_edge_fsv` (lines 12881-12895) proves the new behavior: a row with `used_memory == "[N/A]"` is dropped while a sibling numeric row in the same input is preserved.

## Why this is GENERIC-ONLY rule-compliant (loop.md §2.2)

The GPU snapshot is upstream of every model load on every task. The fix would be equally valuable on `psf__requests-2317`, `django__django-12286`, `sympy__sympy-13895`. It contains zero per-task knowledge — it is purely defensive parsing of an external tool's output. Mental check passes.

## Why we did NOT also handle other sentinels in this turn

`nvidia-smi` has additional documented sentinels (e.g., `[Insufficient Permissions]`, `[Not Supported]`). The current edit only addresses `[N/A]` because that is what the production failure reproduced. Layering more sentinels speculatively would expand scope; the strict-fail behavior for unknown values is correct — it surfaces new sentinels as failures so they can be analyzed and added explicitly.

If `[Insufficient Permissions]` etc. show up in a future engine failure, the same pattern applies and the fix is one-line plus one test.

## Verification record

- `cargo check -p context-graph-cli --bin reality-loop --tests`: PASS, 41.75s
- `cargo test -p context-graph-cli --bin reality-loop`: 36 passed, 0 failed, 1 ignored (pre-existing BF16 GGUF test)
- `cargo test -p context-graph-cli --bin reality-loop -- gpu_compute_process_rows`: 5/5 PASS (4 pre-existing + 1 new)
- `cargo build --release -p context-graph-cli`: PASS, fresh `target/release/reality-loop` (mtime advanced from 09:21:37 → 11:28:52)
- `git diff --stat`: 1 file changed, 24 insertions(+), 0 deletions(-)
- Three harness-transition records minted under `<runtime_root>/rust_reality_1778169857715520237_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/harness-transitions/round-{01,02,03}.json`. Round-03 corrected an inadvertent `#[test]` attribute removal from round-02.

## Next-turn implications

1. Drip a fresh `mcp__cgreality__reality_run_attempt` against `astropy__astropy-12907` (or whichever instance is at the head of the queue). The GPU snapshot should now succeed past the `[N/A]` sentinel; the engine should proceed to actual chat rounds.
2. If the inner-actor silent-hang surfaces (per current-phase.md §queued #1), the wallclock-cap design in `./memory/handoff/agent-41-to-next--wallclock-cap-design.md` becomes the next priority.
3. If a *different* nvidia-smi sentinel surfaces, apply the same one-line + one-test pattern.

## Locked decision

This is a `status: active` locked decision per loop.md §10. Future agents must not contradict it (i.e., must not revert the sentinel skip without writing a superseding decision file).
