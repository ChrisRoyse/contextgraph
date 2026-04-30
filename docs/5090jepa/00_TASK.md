# 5090JEPA — Demo Build Task (Fresh-Agent Entry Point)

**Read this file first. Read it completely before opening any other file.** Every other doc in `docs/5090jepa/` is a supporting reference; this one defines the work.

---

## 1. What You Are Doing

You are implementing a runnable, RTX‑5090‑native proof of concept of the **DynamicJEPA** subsystem inside the existing **Context Graph** Rust workspace. The full DynamicJEPA spec lives in `docs/dynamicjepa/implementationplan/00_master_implementation_plan.md` and is the contract you build to. The 5090jepa demo is a strict subset: enough to drive the *entire* loop (domain pack → ingest → adapter → instrument readings → latent panel → binding → trajectory → dataset shard → trained tiny JEPA model → prediction → plan trace → surprise event → verification run) end‑to‑end on the operator's hardware, with full source‑of‑truth evidence at every stage, and zero fallback paths.

You are **not** building a parallel demo project. You add the DynamicJEPA crates/modules into the existing Context Graph workspace exactly as `02_target_module_and_crate_layout.md` prescribes. The demo is the same code an eventual production deployment would use, just with tiny model dimensions and deterministic synthetic data.

The deliverable proves three claims:

1. The DynamicJEPA contract works end‑to‑end on the operator's RTX 5090 (Blackwell, sm_120, CUDA 13.2, WSL2 Linux), with `candle 0.9.2-alpha` already wired into the workspace.
2. The contract is universal: at least **two** unrelated domain packs (`counter_world.v1` arithmetic and `gridworld_5x5.v1` discrete navigation) feed the same pipeline without code changes.
3. Every stage has independent physical evidence — RocksDB column‑family reads, decoded records, file‑hash recomputation, before/after counts — not just success return values.

If any of those three claims is unverified, the demo is incomplete.

---

## 2. Behavioral Rules (Non‑Negotiable)

The user has been explicit. These rules dominate any conflicting habit.

**R1 — No backwards compatibility.** Pre‑production. Synthetic data only. If a schema changes, wipe the database and rebuild. Never write a migration shim, a default‑on‑missing decoder, or a "best‑effort" fallback. If a record, instrument, model artifact, domain pack, or CF is missing or corrupt, return a structured typed error.

**R2 — No fallback. Fail fast. Log loudly.** Every error must include operation name, target id, source‑of‑truth CF (when storage‑related), expected vs actual values, and a concrete remediation hint. `unwrap_or_default()` on a missing vector, returning an empty `Vec` when a CF read fails, treating deserialization failure as "record not found", or substituting a previous artifact when the requested one is missing are all forbidden.

**R3 — No mock data in tests.** Tests must use real RocksDB instances (under `/tmp/`) and deterministic synthetic real input through the actual pipeline. Tests must not pass when the system is broken. If you find yourself writing a test that asserts only the return value while the source‑of‑truth write was supposed to happen, stop and fix the test.

**R4 — Full State Verification (FSV) is mandatory for every persisted behavior.**
- Identify the **Source of Truth** (CF name, on‑disk artifact path, or specific record). State it out loud in your evidence log.
- **Execute then independently inspect.** Never rely on the operation's return value. Read the source of truth back through a separate code path (CLI `inspect-cf`, `get-*` tool, raw RocksDB scan, file hash recomputation).
- **Boundary & Edge Case Audit.** For every persisted behavior, manually exercise three edge cases (empty input, hitting a declared maximum, invalid format). For each: print system state before, run the action, print system state after, prove the outcome.
- **Evidence of Success.** Provide a JSON log showing actual decoded record data residing in the system after execution. "It returned ok" is not evidence.

**R5 — Trigger → Outcome verification.** In computing every triggered process X produces a specific intended outcome Y. Both X (the trigger event) and Y (the produced output) are observable. If you cannot show *both* — what fired and what was actually written/changed — your test is not finished. Find the database row, the file hash, the audit entry, the count delta. Look at it. Confirm the bytes are what they should be. Ambiguous "it probably worked" reports are bugs.

**R6 — Occam's razor.** When two paths achieve the same verifiable outcome, pick the one with fewer moving parts. Counter‑world is the canonical Occam answer for "does the plumbing work" — three integer transitions, one scalar instrument, one trajectory, one two‑row dataset. Gridworld is the canonical Occam answer for "does it actually generalize" — small enough to train in seconds, structured enough that a randomly initialized predictor cannot solve it. Resist the urge to add complexity that does not produce new evidence.

**R7 — Assume the task is wrong.** Before you start any phase, audit this task document against the *current state of the codebase*. File names move. Crate boundaries shift. Constants change. If anything in this doc disagrees with what you observe in `crates/` right now, the codebase wins; update this doc instead of writing code that depends on stale references. Concretely: re‑run the inventory in `02_repository_state_inventory.md` before Phase 0, and again any time you suspect drift.

**R8 — Update memory and lessons after every correction.** If the user corrects you, write the rule to your auto‑memory and append a one‑line lesson to `tasks/lessons.md`. If you discover a non‑obvious approach worked (the user accepts an unusual choice without pushback), save that too.

---

## 3. Hardware & Toolchain You Are Targeting

These are the operator's *actual* numbers, verified at the start of this planning session. Do not assume different. If `nvidia-smi` reports something different when you run it, *that* is the truth and you re‑update the docs.

| Component | Value |
| --- | --- |
| GPU | NVIDIA GeForce RTX 5090 (Blackwell), compute capability **12.0** (sm_120), 32607 MiB VRAM |
| Driver | 596.21 |
| CUDA Toolkit | 13.2 (release 13.2.r13.2 V13.2.78) |
| Host OS | Windows 11 Pro running WSL2 (kernel 6.6.87.2-microsoft-standard-WSL2) |
| CPU | AMD Ryzen 9 9950X3D — 16 cores / 32 threads, AVX‑512 |
| System RAM | 128 GB DDR5 |
| Rust toolchain | stable 1.92.0 (workspace pinned) |
| Candle | `candle-core 0.9.2-alpha`, `candle-nn 0.9.2-alpha`, both with `cuda` feature; comment in `Cargo.toml` confirms "Using 0.9.2-alpha for CUDA 13.x support (RTX 5090 Blackwell)" |
| llama.cpp bindings | `llama-cpp-2 = "0.1"` for GGUF inference (only used by causal/graph LLM agents; not required by 5090jepa) |
| Working tree | `/home/cabdru/contextgraph` |
| Branch | `main` |

Build/runtime environment requirements you must enforce explicitly (see `01_environment_and_hardware.md`):

- `CUDA_COMPUTE_CAP=120` exported at build time so candle compiles cubin for sm_120 and avoids a JIT path. (PTX‑only fallback works but is slower on first kernel launch.)
- `LD_LIBRARY_PATH` includes the WSL CUDA 13.2 lib path.
- `nvidia-smi --query-gpu=compute_cap --format=csv,noheader` returns `12.0`. If it returns anything else, abort with a structured error.

---

## 4. Definition of Done

The demo is done when **all** of the following are true on a freshly cloned tree, on this machine:

1. `cargo build --workspace --release` succeeds with `cuda` features enabled and zero warnings.
2. `cargo test --workspace --all-features` passes with no `#[ignore]` skips on DynamicJEPA test files and **no mock‑only tests** in any DynamicJEPA crate.
3. The new CLI command `context-graph dynamicjepa verify-counter-world` runs against a fresh `/tmp/5090jepa-cw-fsv` RocksDB and produces a `VerificationRunRecord` with status `passed`, plus a JSON evidence log under `tmp/5090jepa_evidence/counter_world/`.
4. The new CLI command `context-graph dynamicjepa verify-gridworld` does the same against `/tmp/5090jepa-gw-fsv` with `gridworld_5x5.v1`.
5. After both verifications, `context-graph dynamicjepa inspect-counts --db <path> --json` shows non‑zero counts in every DynamicJEPA CF that the run is supposed to touch (see `14_full_state_verification_protocol.md` for the exact expected delta table).
6. For each verification, `context-graph dynamicjepa get-artifact --id <ID> --verify-files --json` recomputes the SHA‑256 of `model.safetensors`, `config.json`, and `metrics.json` from disk and confirms they match the `ModelArtifactRecord` row.
7. Three edge cases per domain pack (empty input, invalid action format, panel‑shape ceiling) are exercised, leave the source of truth in the expected state, and have decoded evidence logs.
8. `get_capability_matrix` MCP tool reports `dynamicjepa_enabled: true` and lists every DynamicJEPA tool, every DynamicJEPA CF, and current row counts.
9. The `tasks/lessons.md` file has at least one entry for any user correction received during the build.

If any item is missing, the demo is not done. Do not declare success.

---

## 5. Document Map (Read In This Order)

Each file is intentionally scoped. Read them top to bottom before coding.

| # | File | Purpose |
| --- | --- | --- |
| 00 | `00_TASK.md` (this file) | Master rules, definition of done, document map |
| 01 | `01_environment_and_hardware.md` | Concrete env probes, build commands, sm_120 specifics |
| 02 | `02_repository_state_inventory.md` | What already exists in `crates/` and `docs/` and what is missing |
| 03 | `03_demo_scope_and_constraints.md` | Subset of full DynamicJEPA covered by this demo, and what is explicitly *out of scope* |
| 04 | `04_data_contracts_v1.md` | Trimmed core records, ids, validation, error variants used in v1 |
| 05 | `05_storage_schema_v1.md` | Exact CF list to add, key formats, batch boundaries, migration policy (= wipe DB) |
| 06 | `06_domain_pack_counter_world.md` | Full TOML, fixtures, expected count deltas |
| 07 | `07_domain_pack_gridworld.md` | Second pack: 5×5 deterministic gridworld, JSONL fixture spec |
| 08 | `08_adapter_and_panel_pipeline.md` | `json_event` adapter, scalar/categorical/onehot/time instruments |
| 09 | `09_trajectory_and_dataset.md` | One‑step + 4‑step datasets, splits, negatives, leakage checks |
| 10 | `10_tiny_jepa_model.md` | Architecture, EMA target encoder, VICReg loss, collapse audits |
| 11 | `11_training_runs_and_artifacts.md` | Lifecycle, hashing, on‑disk layout, registry rules |
| 12 | `12_prediction_planning_surprise.md` | Inference, candidate actions, guards, plan trace, surprise events |
| 13 | `13_mcp_and_cli_surface.md` | Exact tool/command names and JSON output contracts for v1 |
| 14 | `14_full_state_verification_protocol.md` | The FSV harness: happy path + edge cases, expected deltas, evidence schema |
| 15 | `15_runbook.md` | End‑to‑end command sequence a fresh agent runs to reproduce the demo |
| 16 | `16_troubleshooting_and_failure_taxonomy.md` | Structured error categories, no‑fallback patterns to reject in review |
| 17 | `17_open_questions_and_recommendations.md` | Decisions to lock + alternatives considered |

The full DynamicJEPA contract lives in `docs/dynamicjepa/implementationplan/`. **The 5090jepa docs override it only where they explicitly say "this demo trims X". Everywhere else the full contract is binding.**

---

## 6. Implementation Order

Follow `docs/dynamicjepa/implementationplan/14_phase_task_breakdown.md` Phases 0–10 verbatim. The 5090jepa specifics:

- **Phase 0** (≈1 day): Lock fixtures and write the two domain pack TOMLs into `configs/dynamicjepa/domain_packs/counter_world.v1.toml` and `configs/dynamicjepa/domain_packs/gridworld_5x5.v1.toml`. Generate happy‑path JSONL fixtures + three edge‑case fixtures per pack. **Phase 0 is done when fixture files parse and the expected count deltas in `14_full_state_verification_protocol.md` match what the design says.**
- **Phase 1** (≈2 days): Add `crates/context-graph-core/src/dynamicjepa/` with all record types from `04_data_contracts_v1.md`. Round‑trip serialization tests using **real bincode**, no shortcuts. `cargo test -p context-graph-core dynamicjepa` must pass.
- **Phase 2** (≈3 days): Add `crates/context-graph-storage/src/dynamicjepa/` per `05_storage_schema_v1.md`. Open all CFs in the existing physical DB. Add `inspect-counts` / `inspect-cf` / `get-*` paths. Fresh `/tmp` DB opens with all DynamicJEPA CFs at count zero. Corrupt version byte fails with a typed codec error.
- **Phase 3** (≈1 day): Wire the CLI module. `register-domain` round‑trips the two TOMLs into RocksDB and the duplicate‑instrument edge case leaves counts unchanged.
- **Phase 4** (≈3 days): `json_event` adapter + scalar/categorical/onehot/time instruments + panel materialization for both domain packs. Three counter_world events and three gridworld events round‑trip through `raw → adapter run → normalized → readings → panel`.
- **Phase 5** (≈2 days): Deterministic binding (id‑equality and explicit‑mapping) + trajectory compiler. One trajectory per pack with the expected ordered transitions.
- **Phase 6** (≈2 days): One‑step dataset compiler + `by_entity` + `by_time` splits + `same_domain_different_outcome` negatives. Counter_world produces 2 rows; gridworld produces N rows where N = `event_count - 1` per trajectory. Insufficient negatives fails explicitly.
- **Phase 7** (≈4 days): Add `crates/context-graph-dynamicjepa/`. Tiny encoder + tiny predictor (per `10_tiny_jepa_model.md`), EMA target, VICReg loss, training loop on candle CUDA, artifact writer. Counter_world tiny config converges in <30 seconds on the 5090; gridworld in <5 minutes. Hash recomputation matches registry rows. Corrupt artifact load fails before any prediction row is written.
- **Phase 8** (≈2 days): Prediction → plan → guard → surprise. Hypothetical actions stored with `action_origin = hypothetical`. Rejected guard creates evidence; threshold breach creates surprise event linked to prediction + outcome.
- **Phase 9** (≈2 days): MCP tool definitions, handlers, DTOs, dispatch entries, capability matrix updates, tool count tests.
- **Phase 10** (≈1 day): `verify-counter-world` and `verify-gridworld` end‑to‑end FSV harness commands that run the full loop, persist a `VerificationRunRecord`, and write JSON evidence to `tmp/5090jepa_evidence/<domain>/`.

Total target: ≈3 weeks single‑agent, less in parallel. Do not skip phases. Do not collapse Phase 4 into Phase 7. The whole point is that storage works before models exist.

---

## 7. Source‑of‑Truth Evidence Rule (Reading This Once Is Not Enough)

For every behavior you implement, before you call it done you must answer these four questions in writing in your evidence log:

1. **What is the trigger event?** (e.g. "operator runs `dynamicjepa ingest-event --file counter_world_happy_path.jsonl`")
2. **What is the named source of truth?** (e.g. "rows in `CF_DJ_RAW_EVENTS`, `CF_DJ_ADAPTER_RUNS`, `CF_DJ_NORMALIZED_STATES`, `CF_DJ_ACTIONS`, `CF_DJ_OUTCOMES`, `CF_DJ_TRANSITIONS`")
3. **What was the system state before? After?** (counts and decoded JSON of new rows)
4. **What independent read path proves the new rows exist and decode to the expected values?** (e.g. "`dynamicjepa inspect-cf --cf CF_DJ_RAW_EVENTS --limit 5 --json` plus `get-raw-event --id <UUID> --json`")

If you cannot answer any of those four questions for a behavior, the behavior is not verified. Treat it as a P0 bug.

This rule is non‑negotiable because the user has explicitly said: *"if there is some way you can validate the outcome for whatever it is you are manually testing then you MUST MANUALLY VERIFY THE OUTPUT BY CHECKING IF THE OUTPUT EXISTS"*.

---

## 8. What Already Exists (Anti‑Hallucination Anchor)

Verified again during the Phase 2 storage audit on 2026-04-30. The live tree wins if anything below drifts:

- `crates/` still contains the original 10 packages. The DynamicJEPA service crate `context-graph-dynamicjepa` does **not** exist yet; it is still a Phase 7 task.
- Phase 1 core contracts are present: `crates/context-graph-core/src/lib.rs` declares `pub mod dynamicjepa;`, and `crates/context-graph-core/src/dynamicjepa/` contains the v1 records, ids, validation, errors, and versioned bincode tests.
- Phase 2 storage data plane is present: `crates/context-graph-storage/src/dynamicjepa/` defines 25 DynamicJEPA CFs, store modules, key helpers, codec helpers, inspection/count APIs, audit records, and a storage FSV test.
- The physical DB open path now opens **98 total CFs**: 11 base + 41 teleological + 14 quantized + 5 code + 2 causal + 25 DynamicJEPA. Fresh `/tmp` DBs open with all 25 DynamicJEPA CFs at zero rows.
- `crates/context-graph-mcp/src/tools/names.rs` exposes the existing MCP tools only; DynamicJEPA MCP tools are still Phase 9.
- `crates/context-graph-cli/src/commands/mod.rs` registers existing subcommands only; the `dynamicjepa` CLI surface is still Phase 3.
- `configs/dynamicjepa/` exists and contains the Phase 0 domain packs, verification fixtures, and generated gridworld corpus.
- `data/contextgraph_data` (or wherever `CONTEXT_GRAPH_STORAGE_PATH` points) is the existing RocksDB physical database. **Do not corrupt it.** All FSV runs use a fresh `/tmp/` path.
- `models/` already contains all 14 production embedder weights; do not redownload. The DynamicJEPA tiny model trains from scratch in candle and writes to `artifacts/dynamicjepa/...`, a new directory.

If any of those statements is wrong when you re‑audit before a later phase, the audit wins; update this section.

---

## 9. Dataset & Synthetic‑Input Strategy

**Counter_world** is fully deterministic, hand‑authored, three transitions. Inputs are integer `(state, action, outcome)` triples whose arithmetic is always `outcome = state + action`. A correctly trained tiny JEPA must achieve >0.99 next‑state cosine on the held‑out transition. A model with shuffled targets must score near 0.0. If a randomly initialized model scores >0.5 on this task, you have a leakage bug.

**Gridworld_5x5** is also fully deterministic, generated by `scripts/5090jepa/generate_gridworld_corpus.py` (you create this; it lives in `scripts/`, not `tests/`, per CLAUDE.md). Inputs are 5×5 grid coordinates `(x, y)` ∈ {0..4}², a one‑hot action `{up, down, left, right, noop}`, and the outcome `(x', y')` after applying the action with wall‑clipping. The generator emits 1000 trajectories of length 8 each, train/val/test split 700/200/100 by trajectory hash. Expected metrics from `10_tiny_jepa_model.md`: latent MSE < 0.05 on val, action‑contrast accuracy > 0.95.

Both domains are synthetic but real — they pass through the *actual* adapter, instrument, panel, trajectory, dataset, and training pipeline. No mocked components anywhere. If anything is mocked you have violated R3.

---

## 10. Where To Start (For The Fresh Agent)

1. Open and read every file `01_…` through `17_…` in this directory in order. Do not skim.
2. Open `docs/dynamicjepa/implementationplan/00_master_implementation_plan.md` and `14_phase_task_breakdown.md`. Re‑read `12_full_state_verification_manual_tests.md` carefully — it is your FSV template.
3. Run the environment probe in `01_environment_and_hardware.md` Section 2 and confirm every assertion.
4. Run the inventory in `02_repository_state_inventory.md` Section 3 and update Section 8 of *this* file with anything that has drifted.
5. Begin Phase 0 only when (1)–(4) are clean. Each phase must end with the FSV evidence log per `14_full_state_verification_protocol.md`.

After every user correction, append a one‑line lesson to `tasks/lessons.md` and update memory.

---

## 11. Out Of Scope For The 5090 Demo

Listed explicitly so you do not waste time on them:

- ME‑JEPA career‑trajectory pack (Phase 12 of the full plan). Out.
- Existing Context Graph data ingestion via `context_graph_memory.v1.toml` / `learner_state.v1.toml` (Phase 11). Out — the demo uses **only** synthetic counter_world + gridworld.
- LLM‑driven discovery (`trigger_causal_discovery`, `discover_graph_relationships`). Out — the demo's binding methods are deterministic id‑equality and explicit‑mapping.
- E12 ColBERT, E13 SPLADE, E11 Kepler, E9 HDC, asymmetric E5/E8/E10 cause/effect heads. None of these participate in the 5090 demo's instrument set; the only instruments used are scalar, categorical, onehot, and time.
- Multi‑node training. Single 5090, single process.
- Production phase config. The demo runs in `phase = "development"`.
- Removing or refactoring any existing crate or CF. Additive only.

If you find yourself touching anything in this list, stop and ask the user.

---

## 12. Acceptance Sign‑Off Checklist (Use This Verbatim)

Before declaring the demo done, paste this checklist into the closing report and tick each box with the proof artifact:

- [ ] `cargo build --workspace --release` clean — paste compile time and warning count
- [ ] `cargo test --workspace --all-features` clean — paste pass count
- [ ] `nvidia-smi` confirms RTX 5090 detected from inside training process — paste output
- [ ] `dynamicjepa inspect-counts --db /tmp/5090jepa-cw-fsv --json` after `verify-counter-world` — paste JSON
- [ ] `dynamicjepa inspect-counts --db /tmp/5090jepa-gw-fsv --json` after `verify-gridworld` — paste JSON
- [ ] Decoded `VerificationRunRecord` for each domain — paste JSON
- [ ] Recomputed artifact SHA‑256 matches registry for each domain — paste both hashes
- [ ] Three edge cases per domain leave the SoT in the expected state — paste before/after diffs
- [ ] `get_capability_matrix` shows dynamicjepa enabled with non‑zero CF counts — paste JSON
- [ ] `tasks/lessons.md` updated for every user correction during the build — paste new entries

When all ten ticks land with proof, the demo is done.
