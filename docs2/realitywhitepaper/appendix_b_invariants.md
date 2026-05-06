# Appendix B — Fail-Closed Gate Inventory

Companion to [`whitepaper.md`](whitepaper.md). This appendix enumerates every fail-closed gate in the Reality Loop and the ME-JEPA service, with source citations and remediation surface. A gate is *fail-closed* when its precondition is checked *before* the dependent step executes, and a typed error aborts the run when the precondition does not hold.

The gate inventory is the operational expression of the design criteria in §4 of the white paper. There are 37 gates across the Reality Loop and 18 across ME-JEPA, every one of which can be triggered deterministically and every one of which produces a JSON evidence file the operator can inspect.

---

## B.1 Reality Loop — Configuration / Precondition Gates

| # | Gate | Error code | Source line | Remediation surface |
|---|---|---|---|---|
| 1 | `--max-attempts >= 1` | `RUST_REALITY_MAX_ATTEMPTS_INVALID` | reality-loop.rs:197 | Re-invoke with `--max-attempts >= 1`. |
| 2 | `--run-until-resolved` requires `--official-eval-each-attempt` | `RUST_REALITY_CONTINUOUS_REQUIRES_OFFICIAL_EVAL` | reality-loop.rs:206 | Add `--official-eval-each-attempt` or drop continuous mode. |
| 3 | `--official-eval-each-attempt` is mandatory for SWE loops | `RUST_REALITY_OFFICIAL_EVAL_REQUIRED` | reality-loop.rs:231 | The loop refuses to run without an oracle. |
| 4 | `--context-graph-bin` exists on disk | `RUST_REALITY_CONTEXT_GRAPH_BIN_MISSING` | reality-loop.rs:217 | Build the binary or pass an explicit path. |
| 5 | `--task-manifest` exists on disk | `RUST_REALITY_TASK_MANIFEST_MISSING` | reality-loop.rs:223 | Pass a valid manifest path. |

These five gates run before any reality-state mutation. Each writes a typed `RealityError` JSON to `<runtime_root>/failures/failure-<ts>-<code>.json` *with byte-equal readback verification* before the process exits non-zero.

---

## B.2 Reality Loop — Manifest Validation Gates

The task manifest is parsed as a `TaskManifest` struct via `serde_json` and then run through `validate_task_manifest_contract`. Every field is checked individually so the failure message names the exact violation.

| # | Gate | Error code | Source line | Remediation surface |
|---|---|---|---|---|
| 6 | `dataset == "princeton-nlp/SWE-bench_Lite"` | `RUST_REALITY_TASK_DATASET_MISMATCH` | reality-loop.rs:654 | This binary is bound to SWE-bench Lite by design; build a different binary for other datasets. |
| 7 | `instance_id` non-empty + contains `"__"` | `RUST_REALITY_TASK_INSTANCE_ID_INVALID` | reality-loop.rs:664 | SWE-bench instance IDs always contain `__`. |
| 8 | `repo` is `owner/name` and contains no `..` | `RUST_REALITY_TASK_REPO_INVALID` | reality-loop.rs:674 | No path traversal in repo identifiers. |
| 9 | `base_commit` is a 40-character hex SHA | `RUST_REALITY_TASK_BASE_COMMIT_INVALID` | reality-loop.rs:684 | Use the full commit SHA, not an abbreviated form. |
| 10 | `problem_statement` non-empty | `RUST_REALITY_TASK_PROBLEM_STATEMENT_MISSING` | reality-loop.rs:694 | The model needs a problem statement; refusing to use one is a hard error. |
| 11 | `fail_to_pass` non-empty | `RUST_REALITY_TASK_FAIL_TO_PASS_MISSING` | reality-loop.rs:704 | Without a FAIL_TO_PASS list, the success predicate is undefined. |
| 12 | `pass_to_pass_count > 0` | `RUST_REALITY_TASK_PASS_TO_PASS_MISSING` | reality-loop.rs:714 | The system also tracks regressions in PASS_TO_PASS tests. |
| 13 | `source_of_truth` binds dataset + instance | `RUST_REALITY_TASK_SOURCE_OF_TRUTH_INCOMPLETE` | reality-loop.rs:724 | Manifest-level provenance is non-optional. |

Manifest validation is purely structural — it never invokes a model and never writes to RocksDB. A failed manifest aborts the run before the GPU is touched.

---

## B.3 Reality Loop — Execution Gates

These gates are checked inside the per-attempt loop, in order. Failure of any gate aborts the attempt (or, in continuous mode, aborts the entire loop) with a typed error and a rich `details` block.

| # | Gate | Error code | Source line | When checked |
|---|---|---|---|---|
| 14 | `git rev-parse HEAD == base_commit` after reset | `RUST_REALITY_BASE_COMMIT_READBACK_MISMATCH` | reality-loop.rs:1190 | After `git reset --hard <base_commit>` and `git clean -fdx`. |
| 15 | `git status --porcelain` empty after reset | `RUST_REALITY_WORKSPACE_NOT_CLEAN_AFTER_RESET` | reality-loop.rs:1212 | After reset; ensures no leftover files. |
| 16 | Free GPU VRAM ≥ `--gpu-min-free-mb` | `RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED` | reality-loop.rs:2076 | Before each Ollama call. |
| 17 | Used GPU VRAM ≤ `--gpu-max-used-mb` (when set) | `RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED` | reality-loop.rs:2087 | Before each Ollama call. |
| 18 | Model response parses as JSON `{patch, notes}` | `RUST_REALITY_MODEL_RESPONSE_JSON_INVALID` | reality-loop.rs:1100 | After Ollama `/api/chat` returns. |
| 19 | Patch field is present and non-empty | `RUST_REALITY_MODEL_PATCH_MISSING` | reality-loop.rs:1124 | After JSON parse. |
| 20 | Patch differs from previous attempts (SHA-256) | `RUST_REALITY_PATCH_REPEATED` | reality-loop.rs:1250 | After patch parse, before `git apply`. |
| 21 | Patch applies via `git apply` | `RUST_REALITY_PATCH_APPLY_FAILED` | reality-loop.rs:1271 | At apply time. |
| 22 | Patch does not touch test/fixture/harness paths | `RUST_REALITY_PATCH_MODIFIES_TESTS` / `RUST_REALITY_PATCH_TOUCHES_BENCHMARK` | reality-loop.rs:1303 | After apply, before official eval. |
| 23 | Docker SDK preflight returns JSON | `RUST_REALITY_OFFICIAL_DOCKER_UNAVAILABLE` / `RUST_REALITY_OFFICIAL_DOCKER_PREFLIGHT_JSON_INVALID` | reality-loop.rs:1536, 1546 | Before invoking the official evaluator. |
| 24 | Official harness exits 0 | `RUST_REALITY_OFFICIAL_EVAL_FAILED` | reality-loop.rs:1495 | After `python -m swebench.harness.run_evaluation`. |
| 25 | Final report contains `resolved_ids` | `RUST_REALITY_OFFICIAL_REPORT_MISSING` | reality-loop.rs:1610 | After eval; finds the JSON report by `<run_id>` filename match. |
| 26 | Target instance is classified | `RUST_REALITY_OFFICIAL_REPORT_INSTANCE_MISSING` | reality-loop.rs:1697 | After report load; instance must be in `completed_ids ∪ incomplete_ids ∪ empty_patch_ids ∪ error_ids`. |

Note on Gate 26: this is the most subtle of the execution gates. Some failure modes can produce a SWE-bench report whose top-level `resolved_ids` array exists but whose target instance is in *none* of the classified subsets — typically when the harness silently skipped the instance. The gate refuses to treat such a report as a non-resolution, because "I couldn't tell what happened" is not a permitted state. The loop produces an error rather than a misleading success/failure verdict.

The `is_forbidden_benchmark_mutation` predicate (Gate 22, reality-loop.rs:1332) deems a path forbidden when it starts with `test_`, `tests/`, contains `/test_`, `/tests/`, ends in `_test.py`, contains `conftest.py`, `tox.ini`, or `pytest`. The match is conservative by design; any path that *might* be a test file is rejected.

---

## B.4 Reality Loop — Persistence Integrity Gates

Every artefact the loop persists is byte-equal-verified on readback. This is the runtime expression of the reality-shift principle.

| # | Gate | Error code | Source line | What it checks |
|---|---|---|---|---|
| 27 | All JSON writes do byte-equal readback | `RUST_REALITY_JSON_READBACK_MISMATCH` | reality-loop.rs:2208 | After `fs::write`, re-read, parse to `Value`, compare structurally. |
| 28 | All text writes do byte-equal readback | `RUST_REALITY_TEXT_READBACK_MISMATCH` | reality-loop.rs:2228 | After `fs::write`, re-read, compare bytes. |
| 29 | Each appended event must round-trip | `RUST_REALITY_EVENT_READBACK_MISMATCH` | reality-loop.rs:1989 | After append to `reality-events.jsonl`, re-read, compare. |

These three gates exist because filesystem writes can fail for many reasons (full disk, permissions, transient I/O errors, kernel page-cache surprises) without surfacing as a Rust `io::Error`. A successful `fs::write` does *not* prove the file's contents on disk match the bytes the program intended to write. The readback gates close that gap.

---

## B.5 Reality Loop — Hidden Solution Leak Gate

This is one of the most epistemically important gates in the system. The deterministic prompt compiler enforces:

| # | Gate | Trigger | What it prevents |
|---|---|---|---|
| 30 | `hidden_solution_leak_absent` | the prompt is rejected if the SWE-bench gold patch text or test patch text appears anywhere in the prompt | Prevents the system from accidentally feeding ground truth into the model and then claiming success. |

This is implemented as a hard string-match check against the dataset's gold patch and test patch fields *before* the prompt is allowed to leave the compiler. A prompt that fails this gate is rejected with a typed error; the run cannot proceed.

The gate exists because it is surprisingly easy to leak ground truth through repository context. If the loop's `repository_context` builder happens to include a file the gold patch already modified, the model could, in principle, observe the gold change. The hard gate catches this both as a bug and as a sanity check on test integrity.

---

## B.6 Reality Loop — Prompt Quality Gates

Eight quality checks must pass before a compiled prompt is used:

| # | Gate | What it requires |
|---|---|---|
| 31 | `failed_test_included` | every `failed_fail_to_pass` and `failed_pass_to_pass` test ID is mentioned in the prompt. |
| 32 | `official_error_code_included` | the `official_error_code` text from the official failure evidence is in the prompt. |
| 33 | `assertion_excerpt_included` | the first 80 characters of `test_output_excerpt` is in the prompt. |
| 34 | `source_snippet_included` | the literal `Repository context` is present (so the model knows where to look). |
| 35 | `previous_patch_hash_included` | every previous patch hash appears in the `NO_REPEAT_PATCH_SHA256` list. |
| 36 | `no_repeat_constraint_included` | the literal `Do not repeat an unchanged patch` is present. |
| 37 | `source_of_truth_refs_included` | every short source-of-truth ref appears. |

Gate 30 (hidden solution leak absent) is the eighth quality gate. Together with the seven above, they form the canonical contract for prompts emitted by the deterministic compiler.

---

## B.7 ME-JEPA — Configuration Gates

These gates are checked in `TrainConfig::validate` (`config.rs:131-232`) before any training run begins.

| # | Gate | Validation |
|---|---|---|
| 1 | `model.encoder.kind == "mlp"` | Encoder kind whitelist. |
| 2 | `model.encoder.hidden` non-empty + no zero | MLPs without layers or with zero-width layers are invalid. |
| 3 | `model.encoder.out_dim > 0` | Latent dimension must be positive. |
| 4 | `model.predictor.kind == "mlp"` | Predictor kind whitelist. |
| 5 | `model.predictor.hidden` non-empty + no zero | Same as encoder. |
| 6 | `model.predictor.out_dim == model.encoder.out_dim` | Predictor lives in latent space; output dim must match encoder out. |
| 7 | `model.ema_momentum ∈ [0, 1)` | EMA momentum must be a valid probability strictly less than 1. |
| 8 | `loss.*` weights finite and ≥ 0 | Any negative or non-finite loss weight is invalid. |
| 9 | `optim.kind == "adamw"` | Phase 7 supports AdamW only. |
| 10 | `optim.lr`, `optim.weight_decay` finite and ≥ 0 | Standard AdamW parameter constraints. |
| 11 | `schedule.epochs > 0` | Training runs must have at least one epoch. |
| 12 | `schedule.batch_size > 0` | Standard ML constraint. |
| 13 | `schedule.device == "cuda"` | **CPU fallback is explicitly forbidden.** |
| 14 | `stopping.metric` is `<split>_<metric>` form | e.g. `val_latent_mse`. |
| 15 | `stopping.target` finite and ≥ 0 | Standard. |
| 16 | `stopping.max_seconds > 0` | Training runs have a wall-clock budget. |
| 17 | `seed` is set | Reproducibility constraint. |

Gate 13 is the most consequential of the group. ME-JEPA refuses to fall back to CPU training. The choice forces operators on hardware without compute capability 12.0 to either upgrade or refrain from training. The deliberate inflexibility prevents silent degradation: a run that starts on CPU cannot produce numerically equivalent results to one on GPU, and the system would rather refuse than mislead.

---

## B.8 ME-JEPA — Runtime Integrity Gates

| # | Gate | What it checks |
|---|---|---|
| 18 | Identical positive/negative pairs rejected | `validate_examples` errors when `target_panel == negative_panel`. |
| 19 | Non-finite values rejected | Every input panel and action is checked for NaN/Inf. |
| 20 | Wall-clock budget respected | If `started.elapsed() > stopping.max_seconds`, the run errors. |
| 21 | Final stopping metric must hit target | If not converged AND final stopping metric > target, the run is `TrainingFailed`. |
| 22 | Artefact root must not exist | `ensure_clean_run_dir` errors if the directory exists; evidence is immutable. |
| 23 | All artefact files SHA-256 verified on inference | `load_artifact_for_inference` rejects on hash mismatch. |
| 24 | Panel shape match on inference | `PanelShapeMismatch { domain_pack_id, expected, actual }` if the panel dim drifts. |
| 25 | Predicted vector contains no non-finite values | Every intermediate is checked; non-finite aborts with `TrainingFailed`. |
| 26 | Persisted record matches in-memory record | After every batch write, immediate readback + bincode equality check. Mismatch returns `StorageInvariantViolation`. |
| 27 | Domain pack required CFs present | `verification_policy.required_cfs` enforced at registration. |
| 28 | Failed runs are still audited | Even if the original `Started` row was already persisted, the `Failed` row is written; no state transition is lost. |
| 29 | Surprise threshold is hard | Surprise events are persisted *only* when `cosine < threshold`. No "soft" surprise; no "high-confidence" classifier. |
| 30 | Constellation modalities reject `SelfReport` | A `LearnerConstellation` with a `SelfReport` centroid is rejected at validation. |
| 31 | KSG-1 MI estimator min sample size | `MI_AUDIT_MIN_SAMPLE_SIZE = 50`; below this, the audit refuses to estimate. |
| 32 | KSG-1 MI rejects non-finite | `validate_finite_pair` rejects any pair with NaN/Inf. |

---

## B.9 Cross-System Substrate Gates

The audit substrate (BlackBoxFix SQLite + reality-events.jsonl + RocksDB audit logs) shares a discipline that is enforced at every write:

| # | Gate | What it requires |
|---|---|---|
| 1 | Append-only column families never updated | `audit_log`, `audit_by_target`, `dj_audit_log`, `merge_history`, `importance_history` are never `update`ed; only `append`ed. |
| 2 | SHA-256 content addressing | Every record carries either a `payload_sha256` (events) or a `content_hash` (DJ records). |
| 3 | Permanent merge history | `MergeRecord` rows never expire and are never deleted. |
| 4 | Permanent importance history | `ImportanceChangeRecord` rows never expire. |
| 5 | Failed write is a hard error | A `WriteBatch` that fails `db.write` aborts the operation; partial state is never exposed. |
| 6 | RocksDB single-writer | `<storage.path>/mcp.pid` Unix `flock`. Daemon mode also runs a port-health check; "alive PID" alone is insufficient. |
| 7 | Zombie-aware lock reclaim | `kill(pid, 0)` returns 0 for zombies; `/proc/{pid}/status` is read for `State:\tZ` to detect this case. |
| 8 | Schema version mismatch is a hard error | First-byte version mismatch on read returns `CoreError::SerializationError("...No automatic migration is supported - data must be regenerated")`. |
| 9 | RocksDB requires all on-disk CFs to be opened | Adding a CF works on next open; removing one requires deleting the database (RocksDB protocol). |

---

## B.10 Operator Surface

Every gate failure produces:

- **A typed error code** (e.g. `RUST_REALITY_PATCH_REPEATED`).
- **A `field_path`** identifying which field of which record triggered the gate.
- **A `remediation`** message describing what the operator should do.
- **A `details`** block with the offending values (e.g. the SHA of the repeated patch).
- **A `source_of_truth`** path identifying where the persisted evidence lives.

The shell consumer surface is stable:

- On any failure: stderr line `RUST_REALITY_LOOP_ERROR_SOURCE_OF_TRUTH=<path>` (where `<path>` is the failure JSON file).
- On success: stdout line `RUST_REALITY_LOOP_SUMMARY=<path>`.

The operator does not need to scrape stack traces or grep logs. The program tells them, on a single line, exactly which file to read for the full story.

---

## B.11 Why This Many Gates?

A natural question: is 37 gates too many? The answer the codebase commits to is *no, because each one represents a real failure mode observed in practice or imported from the SWE-bench experience*. A cheaper system with 5 gates would require either trusting the model's natural-language reports (fails at claim-drift) or trusting a single end-to-end test (fails at oracle-drift, since "the test passed" is not the same as "the right test passed in the right way").

The cost of a fail-closed gate is small — typically a few microseconds of validation. The benefit is qualitative: every gate is a place where a misalignment between the system's intent and reality surfaces *immediately*, with a typed name, instead of propagating into a downstream artefact and corrupting later decisions.

Four of these gates have, in development, caught regressions that no integration test would have caught:

- **Gate 20 (`patch_repeated`)** prevents the loop from declaring "no progress" as a loop-state when the model is in fact regenerating identical responses.
- **Gate 22 (`patch_modifies_tests`)** caught accidental deny-rule bypasses during early Python development.
- **Gate 26 (`official_report_instance_missing`)** caught a SWE-bench harness mode where the official runner silently skipped instances under specific Docker conditions.
- **Gate 30 (`hidden_solution_leak_absent`)** caught a repository-context builder regression that included gold-patch lines as "context".

Gates are paid for in lines of code. They are repaid in epistemic clarity.
