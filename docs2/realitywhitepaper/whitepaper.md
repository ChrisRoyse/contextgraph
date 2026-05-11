# ContextGraph Reality Loop: A Measurement-Bound Repair Engine with a Joint-Embedding Predictive World Model

**A technical white paper on fail-closed software repair, ME-JEPA latent dynamics, and durable provenance in the ContextGraph runtime.**

Version 1.0 — 2026-05-06.
Subject directory: `/home/cabdru/contextgraph` (workspace `context-graph`).
Primary source artefacts: `crates/context-graph-cli/src/bin/reality-loop.rs` (2,385 LOC) and `crates/context-graph-dynamicjepa/` (Phase 7/8 service + Candle CUDA training loop).

---

## Abstract

Modern coding agents that drive code edits and tool calls increasingly speak in the language of certainty — "fixed", "passes", "ready to merge" — without owning a primitive that proves any of those words. The result is a quiet, expensive class of failures: patches that look right but never apply, tests that pass locally and fail in CI, capabilities reported by self-confidence rather than by an oracle. ContextGraph's *Reality Loop* is a Rust subsystem that refuses to declare success unless the official SWE-bench Lite Docker evaluator places the target instance in `resolved_ids`. Every artefact along the way — the prompt, the model response, the candidate patch, the per-attempt evidence, the audit row — is byte-equal-readback verified before it is permitted to influence the next decision. Alongside the loop, ContextGraph ships a Joint-Embedding Predictive Architecture (called *ME-JEPA* in this paper, *DynamicJEPA* in the codebase) that learns latent dynamics from instrument readings and emits *surprise events* whenever the world disagrees with its predictions. Together, the two engines turn a coding agent from a confident narrator into a measurement-grounded participant: it perceives current state, alters it through governed tools, observes a real outcome, and persists the delta on an append-only audit substrate. This paper documents how the loop and the world model are built, why specific design decisions were made, and what evidence the system produces to back its claims.

**Audience.** Machine-learning engineers, AI-safety researchers, software-engineering teams operating coding agents, and infrastructure leads evaluating measurement-grounded autonomy.

**Contents.** §1 framing. §2 the verification gap in coding agents. §3 the problem statement. §4 design criteria. §5 the reality loop. §6 the role of ME-JEPA (DynamicJEPA). §7 evidence and invariants. §8 constraints, tradeoffs, limitations. §9 security, privacy, maintainability. §10 conclusion. Appendices: architecture, fail-closed gate inventory, glossary.

---

## 1 Background and Context

ContextGraph is an 11-crate Rust workspace that stores text and code as 14-embedder semantic fingerprints in a single RocksDB database (101 column families) and exposes them through 141–145 Model Context Protocol tools. It runs locally on commodity GPU hardware (the development target is RTX 5090 / Blackwell, sm_120a, CUDA 13.2) and is built primarily to act as a durable, queryable memory substrate for AI agents.

Two of its subsystems push beyond memory:

- The **Reality Loop** (`crates/context-graph-cli/src/bin/reality-loop.rs`) is a self-contained `tokio::main` binary that drives a single local Ollama model through SWE-bench Lite repair attempts. Its single, immovable definition of success is whether the official SWE-bench Docker harness reports the target instance in `resolved_ids`.
- **DynamicJEPA / ME-JEPA** (`crates/context-graph-dynamicjepa/`) is a 28-column-family Joint-Embedding Predictive Architecture trained on Candle 0.10.2 with CUDA. It learns to predict the next latent panel from a `(panel, action)` pair and persists *surprise events* whenever observed reality diverges from its prediction.

Both subsystems share a single audit substrate — a BlackBoxFix SQLite ledger at `LEDGER_SCHEMA_VERSION = 14` and an append-only `reality-events.jsonl` event stream. Neither subsystem can claim success without the other's source-of-truth confirmation in scope. This separation is intentional: the loop owns *external* truth (Docker), the world model owns *internal* prediction quality, and the audit substrate is the only place where the two meet.

---

## 2 The Verification Gap in Coding Agents

Industry-standard coding agents are end-to-end black boxes from the operator's point of view. The agent ingests a problem statement, emits a candidate solution, and (optionally) reports a confidence score. Three failure modes recur in this paradigm:

1. **Claim drift.** The agent's natural-language report ("I have applied the fix") is not coupled to anything verifiable. The fix may not exist in the working tree; the patch may not parse; the unit test it claims to pass may not exist. There is no read-after-write step between the model's *assertion* and the operator's *belief*.
2. **Oracle drift.** Even when a verifier is present, it is often the agent's own test of its own work. Self-graded test passes are not safe to treat as success because the graders, the prompts, and the test invocation can all be manipulated by the agent under pressure to succeed. Without an *external*, *immutable* oracle, every claim is suspect.
3. **Capability inflation.** Reports aggregated across many runs amplify both signal and noise. If 1 in 8 runs of an agent self-grades as a success and the system promotes those runs without external verification, the public-facing capability score will drift upward over time without any underlying improvement.

These failure modes are not pathologies of any particular model. They are predictable consequences of *agents that own their own success criteria*. The fix is not better prompting or more capable models; it is to take the success criterion away from the agent entirely.

---

## 3 Problem Statement

A coding agent operating in a real environment must satisfy a set of properties that no ungoverned LLM stack provides out of the box:

| Property | Meaning |
|---|---|
| **External oracle** | Success is defined by a process the agent does not control. |
| **Read-after-write provenance** | Every artefact (prompt, response, patch, evidence) is verified to exist on disk with the bytes the system intended to write. |
| **Bounded edit surface** | The agent edits a small, declared set of paths; the verifier, the harness, the dataset, the reward spec, and the test suite are immutable from the agent's perspective. |
| **Fail-closed gates** | Any unmet precondition produces a typed error and aborts the run *before* the model is invoked. |
| **Auditable history** | Every artefact is persisted into an append-only ledger keyed to the run, so two engineers reading the database from different machines see the same story. |
| **No silent fallback** | When a required input is missing, the system refuses to proceed rather than substituting a default. |

The problem this paper addresses is: *how do you build a coding-agent runtime that satisfies all six of these properties simultaneously, runs locally on commodity hardware, and produces evidence durable enough to be auditable months after the run?*

---

## 4 Criteria for a Good Solution

The ContextGraph Reality Loop was designed under the following criteria, all of which are enforced in code rather than only in documentation.

### 4.1 Source-of-truth before action

No alter call may execute without first perceiving current state. Concretely, before any file is edited, the harness opens a window over the current bytes and records the SHA. After the alter call, the harness re-reads those bytes and confirms the new SHA. The session-level shift log records every edit. This is the *reality-shift principle*: "no alteration without current state, no alteration without a delta back into perception."

### 4.2 One immovable success predicate

The success predicate for SWE-bench is, by construction, the only one the loop honours: *the official Docker SWE-bench report's `resolved_ids` array contains the target instance*. The loop refuses to enter "run-until-resolved" mode without `--official-eval-each-attempt`; without the official Docker evaluator there is no oracle, and so there is no run.

### 4.3 Deny-by-default edit boundaries

The model is permitted to edit two things: `candidate.patch` and `notes`. The model is forbidden from editing the task manifest, the reward spec, the verifier code, the SWE-bench fixtures, the harness itself, the official evaluator, the BlackBoxFix ledger, and the dataset metadata. Patches that touch any path matching `test_`, `tests/`, `/test_`, `/tests/`, `_test.py`, `conftest.py`, `tox.ini`, or `pytest` are rejected with `RUST_REALITY_PATCH_TOUCHES_BENCHMARK`.

### 4.4 Byte-equal readback on every persisted artefact

Every JSON write parses back through `serde_json::Value` and is compared structurally against the in-memory record. Every text write compares bytes. Every appended event in `reality-events.jsonl` round-trips and is rejected with `RUST_REALITY_EVENT_READBACK_MISMATCH` if the readback does not match. This catches both filesystem corruption and serialisation regressions before they propagate to the next decision.

### 4.5 GPU and Docker as first-class preconditions

Before the model is invoked, the loop reads `nvidia-smi` and asserts `free VRAM >= --gpu-min-free-mb` (default 8 GB) and (when set) `used VRAM <= --gpu-max-used-mb`. Before the official evaluator is called, the loop runs a Python-side preflight: `import docker; docker.from_env().version()`. Either failure aborts the run with a typed error and a remediation message; the model is never invoked under degraded conditions.

### 4.6 No silent fallback

The loop deletes its own old Python implementations rather than retain them as a "compatibility shim". Pre-production policy is: when a record version changes, the database is wiped, not migrated. When a model file is missing, the load fails fast, not silently substitutes a base model. When the official SWE-bench report is missing, the run is *blocked*, not classified as "uncertain".

---

## 5 The ContextGraph Reality Loop

The Reality Loop turns the criteria in §4 into a single Rust binary. This section walks through its lifecycle, its measurement contract, its agent-tool surface, its prompt compiler, and the artefacts it writes.

### 5.1 Lifecycle

```
+-----------------------+      +------------------------+      +------------------------+
| reality-loop.rs       |--->  | target/debug/          |--->  | blackbox.sqlite        |
| (Rust, tokio::main)   |      | context-graph (CLI)    |      | LEDGER_SCHEMA_VERSION 14|
|                       |      | blackbox subcommand    |      +------------------------+
|                       |--->  | reality-events.jsonl (append-only, byte-equal readback)
+-----------+-----------+
            |
            v
   Ollama /api/chat (qwen3.6:27b, --keep-alive 30m)
            |
            v
   git apply --whitespace=nowarn (--> repo workspace at exact base_commit)
            |
            v
   python -m swebench.harness.run_evaluation (Docker Desktop / WSL2)
            |
            v
   official report --> resolved_ids[] + per-instance run logs
            |
            v
   official-{success|failure}-evidence.json + reward + score rows
```

A single attempt — the unit of work the loop orchestrates — is roughly:

1. **Validate the task manifest** against an immutable measurement contract.
2. **Compile a deterministic prompt** that includes verified failure evidence and the no-repeat-patch hash list.
3. **Drive the local Ollama model** under a GPU VRAM budget.
4. **Apply the candidate patch** to the real repository, checked out at the exact `base_commit`.
5. **Validate fail-closed**: no test/harness mutation, no patch repeat, patch must apply with `git apply`.
6. **Run the official Docker SWE-bench evaluator** and read `resolved_ids` back from the JSON report.
7. **Persist every step** into the BlackBoxFix SQLite ledger and the JSONL reality event stream.

Each step is fail-closed with a typed `RealityError` carrying `error_code`, `field_path`, `remediation`, `details`, and `source_of_truth`. Failure is logged to `<runtime_root>/failures/failure-<ts>-<code>.json` with readback verification before the process exits non-zero.

### 5.2 The Measurement Contract

The most consequential file in the system is the JSON object the loop writes to `<run_root>/measurement-contract.json` *before any model is called*. It pins the entire claim scope and is signed by SHA-256 read-after-write so it cannot be silently mutated mid-run.

| Field | Value | Purpose |
|---|---|---|
| `schema` | `"contextgraph_reality_measurement_contract_v1"` | Distinguishes contract versions; contract v1 is currently authoritative. |
| `claim_scope.primary_claim` | `"official_swe_instance_resolution_only"` | The system claims one thing: a specific SWE-bench instance was resolved by the official evaluator. Nothing else. |
| `claim_scope.success_predicate` | `"official_report.resolved_ids contains task.instance_id"` | The exact predicate the run is testing, in plain text. |
| `claim_scope.explicit_non_claims` | three entries: no full-repo proof, no human-intent proof, no success without official Docker | The system *publicly disowns* claims it does not measure. |
| `oracle.dataset` | `"princeton-nlp/SWE-bench_Lite"` | Dataset name as a hard constant; loops on other datasets must be different binaries. |
| `oracle.split` | `"test"` | Always test split for SWE-bench Lite. |
| `oracle.instance_id` | from manifest | The single instance under test. |
| `oracle.fail_to_pass` / `pass_to_pass_count` | from manifest | The exact tests whose outcome must change / be preserved. |
| `oracle.source_of_truth` | from manifest | A binding statement that ties the claim to dataset + instance. |
| `immutable_boundaries.task_manifest` | `file:<path>` | The task manifest itself becomes part of the immutable scope. |
| `immutable_boundaries.base_commit` | git SHA | The exact commit the repair must work against. |
| `immutable_boundaries.official_evaluator` | `"python -m swebench.harness.run_evaluation"` | The verifier is named explicitly, not abstracted. |
| `immutable_boundaries.reward_spec` | `file:config/reward_spec.verified_outcome.json` | The reward spec is also immutable from the model's view. |
| `mandatory_readbacks` | 10 readback IDs | Every step that *must* readback to count as performed. |
| `model_editable_scope` | `["candidate.patch", "notes"]` | The minimal surface the model is allowed to write. |
| `model_forbidden_scope` | 7 explicit names (manifest, harness, ledger, reward spec, verifier, tests, fixtures) | Deny-by-default. |
| `failure_policy.unmeasured_success` | `"blocked"` | Success without official Docker confirmation is blocked, not pending. |
| `failure_policy.test_or_harness_mutation` | `"failed"` | Touching forbidden paths is a hard failure, not a warning. |

The 10 mandatory readbacks include `task_manifest_readback`, `measurement_contract_readback`, `git_head_equals_base_commit`, `git_status_clean_before_attempt`, `candidate_patch_applies_to_base`, `candidate_patch_does_not_modify_tests_or_harness`, `official_docker_sdk_preflight`, `official_swe_report_with_resolved_ids`, `blackbox_sqlite_ledger_counts`, and `reality_event_log_append_readback`. Each readback maps to a fail-closed gate enumerated in Appendix B.

### 5.3 Reality Branches

Every event the loop emits is namespaced by a *reality branch ID* so the subsystem is auditable across multiple layers — model, instance, attempt, prompt-candidate.

| Function | Pattern |
|---|---|
| `root_reality_branch_id(loop_id)` | `<loop_id>:root` |
| `task_reality_branch_id(loop_id, model, instance)` | `<loop_id>:<model>:<instance>:task-root` |
| `attempt_reality_branch_id(...)` | `<loop_id>:<model>:<instance>:attempt-NNN` |
| `prompt_candidate_branch_id(...)` | `<loop_id>:<model>:<instance>:prompt-candidate-NNN` |

Every event carries `schema_version = 1` and includes `loop_id`, `loop_layer`, `event_type`, `status`, `model`, `instance_id`, `attempt`, `reality_branch_id`, `parent_reality_branch_id`, `promotion_status`, `payload`, and `payload_sha256`. Reading the JSONL event stream is sufficient to reconstruct an attempt's full timeline without needing live access to the runtime.

### 5.4 Governed Agent Tools

When the model needs to read the working repository, it does so through a small, governed tool surface:

| Tool | Mode | Purpose |
|---|---|---|
| `list_files` | read-only | List files matching an optional pattern under a path. |
| `read_file` | read-only | Read bytes from a file (with optional line range) and return SHA-256. |
| `search_repo` | read-only | grep-style search across the repo, returning file paths and SHAs. |
| `inspect_repository_context` | read-only | Read pre-materialised `repository-context.json`. |
| `read_failure_evidence` | read-only | Read persisted validation/official evidence files for the current task. |

Every tool call is rejected with a typed `tool_result_blocked` row when: the tool is unknown, a required path is missing, the path is absolute or contains `..`, the resolved path escapes the repository, the listed directory is missing, the file is missing, or a query is empty. There is no `write_file` or `apply_patch` tool — writes happen only via the loop's outer fail-closed gates after the candidate patch is parsed from the model's structured response.

### 5.5 The Deterministic Prompt Compiler

A coding agent's prompt is part of its specification. The loop's prompt compiler enforces a binding *prompt contract* — three template kinds, each with required source-of-truth inputs.

| `prompt_kind` | Template ID | Required inputs |
|---|---|---|
| `initial` | `swe_repair_initial_v2` | task + repo_context (with `source_of_truth`) |
| `local_failure_refinement` | `swe_repair_local_failure_v2` | + a `validation` row with `source_of_truth` |
| `official_failure_refinement` | `swe_repair_official_failure_v2` | + `official_evidence` with `source_of_truth.official_report` |

Every compiled prompt carries `instance_id`, `repo`, `base_commit`, the problem statement wrapped in explicit markers, a `PREVIOUS_ATTEMPT_JSON` block, a `VERIFIED_EVIDENCE_JSON` block, a `NO_REPEAT_PATCH_SHA256` list (so the model cannot re-emit a previously rejected patch), and a `REPOSITORY_CONTEXT` block of durable on-disk file snippets.

The compiler enforces eight quality gates before the prompt is permitted to leave the function:

| Quality check | Trigger |
|---|---|
| `failed_test_included` | every `failed_fail_to_pass` and `failed_pass_to_pass` test ID is mentioned. |
| `official_error_code_included` | `official_error_code` text is in the prompt. |
| `assertion_excerpt_included` | first 80 chars of `test_output_excerpt` is in the prompt. |
| `source_snippet_included` | the literal `Repository context` is present. |
| `previous_patch_hash_included` | every previous patch hash appears. |
| `no_repeat_constraint_included` | the literal `Do not repeat an unchanged patch` is present. |
| `source_of_truth_refs_included` | every short source-of-truth ref appears. |
| `hidden_solution_leak_absent` | gold patch / test patch text is absent (hard gate against accidentally leaking ground truth). |

Every output (prompt text, prompt manifest, request/response artefacts, parse report, prompt-quality metadata) is byte-equal-verified on readback. The previous Python prompt compiler was deleted from the repository to prevent split ownership; the Rust compiler is the only authority.

### 5.6 Per-Attempt Official Evaluation

When validation passes locally, `run_official_eval` is invoked inside the attempt loop:

1. Verify Docker SDK preflight via `python -c "import docker; ..."` returning JSON.
2. Write `official-prediction.jsonl` with one row `{instance_id, model_name_or_path, model_patch}`.
3. Run `python -m swebench.harness.run_evaluation --dataset_name princeton-nlp/SWE-bench_Lite --split test --instance_ids <id> --predictions_path <path> --max_workers <n> --timeout <s> --cache_level <env> --run_id <id> --report_dir <dir>` with `DOCKER_HOST` and `DOCKER_CONFIG` env vars.
4. Find the JSON report whose filename contains `<run_id>` and that has a `resolved_ids` array.
5. Build `official-success-evidence.json` or `official-failure-evidence.json` carrying `resolved`, `completed`, `incomplete`, `empty_patch`, `errored`, `previous_patch_sha256`, `source_of_truth`, and the full `official_report` payload.
6. Persist a BlackBoxFix `score` row and (when the reward spec is present) a `reward compile` row for the attempt.

If the report exits 0 but the target `instance_id` is not classified into any of `completed_ids | incomplete_ids | empty_patch_ids | error_ids`, the loop raises `RUST_REALITY_OFFICIAL_REPORT_INSTANCE_MISSING` rather than silently treating the run as a non-resolution. *Failure evidence becomes the next prompt's `official_evidence` input.* This closes the loop: the next attempt's reasoning is grounded in concrete, byte-verified evidence of what failed under the real evaluator.

### 5.7 Reward Spec

The reward spec (`config/reward_spec.verified_outcome.json`) is a six-component weighted scoring function operating on the verified outcome of an attempt:

| Component | Feature | Weight |
|---|---|---|
| `correctness` | `correctness` | +4.0 |
| `verification_confidence` | `verification_confidence` | +2.0 |
| `risk_penalty` | `risk` | -2.0 |
| `incident_penalty` | `incident_present` | -3.0 |
| `policy_missing_penalty` | `policy_missing` | -1.0 |
| `training_value` | `training_value` | +1.0 |

Bounds are `[-10.0, +10.0]`. The Rust loop forwards the spec file to `target/debug/context-graph blackbox reward compile --input <reward_spec>` after every attempt.

### 5.8 Continuous Run-Until-Resolved + GPU VRAM Budget

The loop accepts `--run-until-resolved`. When set, the attempt counter is uncapped and the loop only terminates on `solved == true` (an attempt's official evidence has `resolved_ids` containing the instance) *or* a fail-closed error. Continuous mode requires `--official-eval-each-attempt`; without official Docker readback there is no oracle and the loop refuses to enter continuous mode.

GPU VRAM is read from `nvidia-smi --query-gpu=index,name,uuid,memory.total,memory.used,memory.free,driver_version` before and after every Ollama call.

| Flag | Default | Purpose |
|---|---|---|
| `--gpu-index` | 0 | Which GPU to read. |
| `--gpu-min-free-mb` | 8192 | Refuse to call model when free < this. |
| `--gpu-max-used-mb` | unset | Optional ceiling on used VRAM. |
| `--ollama-keep-alive` | `"30m"` | Passed to Ollama `keep_alive` so the model isn't reloaded each attempt. |

A `compute_processes_raw` snapshot (`nvidia-smi --query-compute-apps=pid,process_name,used_memory`) is captured alongside, so VRAM blame can be assigned post-hoc when contention is observed.

### 5.9 Smoke Curriculum

The initial smoke curriculum (`gemmafrontend/swe_smoke_benchmark_suite.json`) is intentionally tiny — two instances — and its expansion is gated on manual review:

| Field | Value |
|---|---|
| `suite_id` | `contextgraph_swe_lite_smoke_v1` |
| `dataset` | `princeton-nlp/SWE-bench_Lite` |
| `split` | `test` |
| `limits.max_models_per_batch` | 1 |
| `limits.max_workers` | 1 |
| `limits.max_attempts_per_model_task_run` | 1 |
| `limits.official_timeout_seconds` | 900 |
| `limits.expand_only_after_manual_review` | true |

Selected instances: `psf__requests-2317` (8 FAIL_TO_PASS, baseline `official_unresolved`) and `django__django-12908` (2 FAIL_TO_PASS, baseline `not_yet_run`). Promotion rules require: official Docker readback persisted into BlackBoxFix for every selected instance; failure evidence from official logs compacted into the next model prompt; at least one task showing reward improvement across attempts; Docker cache manually inspected. The `do_not_claim_success_until` clause re-asserts the official-resolved-ids predicate.

---

## 6 The Role of ME-JEPA (DynamicJEPA)

Where the Reality Loop owns *external* truth (Docker), ME-JEPA owns *internal* prediction quality. It is a Joint-Embedding Predictive Architecture — Yann LeCun's term for a self-supervised predictor that operates on *latent* representations rather than reconstructing pixels or tokens — implemented in pure Rust on Candle 0.10.2 with CUDA. The codebase calls it `DynamicJEPA` because its production variant predicts the next *latent panel* from a `(panel, action)` pair, learning the system's dynamics rather than its appearance.

### 6.1 Architecture

```
input panel x ──▶ online encoder f_θ ──▶ z_online
                                            │
                              concat(z, action)
                                            │
                                       predictor g_φ ──▶ ẑ
                                                          │
target panel x' ─▶ target encoder f_ξ (EMA, detached) ──▶ z_target
```

| Component | Type | Update rule |
|---|---|---|
| Online encoder `f_θ` | MLP (ReLU between layers) | AdamW gradient |
| Target encoder `f_ξ` | MLP, same shape, **detached** | EMA: `ξ ← m·ξ + (1-m)·θ` |
| Predictor `g_φ` | MLP, input = latent ⊕ action | AdamW gradient |

Why a JEPA, and not an autoregressive token predictor? Three reasons:

1. **Predicting in latent space is robust to surface variation.** Two sequences of pixel/token observations that are functionally equivalent can have wildly different surface forms; predicting in a learned latent compresses away the irrelevant variation.
2. **The target encoder is trained without gradient leakage.** The EMA target encoder receives no gradient — only an exponential moving average of the online encoder's weights. Stop-gradient on `z_target` is applied via `.detach()`. This prevents the trivial collapse "make every encoding the same so prediction is easy."
3. **The architecture is small.** ME-JEPA's MLP totals well under a million parameters at default config. It is a *world model*, not a foundation model. It runs in real time on the same GPU that holds the embedding subsystem and the LLM.

### 6.2 Loss

The loss is a weighted sum of three terms:

| Term | Formula | Purpose |
|---|---|---|
| `latent_mse` | `‖ẑ − stop_grad(z_target)‖²` mean | Drive prediction toward target latent. |
| `vicreg_variance` | `mean(relu(target_std − sqrt(var(z_online) + 1e-4)))` | Prevent latent collapse to a constant. |
| `vicreg_covariance` | off-diagonal `‖cov(z_online)‖²_F / latent_dim` | Decorrelate latent dimensions. |

VICReg (variance-invariance-covariance regularisation) is the canonical defence against representation collapse: even if MSE alone could be minimised by learning a constant function, the variance and covariance terms make that solution infeasible. AdamW updates only `online.encoder.params() ∪ predictor.params()` — the target encoder is updated solely via EMA.

### 6.3 Hard Constraints

The `TrainConfig` validator (`config.rs:131-232`) enforces several hard constraints that, together, encode the operational guarantees of the world model:

- `model.predictor.out_dim` MUST equal `model.encoder.out_dim`. The predictor lives in latent space, not panel space; its output is compared against the target encoder's output, not raw pixels.
- `model.ema_momentum` MUST be in `[0.0, 1.0)`.
- `optim.kind` MUST be `"adamw"`.
- `schedule.device` MUST be `"cuda"`. CPU fallback is **explicitly forbidden**. The Phase-7 demo refuses to run on machines without compute capability 12.0.
- `seed` is required — every training run is reproducible bit-for-bit on identical hardware.

These constraints are not soft suggestions; they are validated on entry to `train_dataset` and propagate as `Validation { field, message, remediation }` errors when violated.

### 6.4 Persistence and the 28 Column Families

All ME-JEPA state — domain packs, raw events, normalised states, actions, outcomes, transitions, instrument readings, latent panels, pairwise readings, constellations, threshold calibrations, bindings, trajectories, dataset shards, training runs, model artefacts, predictions, skill policies, plan traces, guard decisions, surprise events, verification runs, and an audit log — is persisted in 28 RocksDB column families prefixed `dj_`. None of them overlap with the teleological / content data plane. After every persistence step, the service performs an *immediate readback* and bincode equality check against the in-memory record; mismatch returns `StorageInvariantViolation`.

Every record carries a `DjRecordHeader { id, version, domain_pack_id, domain_pack_version, created_at_unix_ms, run_id, content_hash }`. `refresh_content_hash()` is called before write; readback equality is enforced. This is the same byte-equal-readback discipline as the Reality Loop, applied to a different domain.

Per training run, the artefact root is `<base_root>/<domain_id>/<training_run_id>` and *must not exist* before the run begins (`ensure_clean_run_dir` errors if it does). The directory contains:

- `model.safetensors` — full tensor map, with `target.encoder.layerN.weight` and `online.encoder.layerN.weight` stored separately so an inference-time EMA snapshot can be reconstructed.
- `config.json` — canonical `TrainConfig` JSON.
- `evaluation_report.json` — the full `EvaluationReport` (every split's `latent_mse`, `latent_cosine`, `action_contrast_acc`, `shuffled_target_cosine`, plus the EpochMetric history).

`ModelArtifactRecord.files: Vec<ArtifactFile>` stores `relative_path`, `sha256: [u8; 32]`, `size_bytes` for every file. `verify_artifact_files` recomputes hashes and refuses any mismatch. `load_artifact_for_inference` rejects on corrupt safetensors and stores `model_file_sha256` for inclusion in `PredictionRecord.model_artifact_hash_at_inference`. *Every prediction is hash-bound to the exact artefact that produced it.*

### 6.5 Predict, Plan, Guard, Surprise

The ME-JEPA service exposes four primary verbs. Each persists into RocksDB with audit and readback verification.

| Function | Inputs | Persists |
|---|---|---|
| `train_dataset` | `dataset_id`, `&TrainConfig`, `artifact_root` | `CF_DJ_TRAINING_RUNS` (Started → Completed/Failed), `CF_DJ_MODEL_ARTIFACTS`, `CF_DJ_AUDIT_LOG` + safetensors files |
| `predict_next_panel` | `artifact_id`, `panel_id`, `action_id` | `CF_DJ_PREDICTIONS` + audit |
| `plan_next_action` | `artifact_id`, `panel_id`, `skill_id` | `CF_DJ_ACTIONS` (candidates), `CF_DJ_PREDICTIONS`, `CF_DJ_GUARD_DECISIONS`, `CF_DJ_PLAN_TRACES`, optional `CF_DJ_SKILL_POLICIES`, audit |
| `record_surprise` | `prediction_id`, `observed_outcome_id`, `observed_panel_id` | `CF_DJ_SURPRISE_EVENTS` + audit (only when `cosine < threshold`) |

**Planning** enumerates candidate actions per the domain's `planner_policy.candidate_actions` (e.g. `EnumeratedDeltas { deltas: [-1, 0, 1] }` for counter-world or `EnumeratedMoves { moves: ["up","down","left","right"] }` for gridworld). For each candidate, the model predicts the next latent panel, computes the utility, and runs every guard in `planner_policy.guards`. The highest-utility allowed action is selected; if every guard rejects every candidate and a constellation fallback is registered, the highest-utility candidate is selected anyway and `no_accepted_candidate = true` is recorded.

**Guard decisions** combine a *bounds check* (does the predicted next state violate the schema's per-field min/max?) and an optional *G_τ check* against per-modality calibrated thresholds. The `GuardDecisionRecord` stores both decisions, the cosine to centroid per modality, the τ values, the failed modalities, and the `constellation_uuid` used. Nothing about a guard decision is implicit; every datum is named in the record.

**Surprise** is the most epistemically interesting verb. After an action is taken and a new panel is observed, `record_surprise` re-encodes the observed panel under the same artefact via the target encoder, computes `cosine(prediction.predicted_next_panel_vec, observed_vec)`, compares against the threshold from `domain.planner_policy.surprise_threshold`, and persists a `SurpriseEventRecord { surprise_kind: UnexpectedOutcome, cosine, threshold, error_norm }` only when the cosine falls below threshold. Surprise events are the system's way of saying: *here, my world model was wrong; remember this*.

### 6.6 Mutual Information Audit (KSG-1)

Joint-embedding architectures are notorious for partial collapse — slots of the latent vector that carry no information independent of other slots. ME-JEPA ships a pairwise mutual-information audit using the KSG-1 estimator. For every pair of panel slots:

1. Validate sample size (≥ 50) and finite values.
2. Compute local MI contributions from k-nearest-neighbor distances.
3. `mi_nats = mean(local_mi_nats)`.
4. Normalise by `min(x_entropy, y_entropy)` and clamp to `[0, 1]`.

The summary records `n_redundant_pairs_loose` (MI > 0.7) and `n_redundant_pairs_strict` (MI > 0.9). When too many pairs are redundant, the latent space is collapsing and the architecture's value is suspect. The audit is exposed as `dynamicjepa_audit_pairwise_mi` in the MCP tool surface; the Phase-7 demo flags the result alongside the training report.

### 6.7 Cross-Domain Transfer

A single ME-JEPA artefact is bound to a domain pack — counter-world, gridworld_5x5, career taxonomy, or counter-to-grid bridge. The `cross_domain_transfer` service runs a counter-world → gridworld pilot with paired bootstrap confidence intervals across five seeds. The output evidence (`transfer_results.json`, paper table, plots, bridge artefacts) provides quantitative answers to "did the world model learned in one domain help in another?" The configuration is fail-closed — `output_root` must not exist (so evidence is immutable), seeds must be unique, source/target events must be ≥ 20 each, bootstrap iterations must be > 0, and bridge instrument count must match `validate_counter_to_grid_bridge_instrument_count`.

### 6.8 How ME-JEPA Complements the Reality Loop

The Reality Loop reduces uncertainty about whether the *outside* world agrees with a claim. ME-JEPA reduces uncertainty about whether the *internal* world model is calibrated. Used together, the two engines support a richer epistemic posture:

- The Reality Loop refuses success without external evidence — Docker says yes or there is no run.
- ME-JEPA accumulates surprise events whenever observed outcomes diverge from predicted ones — surprise is *not* failure, but it *is* a signal that the world model needs updating.
- Surprise events are persisted with the same byte-equal-readback discipline as the Reality Loop's evidence files, on the same single audit substrate (BlackBoxFix SQLite + reality-events.jsonl).
- Neither subsystem can claim success without the other's source-of-truth confirmation in scope. A claim of "the agent learned to repair X" is grounded in *both* the Reality Loop's official Docker pass *and* ME-JEPA's reduced surprise rate over the same task family.

This separation matters. Conflating "the test passed" with "the model understands" is a category error. The Reality Loop tests external truth. ME-JEPA tests internal truth. Both must hold for the system to advance its claim.

---

## 7 Evidence and Invariants

A white paper that asserts properties without exhibiting evidence is a brochure. This section enumerates the verifiable artefacts the system produces.

### 7.1 The Audit Substrate

Every persistence step in the Reality Loop is double-bound to an append-only ledger:

- **BlackBoxFix SQLite ledger** (`LEDGER_SCHEMA_VERSION = 14`) — accessible via `target/debug/context-graph blackbox` subcommand. Contains run/model/tool/verifier/score/reward records, each with SHA-256 read-after-write verification.
- **Reality event stream** (`reality-events.jsonl`) — append-only JSONL. Each row carries `loop_id`, `event_type`, `status`, `payload`, and `payload_sha256`. Appends are byte-equal-verified before the next event is written.

ME-JEPA writes to its own `dj_audit_log` column family (24-byte composite key `[ts_BE:8][audit_id:16]`, `DJ_AUDIT_RECORD_VERSION = 2`). Every operation — `train_predictor`, `register_artifact`, `predict`, `plan`, `record_surprise`, `register_domain_pack` — emits a `DjAuditRecord { operation, status, inputs, outputs, touched_cfs, content_hashes, signal_yield }`. Failed runs are also audited: `service.rs` writes a `Failed` row even when the original `Started` row was already persisted, so every state transition is in the log.

The two audit streams are independent but aligned: both are append-only, both use SHA-256 content addressing, both refuse to ignore a failed write, and both can be replayed to reconstruct the run's state at any point in time.

### 7.2 Per-Artefact Source of Truth

The entire artefact tree under `<runtime_root>/<task_id>/attempt-N/` exposes its source of truth in plain text:

```
<runtime_root>/<task_id>/attempt-N/
  problem-reality-packet.json
  reality-signal-packet.json
  attempt-summary.json
  agent-tool-transcript.json
  validation.json
  official-evidence/<instance>.json
  harness-transitions/round-NN.json
  trigger-decision.json
  claude-code-optimizer/
    recommendation-turn-NN.json
    decision-turn-NN.json
    commands/<unix>-<command>.json
```

Source-of-truth file paths in tool output always start with `file:`. Any operator with read access to `<runtime_root>` can reconstruct exactly what happened on which attempt without privileged access to a database or a live process.

### 7.3 The Smoke Curriculum's Evidence

The smoke curriculum's two instances — `psf__requests-2317` and `django__django-12908` — were chosen to make the predicate testable on a small budget. `psf__requests-2317` has 8 FAIL_TO_PASS tests and a baseline status of `official_unresolved`; the loop's job is to drive the baseline from `official_unresolved` to `resolved`. `django__django-12908` has 2 FAIL_TO_PASS tests and a baseline status of `not_yet_run`; a clean run produces the first official evidence row for that instance. *Promotion to a larger curriculum is gated on manual review of these official artefacts*.

### 7.4 Static Verification Numbers

The verification report (`docs2/contextgraphspecs/15_verification_report.md`) records the static numbers a third-party reviewer can confirm by reading the source:

| Metric | Value | Source |
|---|---|---|
| Workspace crates | 11 | `Cargo.toml` `[workspace.members]` |
| All `.rs` files in repo | 1,346 | `find . -name '*.rs' -not -path '*/target/*'` |
| Total Rust LOC | ~554,400 | source line count |
| `#[test]` / `#[tokio::test]` attributes | 5,437 | grep |
| Most recent test run (Audit #15) | 6,527 passed, 0 failed | recorded 2026-02-26 |
| MCP tools (`llm` feature on) | 145 | `tools/definitions/mod.rs:135` |
| RocksDB column families | 101 | `column_families.rs:408` |
| Reality-loop binary LOC | 2,385 | `bin/reality-loop.rs` |
| BlackBoxFix ledger schema version | 14 | `commands/blackbox.rs:40` |
| Reality measurement contract schema | `contextgraph_reality_measurement_contract_v1` | `bin/reality-loop.rs:750` |
| DynamicJEPA column families | 28 | `dynamicjepa/column_families.rs:74` |
| ME-JEPA training run record version | 1 | `predictor.rs:10` |

These numbers will drift as the codebase evolves; the source files cited remain the authoritative location.

### 7.5 What an Operator Sees

When an attempt succeeds, the loop prints `RUST_REALITY_LOOP_SUMMARY=<summary-source-of-truth>` to stdout. When an attempt fails, it prints `RUST_REALITY_LOOP_ERROR_SOURCE_OF_TRUTH=<failure-artifact-path>` to stderr. Both line prefixes are stable for shell consumers. The operator does not need to scrape logs — the program tells them, on a single line, which file to read for the full story.

---

## 8 Constraints, Tradeoffs, Limitations

Honest white papers name their boundaries.

### 8.1 Single-Writer

ContextGraph enforces a single-writer discipline through a Unix `flock(LOCK_EX | LOCK_NB)` on `<storage.path>/mcp.pid`. On non-Unix systems the lock is *not* acquired — the warning `E_PID_NO_LOCK` is logged, and the operator is responsible for ensuring no two processes write at once. This is a hard limit of the current implementation; running the system on Windows without WSL2 risks RocksDB corruption.

### 8.2 Hardware Dependency

Both the LLM-driven discovery agents (`Hermes-2-Pro-Mistral-7B` for causal/graph discovery) and ME-JEPA require GPU. ME-JEPA's `TrainConfig::validate` rejects any configuration where `schedule.device != "cuda"`; the Phase-7 demo refuses to run without compute capability 12.0. The Reality Loop refuses to call any model when free VRAM is below `--gpu-min-free-mb` (default 8 GB). Operators on CPU-only or pre-Blackwell hardware cannot run the full stack today.

### 8.3 Docker Dependency for the Reality Loop

The Reality Loop refuses to enter the model loop without `--official-eval-each-attempt`. That flag in turn requires a working Docker daemon and the official SWE-bench harness installed at `/mnt/d/contextgraph/cache/contextgraph-swebench/venv` (or wherever `--swebench-python` points). The system explicitly does not implement a "CI-only" or "Docker-free" path; that would amount to building an alternative oracle, which would re-introduce the verification gap the project was built to close.

### 8.4 Single Local Ollama Model

The Reality Loop is bound to a single local Ollama model: `qwen3.6:27b` (cryptographically pinned via `config/ccreality_model_pins.json` and validated at load time by `validate_inner_model_identity` in `reality-loop.rs:2592`). Earlier iterations of the project carried plumbing for additional models (`qwen3:8b`, `mistral-nemo:12b`); that scope was explicitly removed when the loop refactored to a single-model design. The prompt contract remains model-agnostic in principle, so a future operator could re-introduce additional local models, but doing so requires updating the model-pin gate, the harness identity check, and the smoke curriculum in lockstep. Remote API models are out of scope: network introduces uncertainty (rate limits, transient errors, response-latency tail risk) that is hostile to the loop's discipline. Local Ollama is the production posture.

### 8.5 No Backwards Compatibility

ContextGraph is pre-production; synthetic data only. When a record version changes, the database is wiped and rebuilt — there is no migration path. This is a deliberate optimisation against the failure mode "support both old and new shapes forever, accumulating implicit invariants." It is also a real limitation for any operator who wants to preserve historical training data across schema bumps.

### 8.6 Rust-Only Reality Loop

The previous Python SWE-bench loop, Python official-eval bridge, and Python prompt compiler were deleted. This was a deliberate consolidation: split ownership — Python and Rust both implementing the same loop — created subtle drift (different prompt behaviours, different success thresholds, different evidence formats). The current architecture has one canonical implementation. Users who need custom integration must either drive `reality-loop` from the outside or fork the binary.

### 8.7 ME-JEPA Phase Coverage

The ME-JEPA service currently supports `objective.kind = "latent_delta"` only. Phase 3's guard system supports `guards = ["bounds_check"]` only. Phase 7 covers training; Phase 8 covers prediction, planning, and surprise. Other JEPA loss families (predictive coding variants, multi-step rollouts) are out of scope for the current public release. The 5090jepa demo plan in `docs/5090jepa/` enumerates the planned expansion.

### 8.8 ME-JEPA Requires CUDA

Same caveat as §8.2 applied specifically: there is no CPU fallback, no quantisation path, no fp32 emulation. This is fine for the production target (RTX 5090) and a hard wall for any reproducer without compatible hardware.

---

## 9 Privacy, Security, Maintainability

### 9.1 No Outbound HTTP Except MCP

The Reality Loop never calls any HTTP API directly — no curl, no wget. The MCP server is the only network/IO surface. Tools that wrap external services (Ollama, the official SWE-bench evaluator) are invoked via local sockets (Unix domain socket for Docker, localhost HTTP for Ollama). This is enforced both by the `.claude/settings.json` deny list and by the absence of any reqwest call in `reality-loop.rs` that targets an off-host URL.

### 9.2 Deny-Listed Files

The system deny-lists `.env`, `swebench/**`, `tests/oracle_**`, `crates/context-graph-benchmark/datasets/swe_bench_*`, `crates/*/src/reward_specs/**`, and `crates/*/src/verifiers/**`. These deny rules are enforced both at the harness level (`is_forbidden_benchmark_mutation`) and at the `.claude/settings.json` level. Bypassing them is treated as a hard failure (`RUST_REALITY_PATCH_TOUCHES_BENCHMARK`), not a warning.

### 9.3 Append-Only Audit Logs

The audit log column families `audit_log`, `audit_by_target`, `dj_audit_log`, `merge_history`, and `importance_history` are append-only by design. Audit and merge-history rows are *permanent*; they never expire and are never deleted. This makes lineage tracking irreversible in the audit sense: every operation that ever touched a memory or a learner record is queryable forever.

### 9.4 Hash-Bound Predictions

Every ME-JEPA prediction record carries `model_artifact_hash_at_inference` — the SHA-256 of the artefact file used to produce it. If the artefact is later replaced or corrupted, the prediction can still be traced back to the exact model version that produced it. This is a defense in depth against silent model drift.

### 9.5 Permanent Merge History

Phase-4 merge history is not garbage-collected. When two memories are merged, the `MergeRecord` JSON entry persists forever. This is the inverse of the "merge into one and forget" pattern: ContextGraph treats every merge as a fact about the past that future analyses may need.

### 9.6 Importance History

Every change to a memory's `importance` writes an `ImportanceChangeRecord` JSON row. Like merge history, these records are permanent. If an operator boosts a memory's importance because of an event, the audit trail records *what* was boosted, *when*, *by whom* (when operator attribution is enabled), and *why* (when the action carries a reason).

### 9.7 Operator Attribution

The provenance plan adds `created_by` and `operator_id` to every MCP tool call, every merge, and every importance change. Attribution is opt-in but recommended for any deployment where audit accountability matters.

### 9.8 Maintainability

The codebase publishes a hard upper bound on file size: 1,500 lines per `.rs` file (enforced by `file-length-manager` agent in CI). Files that grow past this limit are split with cross-references preserved. This is a maintainability constraint, not a style preference: large files in Rust accumulate trait impls and macro expansions that resist refactoring.

The full repository is `cargo build --release --workspace` clean: zero errors, zero warnings, `cargo clippy --workspace --all-targets -- -D warnings` clean. The lint gate is enforced in CI. The most recent test verification (Audit #15) ran 6,527 tests with 0 failures.

---

## 10 Conclusion

The ContextGraph Reality Loop is a small, opinionated piece of infrastructure that takes the success criterion away from the agent. It refuses to declare victory unless an external Docker harness places the target SWE-bench instance in `resolved_ids`. Every artefact along the way is byte-equal-readback verified before the next decision is allowed to use it. Every failure is a typed error with a remediation message. Every event is appended to an audit log that survives the run.

ME-JEPA — the Joint-Embedding Predictive Architecture the codebase calls DynamicJEPA — sits beside the loop. It learns latent dynamics from instrument readings, predicts the next latent panel from `(panel, action)` pairs, and persists *surprise events* whenever observed outcomes diverge from predictions. It does not replace the Reality Loop's external oracle; it does not pretend to. Instead, it gives the system a calibrated internal world model, and the surprise stream gives operators a reason to investigate when the model and the world disagree.

The two subsystems are aligned in discipline: byte-equal readback, append-only audits, hash-bound artefacts, fail-closed gates, deny-by-default scopes, no silent fallback. Used together, they support a coding-agent runtime in which capability claims are grounded in measurement, not in the model's natural-language confidence.

This is not a finished product. The smoke curriculum is two instances. ME-JEPA's loss family is intentionally narrow. The Rust-only enforcement is a deliberate consolidation, not a long-term endorsement of single-implementation discipline. The hardware dependencies — RTX 5090, CUDA 13.2, Docker for SWE-bench — are real walls between the current artefact and a wider audience.

What we hope a careful reader takes away is structural rather than feature-by-feature: the verification gap that haunts modern coding agents is closeable, the closing tools are mostly fail-closed boundaries plus disciplined provenance, and the extra cost of running them is small relative to the cost of false positive capability claims. The code is in `crates/context-graph-cli/src/bin/reality-loop.rs` and `crates/context-graph-dynamicjepa/`. The contracts are in `<run_root>/measurement-contract.json`. The evidence is in `reality-events.jsonl` and the BlackBoxFix SQLite ledger. The next attempt is one shell command away.

---

## References

1. **ContextGraph specifications** — `docs2/contextgraphspecs/`. Twenty-two markdown files covering the 11-crate Rust workspace, including `01_system_overview.md`, `21_dynamicjepa_subsystem.md`, and `22_reality_loop_subsystem.md`. Authoritative source for every constant, column family, MCP tool, and code path cited above.
2. **The reality-shift principle** — `01b_reality_shift_principle.md` in `docs/ccreality/`. Establishes "no alteration without current state, no alteration without a delta back into perception" as the runtime invariant.
3. **JEPA (Joint-Embedding Predictive Architecture)** — Yann LeCun, "A Path Towards Autonomous Machine Intelligence" (2022). Original framing of latent-space prediction with non-contrastive self-supervision.
4. **VICReg** — Bardes, A., Ponce, J., LeCun, Y. (2022) "VICReg: Variance-Invariance-Covariance Regularization for Self-Supervised Learning." ICLR 2022. The variance and covariance terms used in ME-JEPA's loss.
5. **SWE-bench Lite** — Jimenez, C. E. et al. "SWE-bench: Can Language Models Resolve Real-World GitHub Issues?" ICLR 2024. The dataset and oracle the Reality Loop targets. Source of truth: `princeton-nlp/SWE-bench_Lite` on Hugging Face.
6. **EMA target encoders / BYOL** — Grill, J.-B. et al. "Bootstrap Your Own Latent" (2020). The EMA-target-encoder pattern ME-JEPA inherits.
7. **KSG-1 mutual information estimator** — Kraskov, A., Stögbauer, H., Grassberger, P. "Estimating Mutual Information" (2004). The MI estimator used in ME-JEPA's pairwise audit.
8. **ContextGraph project repository** — `crates/context-graph-cli/src/bin/reality-loop.rs` (2,385 LOC), `crates/context-graph-dynamicjepa/` (Phase 7/8 service + Candle CUDA training loop), `gemmafrontend/server.py` (3,879 LOC; Python evidence console). All source files in this repository.

---

## Appendices

- **Appendix A** — Architecture diagrams and component tables: see [`appendix_a_architecture.md`](appendix_a_architecture.md).
- **Appendix B** — Fail-closed gate inventory (37 gates across the Reality Loop): see [`appendix_b_invariants.md`](appendix_b_invariants.md).
- **Appendix C** — Glossary of project-specific terms: see [`appendix_c_glossary.md`](appendix_c_glossary.md).
