---
title: "ContextGraph Reality Loop: A Measurement-Bound Software-Repair Engine with an In-Process Gemma Harness and a Joint-Embedding Predictive World Model"
subtitle: "A Software-Architecture White Paper on Fail-Closed Software Repair, Cryptographic Model Pinning, Grammar-Constrained Tool Calls, ME-JEPA Latent Dynamics, and Durable Provenance"
author: "Chris Royse\\thanks{Independent Research --- ContextGraph Project. Email: \\texttt{thenumberonellc@gmail.com}. Source repository: \\texttt{contextgraph} workspace, branch \\texttt{ccreality-cutover}.}"
date: "May 6, 2026 \\quad Version 1.1"
keywords:
  - "Software repair agents"
  - "SWE-bench"
  - "Joint-Embedding Predictive Architecture (JEPA)"
  - "Fail-closed verification"
  - "GBNF grammar-constrained tool calls"
  - "Cryptographic model pinning"
  - "Provenance and audit logs"
abstract: |
  Modern coding agents that drive code edits and tool calls increasingly speak in the language of certainty --- "fixed", "passes", "ready to merge" --- without owning a primitive that proves any of those words. The result is a quiet, expensive class of failures: patches that look right but never apply, tests that pass locally and fail in continuous integration, and capabilities reported by self-confidence rather than by an oracle. ContextGraph's *Reality Loop* is a Rust subsystem that refuses to declare success unless the official SWE-bench Lite Docker evaluator places the target instance in `resolved_ids`. Every artefact along the way --- the prompt, the model response, the candidate patch, the per-attempt evidence, the audit row --- is byte-equal-readback verified before it is permitted to influence the next decision. The model itself is loaded in-process via the llama.cpp Rust bindings (\texttt{llama-cpp-2}) from a cryptographically-pinned BF16 GGUF of \texttt{gemma-4-E4B-it}, with tool calls constrained by a GBNF grammar emitted by the chat template so the model literally cannot emit malformed tool syntax. Alongside the loop, ContextGraph ships a Joint-Embedding Predictive Architecture (called *ME-JEPA* in this paper, *DynamicJEPA* in the codebase) that learns latent dynamics from instrument readings and emits *surprise events* whenever the world disagrees with its predictions. Together, the two engines turn a coding agent from a confident narrator into a measurement-grounded participant: it perceives current state, alters it through governed grammar-bounded tools, observes a real outcome, and persists the delta on an append-only audit substrate. This paper documents how the loop and the world model are built, why specific design decisions were made, and what evidence the system produces to back its claims.
documentclass: article
classoption:
  - 11pt
  - a4paper
geometry:
  - margin=1in
fontfamily: lmodern
linkcolor: NavyBlue
urlcolor: NavyBlue
citecolor: NavyBlue
toccolor: black
header-includes:
  - \usepackage{xcolor}
  - \usepackage[dvipsnames]{xcolor}
  - \usepackage{fancyhdr}
  - \pagestyle{fancy}
  - \fancyhf{}
  - \fancyhead[L]{\small ContextGraph Reality Loop --- v1.1}
  - \fancyhead[R]{\small Royse, 2026}
  - \fancyfoot[C]{\small \thepage}
  - \renewcommand{\headrulewidth}{0.4pt}
  - \usepackage{microtype}
  - \usepackage{enumitem}
  - \setlist{topsep=4pt,itemsep=2pt}
  - \usepackage{caption}
  - \captionsetup{font=small,labelfont=bf}
  - \usepackage{fvextra}
  - \DefineVerbatimEnvironment{verbatim}{Verbatim}{fontsize=\scriptsize,breaklines=true,breakanywhere=true,breaksymbolleft=}
  - \fvset{fontsize=\scriptsize,breaklines=true,breakanywhere=true}
  - \usepackage{etoolbox}
  - \AtBeginEnvironment{longtable}{\footnotesize}
  - \AtBeginEnvironment{tabular}{\footnotesize}
  - \setlength{\tabcolsep}{4pt}
  - \renewcommand{\arraystretch}{1.05}
  - \usepackage{setspace}
  - \onehalfspacing
  - \emergencystretch=4em
  - \tolerance=2000
  - \hbadness=10000
  - \usepackage{titlesec}
  - \titlespacing*{\section}{0pt}{12pt}{4pt}
  - \titlespacing*{\subsection}{0pt}{8pt}{2pt}
toc: true
toc-depth: 2
numbersections: true
---

\clearpage

# Background and Context {#sec:background}

ContextGraph is an 11-crate Rust workspace that stores text and code as 14-embedder semantic fingerprints in a single RocksDB database (101 column families) and exposes them through 141--145 Model Context Protocol tools [8]. It runs locally on commodity GPU hardware (the development target is the NVIDIA RTX 5090 / Blackwell architecture, sm_120a, CUDA 13.2) and is built primarily to act as a durable, queryable memory substrate for AI agents.

Two of its subsystems push beyond memory:

- The **Reality Loop** (`crates/context-graph-cli/src/bin/reality-loop.rs`) is a self-contained `tokio::main` Rust binary that drives a single local Gemma model through SWE-bench Lite repair attempts. The model is loaded *in-process* through the llama.cpp Rust bindings (`llama-cpp-2`) [10] from a BF16 GGUF of `gemma-4-E4B-it`. The Reality Loop's single, immovable definition of success is whether the official SWE-bench Docker harness reports the target instance in `resolved_ids` [5,6].
- **DynamicJEPA / ME-JEPA** (`crates/context-graph-dynamicjepa/`) is a 28-column-family Joint-Embedding Predictive Architecture trained on Candle 0.10.2 with CUDA. It learns to predict the next latent panel from a `(panel, action)` pair [3] and persists *surprise events* whenever observed reality diverges from its prediction.

Both subsystems share a single audit substrate --- a BlackBoxFix SQLite ledger at `LEDGER_SCHEMA_VERSION = 14` and an append-only `reality-events.jsonl` event stream. Neither subsystem can claim success without the other's source-of-truth confirmation in scope. This separation is intentional: the Reality Loop owns *external* truth (Docker), the world model owns *internal* prediction quality, and the audit substrate is the only place where the two meet.

# The Verification Gap in Coding Agents {#sec:problem}

Industry-standard coding agents are end-to-end black boxes from the operator's point of view. The agent ingests a problem statement, emits a candidate solution, and (optionally) reports a confidence score. Three failure modes recur in this paradigm:

1. **Claim drift.** The agent's natural-language report ("I have applied the fix") is not coupled to anything verifiable. The fix may not exist in the working tree; the patch may not parse; the unit test it claims to pass may not exist. There is no read-after-write step between the model's *assertion* and the operator's *belief*.
2. **Oracle drift.** Even when a verifier is present, it is often the agent's own test of its own work. Self-graded test passes are not safe to treat as success because the graders, the prompts, and the test invocation can all be manipulated by the agent under pressure to succeed. Without an *external*, *immutable* oracle, every claim is suspect.
3. **Capability inflation.** Reports aggregated across many runs amplify both signal and noise. If 1 in 8 runs of an agent self-grades as a success and the system promotes those runs without external verification, the public-facing capability score drifts upward over time without any underlying improvement.

These failure modes are not pathologies of any particular model. They are predictable consequences of *agents that own their own success criteria*. The fix is not better prompting or more capable models; it is to take the success criterion away from the agent entirely.

# Problem Statement {#sec:requirements}

A coding agent operating in a real environment must satisfy a set of properties that no ungoverned LLM stack provides out of the box:

| Property | Meaning |
|----------------------|------------------------------------------------|
| **External oracle** | Success is defined by a process the agent does not control. |
| **Read-after-write provenance** | Every artefact (prompt, response, patch, evidence) is verified to exist on disk with the bytes the system intended to write. |
| **Bounded edit surface** | The agent edits a small, declared set of paths; the verifier, the harness, the dataset, the reward spec, and the test suite are immutable from the agent's perspective. |
| **Fail-closed gates** | Any unmet precondition produces a typed error and aborts the run *before* the model is invoked. |
| **Cryptographically pinned model** | The exact bytes of the model file are validated before inference begins. |
| **Grammar-bounded outputs** | The model's tool-call syntax is constrained by a formal grammar so it cannot produce malformed calls. |
| **Auditable history** | Every artefact is persisted into an append-only ledger keyed to the run, so two engineers reading the database from different machines see the same story. |
| **No silent fallback** | When a required input is missing, the system refuses to proceed rather than substituting a default. |

The problem this paper addresses is: *how do you build a coding-agent runtime that satisfies all of these properties simultaneously, runs locally on commodity hardware, and produces evidence durable enough to be auditable months after the run?*

# Criteria for a Good Solution {#sec:criteria}

The ContextGraph Reality Loop was designed under the following criteria, all of which are enforced in code rather than only in documentation.

## Source-of-truth before action

No alter call may execute without first perceiving current state. Concretely, before any file is edited, the harness opens a window over the current bytes and records the SHA. After the alter call, the harness re-reads those bytes and confirms the new SHA. The session-level shift log records every edit. This is the *reality-shift principle* [2]: "no alteration without current state, no alteration without a delta back into perception."

## One immovable success predicate

The success predicate for SWE-bench is, by construction, the only one the loop honours: *the official Docker SWE-bench report's `resolved_ids` array contains the target instance* [5]. The loop refuses to enter "run-until-resolved" mode without `--official-eval-each-attempt`; without the official Docker evaluator there is no oracle, and so there is no run.

## Cryptographic model identity

Before the harness loads any GGUF, the loop reads `config/ccreality_model_pins.json` and checks four pin fields against the on-disk file: `expected_sha256`, `expected_size_bytes`, `expected_min_context_tokens`, and `expected_identity_substrings`. A mismatch on any field aborts with `RUST_REALITY_GEMMA_MODEL_PIN_*`. The current production pin is `gemma4_e4b_bf16` referencing `gemma-4-E4B-it-bf16.gguf` (15,053,094,880 bytes, SHA-256 prefix `23458339...`, minimum context 128,000 tokens, identity substring `gemma4`).

## Deny-by-default edit boundaries

The model is permitted to edit two things conceptually: a candidate patch and a notes field. The model is forbidden from editing the task manifest, the reward spec, the verifier code, the SWE-bench fixtures, the harness itself, the official evaluator, the BlackBoxFix ledger, and the dataset metadata. Patches that touch any path matching `test_`, `tests/`, `/test_`, `/tests/`, `_test.py`, `conftest.py`, `tox.ini`, or `pytest` are rejected with `RUST_REALITY_PATCH_TOUCHES_BENCHMARK`. The agent's *write* tools (Section 5.4) only operate on the working repository; they cannot reach outside it.

## Grammar-constrained tool calls

Tool calls are constrained by a GBNF grammar emitted by the GGUF's chat template (or supplied by `--gemma-chat-template`). The grammar is written to disk as `tool-grammar.gbnf`, byte-equal-verified, and then loaded into a `LlamaSampler` chain. The model cannot emit a tool call that fails the grammar; the sampler will not produce tokens that violate it. Three grammar variants are supported: strict, lazy-pattern (pattern-driven trigger), and lazy (free-text trigger). When the GGUF/template combination does not produce a tool grammar, the loop aborts with `RUST_REALITY_GEMMA_TOOL_GRAMMAR_MISSING`.

## Byte-equal readback on every persisted artefact

Every JSON write parses back through `serde_json::Value` and is compared structurally against the in-memory record. Every text write compares bytes. Every appended event in `reality-events.jsonl` round-trips and is rejected with `RUST_REALITY_EVENT_READBACK_MISMATCH` if the readback does not match. This catches both filesystem corruption and serialisation regressions before they propagate to the next decision.

## GPU and Docker as first-class preconditions

Before the model is invoked, the loop reads `nvidia-smi` and asserts `free VRAM >= --gpu-min-free-mb` (default 8 GB) and (when set) `used VRAM <= --gpu-max-used-mb`. Before the official evaluator is called, the loop runs a Python-side preflight: `import docker; docker.from_env().version()`. Either failure aborts the run with a typed error and a remediation message; the model is never invoked under degraded conditions.

## No silent fallback

Pre-production policy is: when a record version changes, the database is wiped, not migrated. When a model file is missing or fails the cryptographic pin, the load fails fast, not silently substitutes a base model. When the official SWE-bench report is missing, the run is *blocked*, not classified as "uncertain". The previous Ollama-based driver and Python loop were deleted from the repository to prevent split ownership; the in-process llama.cpp driver is the only inference path.

# The ContextGraph Reality Loop {#sec:reality-loop}

The Reality Loop turns the criteria in Section 4 into a single Rust binary. This section walks through its lifecycle, its measurement contract, its in-process Gemma harness, its agent-tool surface, its prompt compiler, and the artefacts it writes.

## Lifecycle

\begingroup\scriptsize
```
+-----------------------+      +------------------------+      +-------------------------+
| reality-loop.rs       |--->  | target/debug/          |--->  | blackbox.sqlite         |
| (Rust, tokio::main)   |      | context-graph (CLI)    |      | LEDGER_SCHEMA_VERSION=14 |
|                       |      | blackbox subcommand    |      +-------------------------+
|                       |--->  | reality-events.jsonl (append-only, byte-equal readback)
+-----------+-----------+
            |
            v
   In-process Gemma harness (llama-cpp-2 -> llama.cpp)
   - LlamaModel + LlamaContext, all GPU layers, ctx>=128k, batch 2048
   - cryptographic pin (sha256 + size + identity substrings)
   - chat template from GGUF; GBNF tool grammar; greedy/seeded sampler chain
            |
            v
   Governed write tools (apply_line_window_edit, ...) on repo workspace
            |
            v
   git apply --whitespace=nowarn  (--> repo at exact base_commit)
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
\endgroup

A single attempt --- the unit of work the loop orchestrates --- consists of:

1. Validate the task manifest against an immutable measurement contract.
2. Load and cryptographically pin the Gemma GGUF.
3. Compile a deterministic prompt that includes verified failure evidence and the no-repeat-patch hash list.
4. Drive the Gemma harness in-process under a GPU VRAM budget, with tool calls constrained by GBNF grammar.
5. Apply the candidate patch (built from the agent's `apply_*` tool submissions) to the real repository checked out at the exact `base_commit`.
6. Validate fail-closed: no test/harness mutation, no patch repeat, patch must apply with `git apply`.
7. Run the official Docker SWE-bench evaluator and read `resolved_ids` back from the JSON report.
8. Persist every step into the BlackBoxFix SQLite ledger and the JSONL reality event stream.

Each step is fail-closed with a typed `RealityError` carrying `error_code`, `field_path`, `remediation`, `details`, and `source_of_truth`. Failure is logged to `<runtime_root>/failures/failure-<ts>-<code>.json` with readback verification before the process exits non-zero.

## The Measurement Contract

The most consequential file in the system is the JSON object the loop writes to `<run_root>/measurement-contract.json` *before any model is loaded*. It pins the entire claim scope and is signed by SHA-256 read-after-write so it cannot be silently mutated mid-run.

| Field | Value |
|---------------------|---------------------------------------------|
| `schema` | `contextgraph_reality_measurement_contract_v1` |
| `claim_scope.primary_claim` | `official_swe_instance_resolution_only` |
| `claim_scope.success_predicate` | `official_report.resolved_ids contains task.instance_id` |
| `oracle.dataset` | `princeton-nlp/SWE-bench_Lite` |
| `oracle.split` | `test` |
| `oracle.instance_id` | from manifest |
| `oracle.fail_to_pass` | from manifest |
| `oracle.pass_to_pass_count` | integer from manifest |
| `oracle.source_of_truth` | binding statement from manifest |
| `model.driver` | `llama-cpp-2 in-process` |
| `model.gguf_path` | absolute path to GGUF |
| `model.pin_key` | active key in `ccreality_model_pins.json` |
| `model.expected_sha256` | sha256 enforced before load |
| `model.expected_size_bytes` | byte count enforced before load |
| `immutable_boundaries.task_manifest` | `file:<path>` |
| `immutable_boundaries.base_commit` | git SHA |
| `immutable_boundaries.official_evaluator` | `python -m swebench.harness.run_evaluation` |
| `immutable_boundaries.reward_spec` | `file:gemmafrontend/reward_spec.verified_outcome.json` |
| `mandatory_readbacks` | 10 readback IDs |
| `model_editable_scope` | `["candidate.patch", "notes"]` |
| `model_forbidden_scope` | manifest, harness, ledger, reward spec, verifier, tests, fixtures |
| `failure_policy.unmeasured_success` | `blocked` |
| `failure_policy.test_or_harness_mutation` | `failed` |

The 10 mandatory readbacks include `task_manifest_readback`, `measurement_contract_readback`, `gemma_model_pin_match`, `git_head_equals_base_commit`, `git_status_clean_before_attempt`, `candidate_patch_applies_to_base`, `candidate_patch_does_not_modify_tests_or_harness`, `official_docker_sdk_preflight`, `official_swe_report_with_resolved_ids`, and `reality_event_log_append_readback`. Each readback maps to a fail-closed gate enumerated in Appendix B.

## The In-Process Gemma Harness

The harness is the heart of the new architecture. It is a single Rust struct that owns:

- A `LlamaBackend` (one per process; reused across attempts).
- A `LlamaModel` loaded from the GGUF file at `models/gemma4-e4b/gemma-4-E4B-it-bf16.gguf`.
- A `LlamaContext` with `n_ctx >= 128_000`, `n_batch = 2048`, all transformer layers offloaded to the GPU (`n_gpu_layers = u32::MAX`).
- A `LlamaChatTemplate` (from the GGUF metadata `tokenizer.chat_template`, or from `--gemma-chat-template` if overridden).
- A `LlamaSampler` chain composed of a grammar sampler followed by either a greedy or seeded distribution sampler.

\begingroup\scriptsize

| Knob | Default | Source |
|--------------------------|---------------------|--------------------------|
| `gemma_model_path` | `models/gemma4-e4b/gemma-4-E4B-it-bf16.gguf` | `bin/reality-loop.rs:28` |
| `gemma_model_pin_path` | `config/ccreality_model_pins.json` | `bin/reality-loop.rs:29` |
| `gemma_model_pin_key` | `gemma4_e4b_bf16` | `bin/reality-loop.rs:30` |
| `gemma_min_context_tokens` | 128,000 | `bin/reality-loop.rs:32` |
| `gemma_max_new_tokens` | 8,192 | `bin/reality-loop.rs:33` |
| `gemma_temperature` | 0.0 (deterministic) | `bin/reality-loop.rs:34` |
| `gemma_top_p` | 0.9 | `bin/reality-loop.rs:35` |
| `gemma_batch_size` | 2,048 | `bin/reality-loop.rs:36` |
| `gemma_seed` | 1 | `bin/reality-loop.rs:37` |
| `gemma_gpu_layers` | `u32::MAX` (all) | `bin/reality-loop.rs` |

\endgroup

The model file is loaded with `LlamaModel::load_from_file` after the cryptographic pin check passes. Quantisation is BF16, which is *lossless* relative to the BF16 training weights; the project deliberately rejects the smaller Q4/Q5 quantisations for the production pin so determinism is not contaminated by quantisation rounding. At Temperature 0 the harness's sampling is deterministic given the seed.

## Cryptographic Model Pinning

`config/ccreality_model_pins.json` is the canonical pin manifest. Each pin entry has six fields, every one of which is checked before the harness will load:

\begingroup\scriptsize
```
{
  "models": {
    "gemma4_e4b_bf16": {
      "description": "Gemma 4 E4B Instruction-Tuned, BF16 GGUF (full precision, lossless).",
      "repo_id": "ggml-org/gemma-4-E4B-it-GGUF",
      "filename": "gemma-4-E4B-it-bf16.gguf",
      "expected_sha256": "sha256:23458339ab520c5632ab8251ba42ed8f30baa29e0c74210b3fcbe9a4b047720c",
      "expected_size_bytes": 15053094880,
      "expected_min_context_tokens": 128000,
      "expected_identity_substrings": ["gemma4"]
    }
  }
}
```
\endgroup

`load_model_pin()` parses the manifest, `validate_gemma_model_pin()` compares the on-disk file's SHA-256 and size, and `validate_gemma4_model_identity()` (called after model load) inspects the GGUF's `general.architecture` and `general.name` metadata fields and asserts every required identity substring (case-insensitive) is present. New model entries must include all six fields; missing fields fail closed.

The pin guards against silent model swaps: the harness will refuse to load a quantised version of the same model, a fine-tuned derivative, or an unrelated model with the same filename. This is a defence-in-depth feature: the deny list, the cargo build, the BF16 GGUF file, and the cryptographic pin all have to agree before inference proceeds.

## Reality Branches

Every event the loop emits is namespaced by a *reality branch ID* so the subsystem is auditable across multiple layers --- model, instance, attempt, prompt-candidate.

| Function | Pattern |
|----------------------------|---------------------------------------|
| `root_reality_branch_id` | `<loop_id>:root` |
| `task_reality_branch_id` | `<loop_id>:<model>:<instance>:task-root` |
| `attempt_reality_branch_id` | `<loop_id>:<model>:<instance>:attempt-NNN` |
| `prompt_candidate_branch_id` | `<loop_id>:<model>:<instance>:prompt-candidate-NNN` |

Every event carries `schema_version = 1` and includes `loop_id`, `loop_layer`, `event_type`, `status`, `model`, `instance_id`, `attempt`, `reality_branch_id`, `parent_reality_branch_id`, `promotion_status`, `payload`, and `payload_sha256`. Reading the JSONL event stream is sufficient to reconstruct an attempt's full timeline without needing live access to the runtime.

## Governed Agent Tool Surface

When the model needs to read or write the working repository, it does so through a small, governed tool surface. Read tools are unrestricted in volume; write tools are constrained both by the GBNF grammar (their argument schema cannot be malformed) and by the harness's deny-list (they cannot reach forbidden paths).

\begingroup\scriptsize

**Read tools**

| Tool | Purpose |
|--------------------------|--------------------------------------------------------|
| `list_files` | List files matching an optional pattern under a path. |
| `read_file` | Read bytes from a file with optional line range; returns SHA-256. |
| `search_repo` | grep-style search across the repo, returning paths and SHAs. |
| `inspect_symbol` | Locate a symbol definition and return its source window. |
| `inspect_repository_context` | Read pre-materialised `repository-context.json`. |
| `read_failure_evidence` | Read persisted validation/official evidence files for the current task. |

**Write tools (governed)**

| Tool | Effect |
|--------------------------|--------------------------------------------------------|
| `open_file_window` | Open a writable line window over a file (records pre-edit SHA). |
| `apply_line_window_edit` | Replace the open line window with new text; preferred edit primitive. |
| `apply_structured_edit` | Apply a structured AST-aware edit. |
| `apply_unified_diff` | Apply a unified diff to the repo. |

\endgroup

Every tool call is rejected with a typed `tool_result_blocked` row when: the tool is unknown, a required path is missing, the path is absolute or contains `..`, the resolved path escapes the repository, the listed directory is missing, the file is missing, or a query is empty. Write tools additionally refuse to operate on any path that matches the forbidden-paths predicate (Section 4.4). The default attempt is allowed `--agent-tool-rounds 24` round-trips between the model and the harness; calls past that count are rejected.

## The Deterministic Prompt Compiler

A coding agent's prompt is part of its specification. The loop's prompt compiler enforces a binding *prompt contract* --- three template kinds, each with required source-of-truth inputs.

| `prompt_kind` | Template ID |
|----------------------------|------------------------------|
| `initial` | `swe_repair_initial_v2` |
| `local_failure_refinement` | `swe_repair_local_failure_v2` |
| `official_failure_refinement` | `swe_repair_official_failure_v2` |

Every compiled prompt carries `instance_id`, `repo`, `base_commit`, the problem statement wrapped in explicit markers, a `PREVIOUS_ATTEMPT_JSON` block, a `VERIFIED_EVIDENCE_JSON` block, a `NO_REPEAT_PATCH_SHA256` list (so the model cannot re-emit a previously rejected patch), and a `REPOSITORY_CONTEXT` block of durable on-disk file snippets.

The compiler enforces eight quality gates before the prompt is permitted to leave the function:

\begingroup\scriptsize

| Quality check | Trigger |
|--------------------------------|--------------------------------------|
| `failed_test_included` | every `failed_fail_to_pass` and `failed_pass_to_pass` test ID is mentioned. |
| `official_error_code_included` | `official_error_code` text is in the prompt. |
| `assertion_excerpt_included` | first 80 chars of `test_output_excerpt` is in the prompt. |
| `source_snippet_included` | the literal `Repository context` is present. |
| `previous_patch_hash_included` | every previous patch hash appears. |
| `no_repeat_constraint_included` | the literal `Do not repeat an unchanged patch` is present. |
| `source_of_truth_refs_included` | every short source-of-truth ref appears. |
| `hidden_solution_leak_absent` | gold patch / test patch text is absent (hard gate). |

\endgroup

Every output (prompt text, prompt manifest, request/response artefacts, parse report, prompt-quality metadata) is byte-equal-verified on readback.

## Per-Attempt Official Evaluation

When validation passes locally, `run_official_eval` is invoked inside the attempt loop:

1. Verify Docker SDK preflight via `python -c "import docker; ..."` returning JSON.
2. Write `official-prediction.jsonl` with one row `{instance_id, model_name_or_path, model_patch}`.
3. Run `python -m swebench.harness.run_evaluation --dataset_name princeton-nlp/SWE-bench_Lite --split test --instance_ids <id> --predictions_path <path> --max_workers <n> --timeout <s> --cache_level <env> --run_id <id> --report_dir <dir>` with `DOCKER_HOST` and `DOCKER_CONFIG` env vars.
4. Find the JSON report whose filename contains `<run_id>` and that has a `resolved_ids` array.
5. Build `official-success-evidence.json` or `official-failure-evidence.json` carrying `resolved`, `completed`, `incomplete`, `empty_patch`, `errored`, `previous_patch_sha256`, `source_of_truth`, and the full `official_report` payload.
6. Persist a BlackBoxFix `score` row and (when the reward spec is present) a `reward compile` row for the attempt.

If the report exits 0 but the target `instance_id` is not classified into any of `completed_ids | incomplete_ids | empty_patch_ids | error_ids`, the loop raises `RUST_REALITY_OFFICIAL_REPORT_INSTANCE_MISSING` rather than silently treating the run as a non-resolution. *Failure evidence becomes the next prompt's `official_evidence` input.*

## Reward Spec

The reward spec (`gemmafrontend/reward_spec.verified_outcome.json`) is a six-component weighted scoring function operating on the verified outcome of an attempt:

| Component | Weight |
|-----------------------------|--------|
| `correctness` | $+4.0$ |
| `verification_confidence` | $+2.0$ |
| `risk_penalty` | $-2.0$ |
| `incident_penalty` | $-3.0$ |
| `policy_missing_penalty` | $-1.0$ |
| `training_value` | $+1.0$ |

Bounds are $[-10.0, +10.0]$. The Rust loop forwards the spec file to `target/debug/context-graph blackbox reward compile --input <reward_spec>` after every attempt.

## Continuous Run-Until-Resolved + GPU VRAM Budget

The loop accepts `--run-until-resolved`. When set, the attempt counter is uncapped and the loop only terminates on `solved == true` (an attempt's official evidence has `resolved_ids` containing the instance) *or* a fail-closed error. Continuous mode requires `--official-eval-each-attempt`; without official Docker readback there is no oracle and the loop refuses to enter continuous mode.

GPU VRAM is read from `nvidia-smi --query-gpu=index,name,uuid,memory.total,memory.used,memory.free,driver_version` before and after every model invocation. A `compute_processes_raw` snapshot (`nvidia-smi --query-compute-apps=pid,process_name,used_memory`) is captured alongside, so VRAM blame can be assigned post-hoc when contention is observed.

| Flag | Default |
|------------------------|----------|
| `--gpu-index` | 0 |
| `--gpu-min-free-mb` | 8,192 |
| `--gpu-max-used-mb` | unset |

## Smoke Curriculum

The initial smoke curriculum (`gemmafrontend/swe_smoke_benchmark_suite.json`) is intentionally tiny --- two instances --- and its expansion is gated on manual review:

| Field | Value |
|----------------------------|-----------------------|
| `suite_id` | `contextgraph_swe_lite_smoke_v1` |
| `dataset` | `princeton-nlp/SWE-bench_Lite` |
| `split` | `test` |
| `limits.max_models_per_batch` | 1 |
| `limits.max_workers` | 1 |
| `limits.max_attempts_per_model_task_run` | 1 |
| `limits.official_timeout_seconds` | 900 |
| `limits.expand_only_after_manual_review` | true |

Selected instances: `psf__requests-2317` (8 FAIL_TO_PASS, baseline `official_unresolved`) and `django__django-12908` (2 FAIL_TO_PASS, baseline `not_yet_run`).

# The Role of ME-JEPA (DynamicJEPA) {#sec:me-jepa}

Where the Reality Loop owns *external* truth (Docker), ME-JEPA owns *internal* prediction quality. It is a Joint-Embedding Predictive Architecture --- LeCun's term for a self-supervised predictor that operates on *latent* representations rather than reconstructing pixels or tokens [3] --- implemented in pure Rust on Candle 0.10.2 with CUDA. The codebase calls it *DynamicJEPA* because its production variant predicts the next *latent panel* from a `(panel, action)` pair, learning the system's dynamics rather than its appearance.

## Architecture

\begingroup\scriptsize
```
input panel x ----> online encoder f_theta ----> z_online
                                                    |
                              concat(z_online, action)
                                                    |
                                          predictor g_phi ----> z_hat
                                                                  |
target panel x' --> target encoder f_xi (EMA, detached) ------> z_target
```
\endgroup

| Component | Type | Update rule |
|---|---|---|
| Online encoder $f_\theta$ | MLP (ReLU between layers) | AdamW gradient |
| Target encoder $f_\xi$ | MLP, same shape, **detached** | EMA: $\xi \leftarrow m\,\xi + (1-m)\,\theta$ [7] |
| Predictor $g_\phi$ | MLP, input = latent $\oplus$ action | AdamW gradient |

Why a JEPA, and not an autoregressive token predictor? Three reasons:

1. **Predicting in latent space is robust to surface variation.** Two sequences of pixel/token observations that are functionally equivalent can have wildly different surface forms; predicting in a learned latent compresses away the irrelevant variation [3].
2. **The target encoder is trained without gradient leakage.** The EMA target encoder receives no gradient --- only an exponential moving average of the online encoder's weights. Stop-gradient on $z_{\text{target}}$ is applied via `.detach()`. This prevents the trivial collapse "make every encoding the same so prediction is easy" [7].
3. **The architecture is small.** ME-JEPA's MLP totals well under a million parameters at default config. It is a *world model*, not a foundation model. It runs in real time on the same GPU that holds the embedding subsystem and the Gemma harness.

## Loss

The loss is a weighted sum of three terms [4]:

| Term | Purpose |
|----------------------|------------------------------------------|
| `latent_mse` | Drive prediction toward target latent. |
| `vicreg_variance` | Prevent latent collapse to a constant. |
| `vicreg_covariance` | Decorrelate latent dimensions. |

Concretely: $\mathcal{L} = w_1 \lVert \hat{z} - \mathrm{sg}(z_{\text{target}}) \rVert^2 + w_2\, \mathrm{relu}(\sigma_{\text{tgt}} - \sqrt{\mathrm{var}(z_{\text{online}}) + \epsilon}) + w_3\, \lVert \mathrm{cov}(z_{\text{online}}) \rVert^2_F / d$.

VICReg (variance-invariance-covariance regularisation) is the canonical defence against representation collapse [4]: even if MSE alone could be minimised by learning a constant function, the variance and covariance terms make that solution infeasible. AdamW updates only $\theta_{\text{online}} \cup \phi_{\text{predictor}}$ --- the target encoder is updated solely via EMA.

## Hard Constraints

The `TrainConfig` validator enforces several hard constraints that, together, encode the operational guarantees of the world model:

- `model.predictor.out_dim` MUST equal `model.encoder.out_dim`. The predictor lives in latent space, not panel space; its output is compared against the target encoder's output, not raw pixels.
- `model.ema_momentum` MUST be in $[0, 1)$.
- `optim.kind` MUST be `adamw`.
- `schedule.device` MUST be `cuda`. CPU fallback is **explicitly forbidden**.
- `seed` is required --- every training run is reproducible bit-for-bit on identical hardware.

## Persistence and the 28 Column Families

All ME-JEPA state is persisted in 28 RocksDB column families prefixed `dj_`. None of them overlap with the teleological / content data plane. After every persistence step, the service performs an *immediate readback* and bincode equality check against the in-memory record; mismatch returns `StorageInvariantViolation`.

Every record carries a `DjRecordHeader { id, version, domain_pack_id, domain_pack_version, created_at_unix_ms, run_id, content_hash }`. `refresh_content_hash()` is called before write; readback equality is enforced. This is the same byte-equal-readback discipline as the Reality Loop, applied to a different domain.

Per training run, the artefact root is `<base_root>/<domain_id>/<training_run_id>` and *must not exist* before the run begins (`ensure_clean_run_dir` errors if it does). The directory contains `model.safetensors` (full tensor map), `config.json` (canonical `TrainConfig`), and `evaluation_report.json` (every split's `latent_mse`, `latent_cosine`, `action_contrast_acc`, `shuffled_target_cosine`, plus the EpochMetric history).

`ModelArtifactRecord.files` stores `relative_path`, `sha256`, `size_bytes` for every file. `verify_artifact_files` recomputes hashes and refuses any mismatch. `load_artifact_for_inference` rejects on corrupt safetensors and stores `model_file_sha256` for inclusion in `PredictionRecord.model_artifact_hash_at_inference`. *Every prediction is hash-bound to the exact artefact that produced it.*

## Predict, Plan, Guard, Surprise

The ME-JEPA service exposes four primary verbs. Each persists into RocksDB with audit and readback verification.

\begingroup\scriptsize

| Function | Persists |
|------------------------|---------------------------------------------------|
| `train_dataset` | `CF_DJ_TRAINING_RUNS` (Started -> Completed/Failed), `CF_DJ_MODEL_ARTIFACTS`, `CF_DJ_AUDIT_LOG` + safetensors files |
| `predict_next_panel` | `CF_DJ_PREDICTIONS` + audit |
| `plan_next_action` | `CF_DJ_ACTIONS`, `CF_DJ_PREDICTIONS`, `CF_DJ_GUARD_DECISIONS`, `CF_DJ_PLAN_TRACES`, audit |
| `record_surprise` | `CF_DJ_SURPRISE_EVENTS` + audit (only when cosine < threshold) |

\endgroup

**Planning** enumerates candidate actions per the domain's `planner_policy.candidate_actions`. For each candidate, the model predicts the next latent panel, computes the utility, and runs every guard. The highest-utility allowed action is selected; if every guard rejects every candidate and a constellation fallback is registered, the highest-utility candidate is selected anyway and `no_accepted_candidate = true` is recorded.

**Guard decisions** combine a *bounds check* (does the predicted next state violate the schema's per-field min/max?) and an optional $G_\tau$ check against per-modality calibrated thresholds. The `GuardDecisionRecord` stores both decisions, the cosine to centroid per modality, the $\tau$ values, the failed modalities, and the `constellation_uuid` used.

**Surprise** is the most epistemically interesting verb. After an action is taken and a new panel is observed, `record_surprise` re-encodes the observed panel under the same artefact via the target encoder, computes $\cos(\text{predicted}, \text{observed})$, compares against the threshold from `domain.planner_policy.surprise_threshold`, and persists a `SurpriseEventRecord` only when the cosine falls below threshold. Surprise events are the system's way of saying: *here, my world model was wrong; remember this*.

## Mutual Information Audit (KSG-1)

Joint-embedding architectures are notorious for partial collapse --- slots of the latent vector that carry no information independent of other slots. ME-JEPA ships a pairwise mutual-information audit using the KSG-1 estimator [9]. The summary records `n_redundant_pairs_loose` (MI > 0.7) and `n_redundant_pairs_strict` (MI > 0.9). When too many pairs are redundant, the latent space is collapsing and the architecture's value is suspect.

## How ME-JEPA Complements the Reality Loop

The Reality Loop reduces uncertainty about whether the *outside* world agrees with a claim. ME-JEPA reduces uncertainty about whether the *internal* world model is calibrated. Used together, the two engines support a richer epistemic posture:

- The Reality Loop refuses success without external evidence --- Docker says yes or there is no run.
- ME-JEPA accumulates surprise events whenever observed outcomes diverge from predicted ones --- surprise is *not* failure, but it *is* a signal that the world model needs updating.
- Surprise events are persisted with the same byte-equal-readback discipline as the Reality Loop's evidence files, on the same single audit substrate.
- Neither subsystem can claim success without the other's source-of-truth confirmation in scope. A claim of "the agent learned to repair $X$" is grounded in *both* the Reality Loop's official Docker pass *and* ME-JEPA's reduced surprise rate over the same task family.

This separation matters. Conflating "the test passed" with "the model understands" is a category error. The Reality Loop tests external truth. ME-JEPA tests internal truth. Both must hold for the system to advance its claim.

# Evidence and Invariants {#sec:evidence}

## The Audit Substrate

Every persistence step in the Reality Loop is double-bound to an append-only ledger:

- **BlackBoxFix SQLite ledger** (`LEDGER_SCHEMA_VERSION = 14`) --- accessible via `target/debug/context-graph blackbox` subcommand. Contains run/model/tool/verifier/score/reward records, each with SHA-256 read-after-write verification.
- **Reality event stream** (`reality-events.jsonl`) --- append-only JSONL. Each row carries `loop_id`, `event_type`, `status`, `payload`, and `payload_sha256`. Appends are byte-equal-verified before the next event is written.

ME-JEPA writes to its own `dj_audit_log` column family. Every operation emits a `DjAuditRecord { operation, status, inputs, outputs, touched_cfs, content_hashes, signal_yield }`. Failed runs are also audited so every state transition is in the log.

## Per-Artefact Source of Truth

The artefact tree under `<runtime_root>/<task_id>/<model>/attempts/attempt-N/` exposes its source of truth in plain text:

\begingroup\scriptsize
```
<runtime_root>/<task_id>/<model>/attempts/attempt-N/
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
\endgroup

Source-of-truth file paths in tool output always start with `file:`. Any operator with read access to `<runtime_root>` can reconstruct exactly what happened on which attempt without privileged access to a database or a live process.

## Static Verification Numbers

| Metric | Value |
|------------------------------------|--------------|
| Workspace crates | 11 |
| All `.rs` files in repo | 1,346 |
| Most recent test run | 6,527 passed, 0 failed |
| MCP tools (`llm` feature on) | 145 |
| RocksDB column families | 101 |
| Reality-loop binary LOC | 11,000+ |
| BlackBoxFix ledger schema version | 14 |
| Measurement contract schema | `contextgraph_reality_measurement_contract_v1` |
| DynamicJEPA column families | 28 |
| Production model GGUF | `gemma-4-E4B-it-bf16.gguf` (15,053,094,880 bytes) |
| Production model SHA-256 prefix | `23458339...` |
| Production model min context | 128,000 tokens |
| Production model max new tokens | 8,192 |
| Production model temperature | 0.0 |

These numbers will drift as the codebase evolves; the source files cited remain the authoritative location.

## What an Operator Sees

When an attempt succeeds, the loop prints `RUST_REALITY_LOOP_SUMMARY=<summary-source-of-truth>` to stdout. When an attempt fails, it prints `RUST_REALITY_LOOP_ERROR_SOURCE_OF_TRUTH=<failure-artifact-path>` to stderr. Both line prefixes are stable for shell consumers.

# Constraints, Tradeoffs, Limitations {#sec:limits}

Honest white papers name their boundaries.

## Single-Writer

ContextGraph enforces a single-writer discipline through a Unix `flock(LOCK_EX | LOCK_NB)` on `<storage.path>/mcp.pid`. On non-Unix systems the lock is *not* acquired --- the warning `E_PID_NO_LOCK` is logged. Running the system on Windows without WSL2 risks RocksDB corruption.

## Hardware Dependency

The in-process Gemma harness needs a CUDA GPU; ME-JEPA's `TrainConfig::validate` rejects any configuration where `schedule.device != "cuda"`. The Reality Loop refuses to call any model when free VRAM is below `--gpu-min-free-mb` (default 8 GB). The harness offloads all transformer layers to GPU (`gemma_gpu_layers = u32::MAX`); a BF16 GGUF of Gemma 4 E4B requires roughly 15 GB of VRAM resident plus context-cache memory. Operators on CPU-only or pre-Blackwell hardware cannot run the full stack today.

## Docker Dependency for the Reality Loop

The Reality Loop refuses to enter the model loop without `--official-eval-each-attempt`. That flag in turn requires a working Docker daemon and the official SWE-bench harness installed. The system explicitly does not implement a "CI-only" or "Docker-free" path; that would amount to building an alternative oracle, which would re-introduce the verification gap the project was built to close.

## Single Pinned Local Model

The Reality Loop is bound to a single local model: `gemma-4-E4B-it-bf16.gguf`, validated by the cryptographic pin in `config/ccreality_model_pins.json`. Earlier iterations of the project routed inference through a local Ollama daemon over HTTP and considered driving multiple models in parallel; both choices were retired so optimizer effort stays focused on harness quality for a single, pinned model. A future operator can re-introduce additional models, but doing so requires updating the pin manifest, the harness identity check, the smoke curriculum, and the prompt compiler in lockstep. Remote API models are out of scope: network introduces uncertainty (rate limits, transient errors, response-latency tail risk) that is hostile to the loop's discipline.

## No Backwards Compatibility

ContextGraph is pre-production; synthetic data only. When a record version changes, the database is wiped and rebuilt --- there is no migration path. This is a deliberate optimisation against the failure mode "support both old and new shapes forever, accumulating implicit invariants."

## Rust-Only Reality Loop

The previous Python SWE-bench loop, Python official-eval bridge, and Python prompt compiler were deleted. So was the Ollama HTTP shim. Split ownership --- two implementations of the same loop --- created subtle drift (different prompt behaviours, different success thresholds, different evidence formats, different model load semantics). The current architecture has one canonical implementation.

## ME-JEPA Phase Coverage

The ME-JEPA service currently supports `objective.kind = "latent_delta"` only. The guard system supports `guards = ["bounds_check"]` only. Phase 7 covers training; Phase 8 covers prediction, planning, and surprise. Other JEPA loss families (predictive coding variants, multi-step rollouts) are out of scope for the current public release.

# Privacy, Security, Maintainability {#sec:security}

## No Outbound HTTP

The Reality Loop never calls any HTTP API. The MCP server is the only network/IO surface, and the in-process Gemma harness has no network code at all. Tools that wrap external services (Docker, the official SWE-bench evaluator) are invoked via local sockets. This is enforced both by the `.claude/settings.json` deny list and by the absence of any `reqwest` call in `reality-loop.rs` that targets an off-host URL.

## Deny-Listed Files

The system deny-lists `.env`, `swebench/**`, `tests/oracle_**`, `crates/context-graph-benchmark/datasets/swe_bench_*`, `crates/*/src/reward_specs/**`, and `crates/*/src/verifiers/**`. These deny rules are enforced both at the harness level (`is_forbidden_benchmark_mutation`) and at the `.claude/settings.json` level. Bypassing them is treated as a hard failure (`RUST_REALITY_PATCH_TOUCHES_BENCHMARK`), not a warning.

## Append-Only Audit Logs

The audit log column families `audit_log`, `audit_by_target`, `dj_audit_log`, `merge_history`, and `importance_history` are append-only by design. Audit and merge-history rows are *permanent*; they never expire and are never deleted.

## Hash-Bound Predictions and Hash-Bound Inference

Every ME-JEPA prediction record carries `model_artifact_hash_at_inference` --- the SHA-256 of the artefact file used to produce it. Every Gemma inference is preceded by the cryptographic pin check on the GGUF. If either artefact is later replaced or corrupted, the resulting predictions can still be traced back to the exact model version that produced them.

## Permanent Merge and Importance History

Phase-4 merge history and importance change history are not garbage-collected. When two memories are merged, or a memory's importance is changed, the corresponding record persists forever. ContextGraph treats every such operation as a fact about the past that future analyses may need.

## Operator Attribution

The provenance plan adds `created_by` and `operator_id` to every MCP tool call, every merge, and every importance change. Attribution is opt-in but recommended for any deployment where audit accountability matters.

## Maintainability

The codebase publishes a hard upper bound on file size (1,500 lines per `.rs` file) enforced in CI. The full repository is `cargo build --release --workspace` clean: zero errors, zero warnings, `cargo clippy --workspace --all-targets -- -D warnings` clean.

# Conclusion {#sec:conclusion}

The ContextGraph Reality Loop is a small, opinionated piece of infrastructure that takes the success criterion away from the agent. It refuses to declare victory unless an external Docker harness places the target SWE-bench instance in `resolved_ids`. The model that produces the candidate patch is loaded in-process from a cryptographically-pinned BF16 GGUF; its tool calls are constrained by a GBNF grammar so they cannot be malformed; its inference is byte-deterministic given the seed at temperature 0. Every artefact along the way is byte-equal-readback verified before the next decision is allowed to use it. Every failure is a typed error with a remediation message. Every event is appended to an audit log that survives the run.

ME-JEPA --- the Joint-Embedding Predictive Architecture the codebase calls *DynamicJEPA* --- sits beside the loop. It learns latent dynamics from instrument readings, predicts the next latent panel from $(\text{panel}, \text{action})$ pairs, and persists *surprise events* whenever observed outcomes diverge from predictions. It does not replace the Reality Loop's external oracle; it does not pretend to. Instead, it gives the system a calibrated internal world model, and the surprise stream gives operators a reason to investigate when the model and the world disagree.

The two subsystems are aligned in discipline: byte-equal readback, append-only audits, hash-bound artefacts, fail-closed gates, deny-by-default scopes, no silent fallback, no network for inference. Used together, they support a coding-agent runtime in which capability claims are grounded in measurement, not in the model's natural-language confidence.

What we hope a careful reader takes away is structural rather than feature-by-feature: the verification gap that haunts modern coding agents is closeable, the closing tools are mostly fail-closed boundaries plus disciplined provenance, and the extra cost of running them is small relative to the cost of false-positive capability claims. The code is in `crates/context-graph-cli/src/bin/reality-loop.rs` and `crates/context-graph-dynamicjepa/`. The contracts are in `<run_root>/measurement-contract.json` and `config/ccreality_model_pins.json`. The evidence is in `reality-events.jsonl` and the BlackBoxFix SQLite ledger. The next attempt is one shell command away.

# References {#sec:references}

\setlength{\parskip}{2pt}

[1] *ContextGraph Specifications*, `docs2/contextgraphspecs/` --- twenty-two markdown files covering the 11-crate Rust workspace; authoritative source for every constant, column family, MCP tool, and code path cited in this paper. Available locally in the project repository.

[2] *Reality-Shift Principle*, `docs/ccreality/01b_reality_shift_principle.md`. Establishes "no alteration without current state, no alteration without a delta back into perception" as the runtime invariant.

[3] Y. LeCun, "A Path Towards Autonomous Machine Intelligence" (Version 0.9.2), 27 June 2022. OpenReview. URL: <https://openreview.net/pdf?id=BZ5a1r-kVsf>.

[4] A. Bardes, J. Ponce, and Y. LeCun, "VICReg: Variance-Invariance-Covariance Regularization for Self-Supervised Learning," in *Proc. Int. Conf. on Learning Representations (ICLR)*, 2022. arXiv:2105.04906. URL: <https://arxiv.org/abs/2105.04906>.

[5] C. E. Jimenez, J. Yang, A. Wettig, S. Yao, K. Pei, O. Press, and K. Narasimhan, "SWE-bench: Can Language Models Resolve Real-World GitHub Issues?" in *Proc. Int. Conf. on Learning Representations (ICLR)*, 2024. arXiv:2310.06770. URL: <https://arxiv.org/abs/2310.06770>.

[6] *SWE-bench_Lite Dataset (`princeton-nlp/SWE-bench_Lite`)*, Hugging Face Datasets. URL: <https://huggingface.co/datasets/princeton-nlp/SWE-bench_Lite>.

[7] J.-B. Grill, F. Strub, F. Altché, C. Tallec, P. Richemond, E. Buchatskaya, C. Doersch, B. Avila Pires, Z. Guo, M. Gheshlaghi Azar, B. Piot, K. Kavukcuoglu, R. Munos, and M. Valko, "Bootstrap Your Own Latent: A New Approach to Self-Supervised Learning," in *Advances in Neural Information Processing Systems (NeurIPS)*, 2020. arXiv:2006.07733. URL: <https://arxiv.org/abs/2006.07733>.

[8] *ContextGraph Project Repository*: `crates/context-graph-cli/src/bin/reality-loop.rs`, `crates/context-graph-dynamicjepa/`, `gemmafrontend/server.py`, and the broader 11-crate Rust workspace. Available locally in the project repository.

[9] A. Kraskov, H. Stögbauer, and P. Grassberger, "Estimating Mutual Information," *Physical Review E*, vol. 69, no. 6, p. 066138, 2004. DOI: 10.1103/PhysRevE.69.066138. URL: <https://journals.aps.org/pre/abstract/10.1103/PhysRevE.69.066138>.

[10] G. Gerganov et al., *llama.cpp* (LLM inference in C/C++) and *llama-cpp-2* (Rust bindings used by this project). Sources: <https://github.com/ggerganov/llama.cpp> and <https://github.com/utilityai/llama-cpp-rs>.

\clearpage

\appendix

# Appendix A --- Architecture Diagrams and Component Tables {#sec:appendix-a}

## System-Level Topology

\begingroup\scriptsize
```
+---------------------------------------------------------------------+
|                       Operator surface                              |
|   /ccreality-iterate    /ccreality-smoke                            |
|   /ccreality-trigger-check    /ccreality-readback                   |
|   reality-loop CLI    blackbox CLI                                  |
+----+----------------------------------------------------------------+
     |
     v
+---------------------------------------------------------------------+
|       context-graph-mcp (JSON-RPC 2.0 server, 141/145 tools)         |
|   Single-writer flock at <storage.path>/mcp.pid                      |
|   Daemon mode: stdio-to-TCP relay                                    |
+--+---+---+---+---+---+---+---+---+---+---+---+---+---+---+----------+
   |   |   |   |   |   |   |   |   |   |   |   |   |   |
   v   v   v   v   v   v   v   v   v   v   v   v   v   v
 [Memory] [Causal LLM] [Graph LLM] [Learner UTL] [DynamicJEPA]
                            |
                            v
              +-----------------------------+
              |       RocksDB DB            |
              |  101 column families        |
              |  shared 256MB block cache   |
              +-----------------------------+

+---------------------------------------------------------------------+
|  Reality Loop (separate Rust binary; does not load MCP server)       |
|                                                                      |
|  reality-loop.rs (tokio::main)                                       |
|   - Validates manifest against measurement-contract-v1               |
|   - Cryptographically pins gemma-4-E4B-it-bf16.gguf                  |
|   - Loads model in-process via llama-cpp-2                           |
|   - Drives generation with GBNF-grammar-constrained sampler          |
|   - Routes governed write tools (apply_line_window_edit, ...)        |
|   - Calls official Docker SWE-bench evaluator                        |
|   - Persists to BlackBoxFix SQLite + reality-events.jsonl            |
+---------------------------------------------------------------------+
```
\endgroup

The MCP server and the Reality Loop are *separate processes*. They communicate through the shared filesystem (RocksDB, BlackBoxFix SQLite, runtime root, models directory) and through `target/debug/context-graph blackbox` CLI calls. The Reality Loop must be runnable with the MCP server *down*: failures in the optimizer surface must not block measurement runs.

## Single-Attempt Flow

A single attempt executes the gate sequence enumerated in Appendix B. Each gate produces either a successful artefact (with byte-equal readback verification) or a typed `RealityError` JSON file under `<runtime_root>/failures/`. Stable shell-consumer prefixes (`RUST_REALITY_LOOP_SUMMARY=<path>` on success, `RUST_REALITY_LOOP_ERROR_SOURCE_OF_TRUTH=<path>` on failure) make programmatic handling deterministic.

## ME-JEPA Architecture

ME-JEPA materialises a `LatentPanel` per `(state, action, outcome, time)` event tuple, compiles dataset shards from those panels, and trains an online encoder + EMA target encoder + predictor under the VICReg loss. Phase 7 owns training; Phase 8 owns prediction, planning, and surprise. Every artefact is hash-verified on every inference; every persistence step performs an immediate readback and bincode equality check.

## Audit Substrate

Three independent append-only stores form the substrate: BlackBoxFix SQLite (`LEDGER_SCHEMA_VERSION = 14`), `reality-events.jsonl`, and the RocksDB audit column families (`dj_audit_log`, `audit_log`, `audit_by_target`, `merge_history`, `importance_history`). All three append-only; all three SHA-256 content addressed; failed writes are hard errors; replay alone is sufficient to reconstruct state.

## Component Inventory

\begingroup\scriptsize

| Subsystem | Crate | LOC (.rs files) |
|----------------------|---------------------------|-----|
| MCP server | `context-graph-mcp` | 105 |
| Storage | `context-graph-storage` | 130 |
| Embedding subsystem | `context-graph-embeddings` | 441 |
| Causal LLM agent | `context-graph-causal-agent` | 8 |
| Graph LLM agent | `context-graph-graph-agent` | 9 |
| DynamicJEPA / ME-JEPA | `context-graph-dynamicjepa` | 7 |
| CLI | `context-graph-cli` | 40 |
| Reality Loop binary | `crates/context-graph-cli/src/bin/reality-loop.rs` | --- |
| CUDA kernels | `context-graph-cuda` | 18 |
| Core types | `context-graph-core` | 303 |

\endgroup

## Reality Loop Key Constants

\begingroup\scriptsize

| Constant | Value |
|----------------------------------|-----------------------------|
| `DEFAULT_MODEL` | `gemma4:e4b` |
| `DEFAULT_GEMMA_MODEL_PATH` | `models/gemma4-e4b/gemma-4-E4B-it-bf16.gguf` |
| `DEFAULT_GEMMA_MODEL_PIN_PATH` | `config/ccreality_model_pins.json` |
| `DEFAULT_GEMMA_MODEL_PIN_KEY` | `gemma4_e4b_bf16` |
| `DEFAULT_GEMMA_MIN_CONTEXT_TOKENS`| 128,000 |
| `DEFAULT_GEMMA_MAX_NEW_TOKENS` | 8,192 |
| `DEFAULT_GEMMA_TEMPERATURE` | 0.0 |
| `DEFAULT_GEMMA_TOP_P` | 0.9 |
| `DEFAULT_GEMMA_BATCH_SIZE` | 2,048 |
| `DEFAULT_GEMMA_SEED` | 1 |
| `DEFAULT_AGENT_TOOL_ROUNDS` | 24 |
| `DEFAULT_GPU_MIN_FREE_MB` | 8,192 |
| `DEFAULT_OFFICIAL_DOCKER_HOST` | `unix:///var/run/docker.sock` |
| `DATASET_NAME` | `princeton-nlp/SWE-bench_Lite` |
| `MEASUREMENT_CONTRACT_SCHEMA` | `..._measurement_contract_v1` |
| `LEDGER_SCHEMA_VERSION` | 14 |

\endgroup

## ME-JEPA Persistence Profiles

\begingroup\scriptsize

| Profile | Block | WB | Bloom | Prefix |
|----------------------------|-------|------|-------|--------|
| `dj_uuid_point_cf_options` | 8 KB | 4 MB | yes | 16 B |
| `dj_composite_index_cf_options` | 8 KB | 4 MB | yes | 16 B |
| `dj_secondary_index_cf_options`| 8 KB | 1 MB | no | 16 B |
| `dj_append_only_cf_options` | 8 KB | 4 MB | yes | configurable |
| `dj_large_blob_cf_options` | 64 KB | 8 MB | yes | 16 B |

\endgroup

\clearpage

# Appendix B --- Fail-Closed Gate Inventory {#sec:appendix-b}

There are 38 gates across the Reality Loop and 17 across ME-JEPA. Each gate can be triggered deterministically and each produces a JSON evidence file the operator can inspect.

## Reality Loop --- Configuration / Precondition Gates

| # | Gate | Error code |
|---|---|---|
| 1 | `--max-attempts >= 1` | `RUST_REALITY_MAX_ATTEMPTS_INVALID` |
| 2 | `--run-until-resolved` requires `--official-eval-each-attempt` | `RUST_REALITY_CONTINUOUS_REQUIRES_OFFICIAL_EVAL` |
| 3 | `--official-eval-each-attempt` mandatory | `RUST_REALITY_OFFICIAL_EVAL_REQUIRED` |
| 4 | `--context-graph-bin` exists on disk | `RUST_REALITY_CONTEXT_GRAPH_BIN_MISSING` |
| 5 | `--task-manifest` exists on disk | `RUST_REALITY_TASK_MANIFEST_MISSING` |

## Reality Loop --- Manifest Validation Gates

| # | Gate | Error code |
|---|---|---|
| 6 | `dataset == princeton-nlp/SWE-bench_Lite` | `RUST_REALITY_TASK_DATASET_MISMATCH` |
| 7 | `instance_id` non-empty + contains `__` | `RUST_REALITY_TASK_INSTANCE_ID_INVALID` |
| 8 | `repo` is `owner/name`, no `..` | `RUST_REALITY_TASK_REPO_INVALID` |
| 9 | `base_commit` is 40-char hex SHA | `RUST_REALITY_TASK_BASE_COMMIT_INVALID` |
| 10 | `problem_statement` non-empty | `RUST_REALITY_TASK_PROBLEM_STATEMENT_MISSING` |
| 11 | `fail_to_pass` non-empty | `RUST_REALITY_TASK_FAIL_TO_PASS_MISSING` |
| 12 | `pass_to_pass_count > 0` | `RUST_REALITY_TASK_PASS_TO_PASS_MISSING` |
| 13 | `source_of_truth` binds dataset + instance | `RUST_REALITY_TASK_SOURCE_OF_TRUTH_INCOMPLETE` |

## Reality Loop --- Gemma Harness Gates

| # | Gate | Error code |
|---|---|---|
| 14 | Pin manifest readable + key present | `RUST_REALITY_GEMMA_MODEL_PIN_LOAD_FAILED` |
| 15 | GGUF file size matches `expected_size_bytes` | `RUST_REALITY_GEMMA_MODEL_PIN_SIZE_MISMATCH` |
| 16 | GGUF SHA-256 matches `expected_sha256` | `RUST_REALITY_GEMMA_MODEL_PIN_SHA256_MISMATCH` |
| 17 | GGUF identity substrings present | `RUST_REALITY_GEMMA_MODEL_PIN_IDENTITY_MISSING` |
| 18 | Loaded ctx satisfies `expected_min_context_tokens` | `RUST_REALITY_GEMMA_MIN_CONTEXT_TOKENS_NOT_MET` |
| 19 | Chat template emits a tool grammar | `RUST_REALITY_GEMMA_TOOL_GRAMMAR_MISSING` |
| 20 | `gemma_max_new_tokens > 0` | `RUST_REALITY_GEMMA_MAX_NEW_TOKENS_INVALID` |
| 21 | `gemma_batch_size` valid | `RUST_REALITY_GEMMA_BATCH_SIZE_INVALID` |
| 22 | `gemma_temperature` finite, in `[0, 2]` | `RUST_REALITY_GEMMA_TEMPERATURE_INVALID` |

## Reality Loop --- Execution Gates

| # | Gate | Error code |
|---|---|---|
| 23 | `git rev-parse HEAD == base_commit` | `RUST_REALITY_BASE_COMMIT_READBACK_MISMATCH` |
| 24 | `git status --porcelain` empty after reset | `RUST_REALITY_WORKSPACE_NOT_CLEAN_AFTER_RESET` |
| 25 | Free GPU VRAM $\geq$ `--gpu-min-free-mb` | `RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED` |
| 26 | Used GPU VRAM $\leq$ `--gpu-max-used-mb` | `RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED` |
| 27 | Tool-call submissions parse against grammar | `RUST_REALITY_GEMMA_TOOL_CALL_INVALID` |
| 28 | Patch differs from previous attempts (SHA-256) | `RUST_REALITY_PATCH_REPEATED` |
| 29 | Patch applies via `git apply` | `RUST_REALITY_PATCH_APPLY_FAILED` |
| 30 | Patch does not touch test/fixture/harness | `RUST_REALITY_PATCH_TOUCHES_BENCHMARK` |
| 31 | Docker SDK preflight returns JSON | `RUST_REALITY_OFFICIAL_DOCKER_*` |
| 32 | Official harness exits 0 | `RUST_REALITY_OFFICIAL_EVAL_FAILED` |
| 33 | Final report contains `resolved_ids` | `RUST_REALITY_OFFICIAL_REPORT_MISSING` |
| 34 | Target instance is classified | `RUST_REALITY_OFFICIAL_REPORT_INSTANCE_MISSING` |
| 35 | Agent tool round count $\leq$ `--agent-tool-rounds` | `RUST_REALITY_AGENT_ROUNDS_EXCEEDED` |

## Reality Loop --- Persistence Integrity Gates

| # | Gate | Error code |
|---|---|---|
| 36 | All JSON writes do byte-equal readback | `RUST_REALITY_JSON_READBACK_MISMATCH` |
| 37 | All text writes do byte-equal readback | `RUST_REALITY_TEXT_READBACK_MISMATCH` |
| 38 | Each appended event must round-trip | `RUST_REALITY_EVENT_READBACK_MISMATCH` |

## Reality Loop --- Hidden Solution Leak Gate

The deterministic prompt compiler enforces one hard semantic gate: a prompt is rejected if the SWE-bench gold patch text or test patch text appears anywhere in the prompt (`hidden_solution_leak_absent`). This prevents the system from accidentally feeding ground truth into the model and then claiming success.

## ME-JEPA --- Configuration and Runtime Gates

| # | Gate |
|---|---|
| 1 | `model.encoder.kind == "mlp"` |
| 2 | `model.encoder.hidden` non-empty + no zero |
| 3 | `model.encoder.out_dim > 0` |
| 4 | `model.predictor.out_dim == model.encoder.out_dim` |
| 5 | `model.ema_momentum` $\in [0, 1)$ |
| 6 | `loss.*` weights finite and $\geq 0$ |
| 7 | `optim.kind == "adamw"` |
| 8 | `optim.lr`, `weight_decay` finite and $\geq 0$ |
| 9 | `schedule.epochs > 0` |
| 10 | `schedule.batch_size > 0` |
| 11 | `schedule.device == "cuda"` (CPU forbidden) |
| 12 | `seed` is set |
| 13 | Identical positive/negative pairs rejected |
| 14 | Non-finite values rejected at every input |
| 15 | Wall-clock budget respected |
| 16 | Artefact root must not exist |
| 17 | All artefact files SHA-256 verified on inference |

\clearpage

# Appendix C --- Glossary {#sec:appendix-c}

**Append-only.** A discipline applied to several column families. Records are added via append; deletion is forbidden by convention and unenforced by RocksDB but enforced at the application layer.

**Asymmetric embedder.** A dense embedder that produces *two* vectors per input --- one for the left semantic role (cause, source, paraphrase) and one for the right semantic role (effect, target, context). E5, E8, E10 are asymmetric.

**Attempt.** The atomic unit of work the Reality Loop orchestrates: validate manifest, load model, build prompt, run model, parse and apply tool submissions, run official evaluator, persist artefacts.

**Audit log.** Append-only, permanent record of every operation. Reality Loop writes to BlackBoxFix SQLite + `reality-events.jsonl`; ME-JEPA writes to `dj_audit_log`.

**`base_commit`.** A 40-character hex SHA pinning the exact git commit at which the SWE-bench task is defined.

**BF16.** A 16-bit floating-point format (Brain Floating Point), used by the production GGUF as a lossless precision relative to BF16 training weights.

**BlackBoxFix.** The SQLite ledger system used by the Reality Loop. `LEDGER_SCHEMA_VERSION = 14`.

**Byte-equal readback.** The runtime invariant that every persisted artefact is re-read after write and compared (structurally for JSON, byte-for-byte for text) against the in-memory record.

**Candle.** A Rust-native ML framework (version 0.10.2) used for ME-JEPA training in this project.

**ccreality.** Internal project name for the outer Claude Code optimizer that drives the Reality Loop interactively.

**Cryptographic model pin.** A manifest entry validating six fields of a GGUF before load: `description`, `repo_id`, `filename`, `expected_sha256`, `expected_size_bytes`, `expected_min_context_tokens`, and `expected_identity_substrings`.

**Column family (CF).** A RocksDB construct: an independent, named keyspace inside a single database. ContextGraph uses 101 CFs.

**Constellation.** A compiled, queryable summary of a set of memories. Three distinct stores share the name (content, learner, DynamicJEPA); they are not interchangeable.

**Contract (measurement).** The JSON object the Reality Loop writes to `<run_root>/measurement-contract.json` before any model is loaded.

**DJ.** Two-letter prefix used in the codebase for DynamicJEPA-related identifiers.

**DynamicJEPA.** The codebase's name for ME-JEPA. The "Dynamic" qualifier emphasises that the production variant predicts the next latent panel from $(\text{panel}, \text{action})$.

**EMA target encoder.** The non-trainable counterpart of the online encoder in JEPA architectures, updated only via exponential moving average.

**Embedder.** A model that produces a vector representation of input text. ContextGraph uses 14 production embedders (E1--E14) plus 7 learner-state embedders (E15--E21).

**Fail-closed.** The discipline of treating any unmet precondition as an abort condition.

**Fingerprint.** ContextGraph's term for the per-memory aggregation of all 14 embedder outputs plus metadata.

**flock.** The Unix file-locking primitive used to enforce single-writer discipline.

**$G_\tau$ check.** A guard sub-decision in ME-JEPA that compares the cosine to modality centroids against calibrated $\tau$ thresholds.

**GBNF grammar.** A formal context-free grammar format used by llama.cpp (and `llama-cpp-2`) to constrain a language model's outputs token-by-token. The Reality Loop uses GBNF to make tool-call syntax mechanically un-malformable.

**GGUF.** "GGML Universal Format" --- the on-disk format used by llama.cpp for quantised and full-precision LLM weights. The production model file is `gemma-4-E4B-it-bf16.gguf` (15.05 GB).

**Governed path policy.** The set of paths the ccreality harness is allowed to edit. Defined in `config/ccreality_governed_paths.json`.

**HNSW.** Hierarchical Navigable Small World --- an approximate-nearest-neighbour graph index used for dense vector similarity search.

**Joint-Embedding Predictive Architecture (JEPA).** A self-supervised learning approach in which the model predicts a *latent* representation of one input from another, rather than reconstructing pixels or tokens [3].

**Latent panel.** ME-JEPA's term for a vector representation of a `(state, action, outcome, time)` tuple.

**llama-cpp-2.** The Rust bindings for llama.cpp. The Reality Loop uses `llama-cpp-2` to load the GGUF in-process and run inference; there is no Python or HTTP wrapper involved.

**Loop ID.** A UUID identifying a single Reality Loop run.

**MCP.** Model Context Protocol --- a JSON-RPC 2.0 protocol for AI agents to discover and invoke tools provided by a server.

**ME-JEPA.** The name used in this paper for what the codebase calls *DynamicJEPA*. The "ME" emphasises the architecture's role as the system's *internal* world model in opposition to the Reality Loop's *external* oracle.

**Model artefact.** A persistent, hash-bound model file produced by `train_dataset` in ME-JEPA.

**No silent fallback.** A discipline. When a required input is missing, the system refuses to proceed rather than substituting a default.

**Official evidence.** A JSON file (`official-success-evidence.json` or `official-failure-evidence.json`) carrying the official SWE-bench evaluator's verdict for one attempt.

**Online encoder.** The trainable counterpart of the EMA target encoder in ME-JEPA.

**Oracle.** The process that defines success. The Reality Loop's oracle is the official Docker SWE-bench evaluator [5].

**Pre-production.** ContextGraph's stated maturity level. Pre-production policy is "synthetic data only; wipe DB on schema change; no migration shims."

**Prompt contract.** The structured specification a compiled prompt must satisfy: 8 quality gates, three template kinds, byte-equal readback on every output.

**Provenance.** The chain of source-of-truth references that ties every artefact back to its inputs.

**Reality branch.** A namespace for events in the Reality Loop's audit substrate.

**Reality event.** A row in `reality-events.jsonl`.

**Reality-shift principle.** The runtime invariant [2] that "no alteration without current state, no alteration without a delta back into perception."

**Reward spec.** A weighted scoring function applied to verified outcomes.

**RocksDB.** The embedded key-value store underlying ContextGraph's storage layer.

**Sampler chain.** A `LlamaSampler` composition. The Reality Loop's chain is: grammar sampler $\rightarrow$ greedy or seeded distribution sampler.

**Source of truth.** A path-prefix convention. Source-of-truth file paths in tool output always start with `file:`.

**Surprise event.** A persisted record (in `CF_DJ_SURPRISE_EVENTS`) capturing a moment when the world model's prediction diverged from observed reality.

**SWE-bench Lite.** The benchmark dataset (`princeton-nlp/SWE-bench_Lite`) the Reality Loop targets [5,6].

**Tool grammar.** The GBNF text emitted by the GGUF's chat template (or supplied via `--gemma-chat-template`). Constrains the model's tool-call syntax to a parseable form.

**Typed edge.** A directed edge between two memories carrying an explicit relationship type.

**VICReg.** Variance-Invariance-Covariance Regularisation [4]. A non-contrastive self-supervised loss family.

**Write tool.** A governed agent tool that mutates the working repository (e.g. `apply_line_window_edit`, `apply_structured_edit`, `apply_unified_diff`). All write tools refuse paths that match the deny predicate.
