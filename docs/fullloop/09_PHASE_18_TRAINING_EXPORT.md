# Phase 18 — Training Data Export (Verified Episodes → LoRA-Ready Files)

**Goal:** every attempt with official readback becomes a typed training sample. The data flywheel from `docs/reality/05_auto_optimization_engine.md` starts to spin.
**Effort:** 1–2 days.
**Risk:** low — append-only files, no behavior change to existing artifacts.

## Why now

The cutover stores every attempt durably but never composes attempts into training data. Without this, LoRA fine-tuning of any of the three local models cannot use reality-grounded preference pairs.

## Artifact layout

```
<run_root>/training/
  positive.jsonl           ← official_resolved attempts only
  negative.jsonl           ← official_unresolved with verified failure
  preference_pairs.jsonl   ← same task; higher-reward preferred over lower-reward
  rejected_patches.jsonl   ← malformed, repeated, or harness-rejected patches
  manifest.json            ← schema_version, exporter_version, source attempts
```

All four jsonl files are append-only. `manifest.json` is rewritten on every export.

## Sample shapes

### `positive.jsonl` row
```json
{
  "schema_version": 1,
  "kind": "positive",
  "instance_id": "psf__requests-2317",
  "model": "qwen3.6:27b",
  "attempt_path": "file:/.../attempt-000017",
  "prompt_text_path": "file:/.../prompt.txt",
  "candidate_patch_path": "file:/.../candidate.patch",
  "patch_sha256": "sha256:...",
  "official_report_path": "file:/.../official-success-evidence.json",
  "reward": 4.31,
  "fail_to_pass_passed": ["test_redirects", "test_session_cookies", ...],
  "source_of_truth_refs": ["file:...", "sqlite:...#ledger_records"]
}
```

### `negative.jsonl` row
Same shape plus:
```json
{
  "kind": "negative",
  "failure_class": "patch_apply_failure | py_compile_failure | local_validation_failure | official_eval_failure | tool_grounding_failure",
  "first_error_location": "auth.py:42",
  "failed_test_names": ["test_redirects"],
  "reality_signal_packet_path": "file:/.../reality-signal-packet.json"
}
```

### `preference_pairs.jsonl` row
```json
{
  "kind": "preference_pair",
  "instance_id": "psf__requests-2317",
  "model": "qwen3.6:27b",
  "preferred_attempt_path": "file:/.../attempt-000017",
  "preferred_reward": 4.31,
  "rejected_attempt_path": "file:/.../attempt-000003",
  "rejected_reward": -0.22,
  "preference_strength": 4.53,
  "rationale": "preferred attempt has official_resolved=true; rejected has official_resolved=false"
}
```

### `rejected_patches.jsonl` row
```json
{
  "kind": "rejected_patch",
  "instance_id": "psf__requests-2317",
  "model": "qwen3.6:27b",
  "patch_sha256": "sha256:...",
  "reject_reason": "RUST_REALITY_PATCH_REPEATED | RUST_REALITY_PATCH_APPLY_FAILED | RUST_REALITY_FINAL_EDIT_TOOL_FAILED | RUST_REALITY_STRUCTURED_EDIT_FIND_AMBIGUOUS",
  "patch_text_pointer": "file:/.../candidate.patch (or null when no patch artifact)"
}
```

## Engine changes

`reality-loop attempt` ends with a "training-export" step:

1. Read the freshly written attempt-summary.
2. Classify it: official_resolved → positive; official_unresolved with verified failure → negative; harness-rejected → rejected_patch.
3. Append the appropriate row to the corresponding jsonl.
4. Build preference pairs by joining this attempt against all prior attempts on the same (model, task) where reward differs by ≥ 0.5; append to `preference_pairs.jsonl`.
5. Update `manifest.json`.

Read-after-write byte-equal verification on every append.

## New MCP tool

`training_export_summary { run_id? }` — returns `{ positive_count, negative_count, preference_pair_count, rejected_patch_count, manifest_path, source_of_truth }`.

Read-only. Useful for the dashboard (Phase 20).

## LoRA training driver hookup

`crates/context-graph-embeddings/src/bin/train_causal.rs` and other training binaries gain a `--reality-loop-training-root` flag pointing at `<run_root>/training/`. The driver loads only those files.

A new subagent `cc-reality-training-curator` (Sonnet, read-only) can be invoked to:
- Inspect `manifest.json` for class balance.
- Spot-check 3 random rows of each kind.
- Recommend whether the dataset is ready for a fine-tune.

It does not start training itself; the operator does.

## Sanity rules (engine-enforced)

- A row is appended only if every cited file path exists at append time (read-after-write check).
- A row is appended only if `source_of_truth_refs` includes the official report path (positive) or reality-signal packet (negative).
- Preference pairs are not generated when the lower-reward attempt's reward is `null` (compilation failed); a missing reward is not "preferred" against.

## AI-verifiable end-state for Phase 18

After running 5 attempts with at least one resolved and one unresolved:
```bash
test -s <run_root>/training/positive.jsonl
test -s <run_root>/training/negative.jsonl
test -s <run_root>/training/preference_pairs.jsonl
test -s <run_root>/training/manifest.json

mcp__cgreality__training_export_summary
# expected: positive_count >= 1, negative_count >= 1, preference_pair_count >= 1
```

## Out of scope

- Actual LoRA training run (operator-initiated).
- DPO-style reward-model training (later).
- Cross-model preference pairs (only same-model pairs; cross-model is informational, not preference).
