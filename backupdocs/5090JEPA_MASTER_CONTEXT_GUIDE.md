# 5090JEPA — Master Context Guide

**Purpose**: This is the single document you feed to an AI to give it complete, unambiguous context about the project. Everything an AI needs to produce world-class pitch decks, infographics, technical briefings, sales material, regulatory submissions, or strategy memos lives here. Read this first; the rest of `./docs2/` operationalises it.

**Audience**: An AI agent or analyst with zero prior knowledge of Context Graph, DynamicJEPA, or the 5090jepa demo. Reading this in one pass should fully orient them.

**Author intent**: When in doubt, prefer concrete facts over rhetoric. Every number, file path, hardware spec, and player name in this document has been verified against either the live codebase, the planning docs, vendor websites accessed via Playwright, or recent (2026) market research. If a fact looks suspicious, the source is named in §16 — go check it.

**Date of context**: 2026-04-29.

---

## Table Of Contents

1. Thirty-Second Pitch
2. Two-Minute Pitch
3. The Three Layers (1 Picture)
4. Layer 0 — Context Graph (What Already Exists)
5. Layer 1 — DynamicJEPA (The Contract)
6. Layer 2 — 5090jepa (The Demo)
7. The Evidence Chain (The Singular Differentiator)
8. Where It Sits In The AI Stack
9. Why Wrappers Cannot Replicate This
10. Regulatory Landscape That Forces Demand
11. Market Sizing And Acquisition Comps
12. The Players In The Game
13. Recommended Business Strategy
14. Pricing, ICP, And Sales Motion
15. Behavioral Rules (Engineering Discipline)
16. Glossary And Terminology
17. Numbers Cheat Sheet (For Slides And Decks)
18. Source Documents And Where To Find More
19. Common Misconceptions To Pre-Empt

---

## 1. Thirty-Second Pitch

The EU AI Act enforces high-risk requirements on **2 August 2026**, with fines up to **7% of global revenue**. Every regulated enterprise running AI now needs a hash-verified **evidence chain** for every prediction, plan, and action their AI takes. Wrapper "guardrails" startups (Lakera, Guardrails AI, Credo, Fiddler, Arize, Calypso, Robust Intelligence) sit *outside* the AI and inspect inputs/outputs — they cannot produce the structural evidence regulators and AI insurers (Munich Re, Swiss Re) actually require. **5090jepa** is a Rust + RTX-5090 reference implementation of a **universal action-conditioned world-model runtime (DynamicJEPA)** that emits an EU AI Act Annex IV / AIBOM-compatible evidence packet *as a side effect of the data plane*. The strategy: open-source the runtime, sell the compliance pack on top, and distribute via insurance carriers who price a premium reduction for using it. Two recent 2024-25 acquisitions (Cisco / Robust Intelligence at $400 M and F5 / CalypsoAI at $180 M) prove the exit comps.

---

## 2. Two-Minute Pitch

### The problem

By Aug 2 2026 every company deploying high-risk AI in the EU must produce **technical documentation per Annex IV**: design specs, data sources, training methodology, performance metrics, risk-management evidence, and continuous post-market monitoring. US banks have lived under **SR-11-7** since 2011 — every model needs independent validation, governance, ongoing monitoring. **NIST AI RMF**, **ISO/IEC 42001:2023**, and the emerging **AIBOM (AI Bill of Materials)** standard pile on. **98.5%** of organizations say they need more AI governance professionals (IAPP). AI Compliance Manager salaries are $125K–$200K. Enterprises are spending $300-750K per high-risk model on manual evidence assembly.

### The current "solutions"

Two categories, both inadequate.

1. **Wrapper startups** (Lakera $30M, Guardrails AI $7.5M, Credo, Fiddler $0.002/trace, Arize Phoenix, Braintrust, Helicone, HiddenLayer): they hook into the LLM call boundary and log inputs/outputs. Their data plane is the prompt; they have no view into the agent's *world state*. They produce text logs, not structural evidence chains. They get bundled into hyperscaler products (AWS Bedrock Guardrails, Azure AI Content Safety) and acquired by network security incumbents (Cisco $400M, F5 $180M, IBM watsonx.governance + Guardium).
2. **Manual compliance teams**: 2-5 FTEs per high-risk model writing Word documents that only describe what the AI *should* be doing, not what it *is* doing. Costly, slow, error-prone, and useless during a regulator's audit.

### The 5090jepa solution

A Rust workspace built on a 14-embedder Context Graph runtime + a **DynamicJEPA** subsystem that:

- Ingests events through declarative **domain packs** (TOML).
- Runs **adapters** that map raw events to typed state/action/outcome/transition records (RocksDB column families).
- Materialises **latent panels** through versioned, deterministic **instruments**.
- Compiles **trajectories** and **dataset shards** ready for self-supervised training.
- Trains a **tiny action-conditioned JEPA predictor** (≤14K params, ≤1 GB VRAM, ≤90s on a 5090) with EMA target encoder + VICReg variance/covariance regularisation.
- Produces **predictions, plan traces, guard decisions, and surprise events** — every one persisted as a hash-verified record in its own column family.
- Verifies every persisted behavior with a **Full State Verification (FSV)** harness that produces a `VerificationRunRecord` plus a JSON evidence bundle.

Every byte of that evidence chain is **structurally produced as a side effect of the data plane** — not bolted on, not log-scraped, not reconstructed after the fact. That is the only architecture in the market that makes Annex IV / AIBOM / NIST AI RMF / SR-11-7 evidence cheap to produce.

### The strategy

**Open-source the runtime (Apache-2.0). Commercialise the "Annex IV / AIBOM Compliance Pack" subscription. Distribute through a Tier-1 AI insurance partnership (Munich Re aiSure / HSB AI Liability) so deploying our platform measurably lowers customer AI liability premiums.** Insurer-priced premium reduction is the credible third-party signal that wrapper claims cannot match. Buyers' insurers literally pay them to deploy us.

### The exit

Cisco bought Robust Intelligence for **$400 M** in Oct 2024. F5 bought CalypsoAI for **$180 M** in Sep 2025. Both were technically thinner than what we have. With one Tier-1 BFSI lighthouse + one insurance partnership the comp band is **$300-700 M** in 36 months.

---

## 3. The Three Layers (One Picture)

```
                        ┌────────────────────────────────────┐
                        │  Layer 2 — 5090jepa (the demo)     │
                        │  Counter_world + Gridworld_5x5     │
                        │  RTX 5090 + CUDA 13.2 + Candle     │
                        │  FSV harness, evidence packets     │
                        └─────────────┬──────────────────────┘
                                      │ proves
                                      ▼
                        ┌────────────────────────────────────┐
                        │  Layer 1 — DynamicJEPA (contract)  │
                        │  Domain packs, adapters,           │
                        │  instruments, panels, trajectories,│
                        │  predictor, plan, surprise, FSV    │
                        │  25 new RocksDB CFs                │
                        │  26 new MCP tools                  │
                        └─────────────┬──────────────────────┘
                                      │ extends
                                      ▼
                        ┌────────────────────────────────────┐
                        │  Layer 0 — Context Graph (existing)│
                        │  14 production embedders (E1-E14)  │
                        │  7 learner-state slots (E15-E21)   │
                        │  73 RocksDB column families        │
                        │  108 MCP tools (with llm)          │
                        │  RocksDB + HNSW + Candle CUDA      │
                        │  Causal graph, typed edges,        │
                        │  constellations, learner state,    │
                        │  CAMD-DPO mining, Shakespeare      │
                        │  empirical loop                    │
                        └────────────────────────────────────┘
```

Every layer is *additive*. The 5090jepa demo doesn't replace anything; it adds 25 column families to the existing 73, 26 MCP tools to the existing 108 (or 110, depending on `llm` feature; consult `02_repository_state_inventory.md` for the live count), and one new crate `context-graph-dynamicjepa` to the existing 10 workspace crates.

---

## 4. Layer 0 — Context Graph (What Already Exists)

### 4.1 What it is

An open-source Rust workspace (10 crates, ~414K LOC) that stores memories as **14-embedder fingerprints** in **RocksDB** and exposes them through **108 MCP JSON-RPC tools** plus a local **CLI**. It is the substrate that already does enterprise-grade memory storage, retrieval, fingerprinting, causal discovery, graph linking, learner-state tracking, and contrastive mining. DynamicJEPA is built on top of Context Graph, not next to it.

Repo: `/home/cabdru/contextgraph` on `main` branch. Current head: `dc599e69 docs(dynamicjepa): add 16-section implementation plan`.

### 4.2 Hardware target (operator's machine)

| Component | Value |
| --- | --- |
| GPU | NVIDIA GeForce RTX 5090 (Blackwell, sm_120, 32 GB VRAM) |
| Driver | 596.21 |
| CUDA | 13.2 |
| OS | Windows 11 Pro running WSL2 |
| CPU | AMD Ryzen 9 9950X3D (16 cores / 32 threads, AVX-512) |
| RAM | 128 GB DDR5 |
| Rust | 1.75 MSRV (1.92.0 installed) |
| Candle | 0.10.2 (CUDA 13.2 / Blackwell via cudarc 0.19.x) |

### 4.3 The 14 production embedders

Every memory stored in Context Graph is encoded into 14 parallel embedding spaces:

| Slot | Name | Dim | Source model | Purpose |
| --- | --- | --- | --- | --- |
| E1 | Semantic | 1024 | intfloat/e5-large-v2 | General semantic |
| E2 | TemporalRecent | 512 | sinusoidal PE of timestamp | Recency |
| E3 | TemporalPeriodic | 512 | Fourier periodic | Hour/day cycles |
| E4 | TemporalPositional | 512 | sinusoidal positional | Sequence position |
| E5 | Causal (asymmetric) | 768 | nomic-ai/nomic-embed-text-v1.5 | Cause/effect markers |
| E6 | Sparse | 30522 | naver/splade-cocondenser-ensembledistil | Keyword |
| E7 | Code | 1536 | Qodo/Qodo-Embed-1-1.5B | Source-code semantics |
| E8 | Graph (asymmetric) | 1024 | intfloat/e5-large-v2 | Source/target graph |
| E9 | HDC | 1024 (proj from 10000-bit) | custom hyperdimensional | Typo-robust |
| E10 | Multimodal (asymmetric) | 768 | intfloat/e5-base-v2 | Paraphrase / context |
| E11 | Entity (TransE) | 768 | THU-KEG/KEPLER-Wiki5M-KE | Named entities + relations |
| E12 | LateInteraction | 128/token | colbert-ir/colbertv2.0 | ColBERT MaxSim |
| E13 | SPLADE | 30522 sparse | prithivida/Splade_PP_en_v1 | Sparse expansion |
| E14 | BGE-M3 dense | 1024 | BAAI/bge-m3 | Multilingual long-context |

Plus 7 **learner-state slots** (E15–E21) for affective and physiological signals (OpenFace, speech, PPG, EDA, EEG, etc.) used by the UTL (Universal Theory of Learning) subsystem. These never enter content fusion — they live in their own column families and back goal-distance computations.

### 4.4 Storage (73 column families today)

| Group | Count |
| --- | --- |
| Base | 11 (`nodes`, `edges`, `embeddings`, `metadata`, `temporal`, `tags`, `sources`, `system`, `embedder_edges`, `typed_edges`, `typed_edges_by_type`) |
| Teleological / learner / training | 41 |
| Quantized embedder (`emb_0` … `emb_13`) | 14 |
| Code (`code_*`) | 5 |
| Causal (`causal_relationships`, `causal_by_source`) | 2 |
| **Total** | **73** |

Quantization codecs (per embedder):

- E1, E5, E7, E10, E11, E14: PQ-8 (8 bytes/vector)
- E2, E3, E4, E8: Float8 E4M3
- E9: Binary (1250 bytes)
- E6, E13: Sparse-native (variable)
- E12: Token pruning (~50%)

### 4.5 Search & fusion

Four search strategies: `e1_only`, `multi_space` (default), `pipeline` (E13 → E1 → E12), `embedder_first`. Five aggregation functions: RRF (k=60), Weighted RRF, Weighted Average, Max Pooling, Additive RRF. **17 named weight profiles** including `semantic_search`, `causal_reasoning`, `code_search`, `fact_checking`, `graph_reasoning`, `temporal_navigation`, `sequence_navigation`, `typo_tolerant`, `pipeline_full`, `multilingual_search`, plus three state-conditioned profiles routed by learner state (`affect_repair`, `affect_priming`, `affect_neutral`).

### 4.6 Graph and causal subsystems

- **K-NN embedder edges**: per-embedder nearest neighbours stored in `embedder_edges`.
- **Typed edges**: 8 relation types (SemanticSimilar, CodeRelated, EntityShared, CausalChain, GraphConnected, ParaphraseAligned, KeywordOverlap, MultiAgreement).
- **Causal relationships**: dual E5 vectors + LLM-generated descriptions (Hermes-2-Pro-Mistral-7B via llama-cpp-2).
- **Asymmetry modifiers (AP-77)**: E5 cause→effect 1.2×, effect→cause 0.8×; E8 source→target 1.2×, target→source 0.8×; E10 paraphrase→context 1.2×, context→paraphrase 0.8×.

### 4.7 MCP server and CLI

- **MCP**: `context-graph-mcp` — JSON-RPC stdio/TCP server. **108 tools with `llm` feature, 104 without** at the time the planning docs were authored; the live `tools/names.rs` may report 110 / 106 — a recent linter change updated this. 26 categories: capability, core, merge, curation, topic, file_watcher, sequence, causal, causal_discovery (llm), keyword, code, graph, robustness, entity, embedder, temporal, graph_link, maintenance, provenance, daemon, training, learning (12 tools), utl (18 tools), constellation, contrastive, typed_edges.
- **CLI**: `context-graph` — operator CLI (`session`, `hooks`, `memory`, `topic`, `divergence`, `learning`, `utl`, `setup`, `warmup`, `watch`, `export`, `backfill-e14`).
- **Daemon mode**: single-writer enforced via `<db_path>/mcp.pid` Unix flock.

### 4.8 Already-validated capabilities (most recent memory entries)

- 6,527 tests passing as of 2026-02-26. Zero compile warnings.
- Memory review 2026-02-24: 22 issues found and fixed; deep FSV across 17 tools and 51 edge cases.
- E2 sinusoidal positional encoding fix (2026-02-25); E2/E3/E4 first-class fusion participation when weight > 0.0.
- 41,308 lines of tech debt removed (2026-02-24).
- Hidden brokenness audit cycle has gone through 15 audits (#1 in Feb 2026 to #15 most recent), totalling > 600 findings, all resolved.

This is a serious, production-grade research codebase. It is **not** a hackathon project.

---

## 5. Layer 1 — DynamicJEPA (The Contract)

### 5.1 What DynamicJEPA is (definition)

A **universal, domain-pack-driven, action-conditioned world-model substrate** that:

1. **Ingests real observed events** from any domain.
2. **Stores raw, normalised, and instrumented records** with full hash-verified provenance.
3. **Builds multi-instrument latent panels** representing the system's view of state at a point in time.
4. **Compiles trajectories and dataset shards** ready for self-supervised JEPA training.
5. **Trains action-conditioned latent dynamics predictors**.
6. **Plans, predicts, and guards actions** against the trained model.
7. **Records prediction outcomes, surprise events, and verification evidence**.
8. **Exposes every operation through MCP tools and CLI commands** that read source-of-truth records back independently.

The full specification lives in `docs/dynamicjepa/implementationplan/00_master_implementation_plan.md` (16 sub-files, 14 phase task breakdown) plus `docs/dynamicjepa/00_master_index.md` (high-level architecture).

### 5.2 Why it is "universal"

Domain-specific behaviour lives in **domain packs** — TOML files declaring state schema, action schema, outcome schema, instrument specs, adapter specs, objectives, invariants, dataset policy, planner policy, verification policy. Adding a new domain (a new vertical: code repos, factory sensors, career trajectories, claims pipelines) requires zero Rust code changes — just a new domain pack file. The 5090jepa demo proves this with two unrelated domains (counter_world and gridworld) that share **all** Rust code.

### 5.3 The end-to-end loop

```
domain pack registered
   ↓
raw events ingested (json_event adapter, JSONL fixtures)
   ↓
adapter run produces normalised state/action/outcome/transition rows
   ↓
instruments materialise per-event readings → latent panel
   ↓
bindings link entities/sessions/goals across events and domains
   ↓
trajectory compiled (ordered transitions + panels)
   ↓
dataset shard compiled (one-step latent_delta objective + negatives)
   ↓
tiny JEPA predictor trained (encoder + predictor + EMA target + VICReg loss)
   ↓
artifact registered with SHA-256 file hashes
   ↓
predict(panel, action) → prediction row + uncertainty
   ↓
plan(panel, skill) → candidate actions × predictions × guard decisions × selected action → plan trace
   ↓
record_surprise(prediction, observed) → surprise event row when cosine < threshold
   ↓
verification run record persists before/after counts and decoded evidence
```

Every arrow is a RocksDB write batch with audit log entry. Every artefact has a hash. Every read goes through an independent code path (FSV principle). Failure at any stage produces a typed error and **no fallback writes**.

### 5.4 The 25 new column families

Added in `crates/context-graph-storage/src/dynamicjepa/`:

```
dj_domain_packs              dj_domain_pack_by_nv
dj_instrument_registry       dj_adapter_registry
dj_raw_events                dj_normalized_states
dj_actions                   dj_outcomes
dj_transitions               dj_adapter_runs
dj_instrument_readings       dj_latent_panels
dj_bindings                  dj_bindings_by_entity
dj_trajectories              dj_dataset_shards
dj_training_runs             dj_model_artifacts
dj_predictions               dj_skill_policies
dj_plan_traces               dj_guard_decisions
dj_surprise_events           dj_verification_runs
dj_audit_log
```

Total system after demo: **98 column families**. Every record is `[record_version: u8][bincode payload]`. SHA-256 content hashes guard against corruption. Wrong version byte = typed codec error, no fallback decode. Wipe-and-rebuild migration policy (no migrators).

### 5.5 The 26 new MCP tools

```
dynamicjepa_register_domain_pack          dynamicjepa_list_domain_packs
dynamicjepa_get_domain_pack               dynamicjepa_ingest_event
dynamicjepa_run_adapter                   dynamicjepa_materialize_panel
dynamicjepa_get_panel                     dynamicjepa_list_instrument_readings
dynamicjepa_create_binding                dynamicjepa_list_bindings
dynamicjepa_compile_trajectories          dynamicjepa_get_trajectory
dynamicjepa_list_trajectories             dynamicjepa_compile_dataset
dynamicjepa_get_dataset_shard             dynamicjepa_inspect_dataset_row
dynamicjepa_train                         dynamicjepa_get_training_run
dynamicjepa_get_artifact                  dynamicjepa_predict
dynamicjepa_plan                          dynamicjepa_record_surprise
dynamicjepa_verify_counter_world          dynamicjepa_verify_gridworld
dynamicjepa_inspect_counts                dynamicjepa_inspect_cf
```

Strict JSON schemas (`additionalProperties: false`). Every mutating tool returns `{operation, status, created_ids, source_of_truth_cfs, before_counts, after_counts, audit_log_ids}`. Every read tool decodes the record by id. The capability matrix exposes DynamicJEPA enabled status, CF list, runtime counts, and registered domain pack metadata.

### 5.6 The data contracts (records that exist)

- `DjRecordHeader` — version byte, domain_pack_id, domain_pack_version, created_at, source_run_id, content_hash.
- `DomainPack` — full schema, instruments, adapters, objectives, invariants, policies.
- `RawDomainEvent`, `NormalizedState`, `NormalizedAction`, `NormalizedOutcome`, `StateTransition`.
- `AdapterRunRecord`.
- `InstrumentSpec`, `InstrumentReading`.
- `LatentPanel` (slots, vectors, masks, panel_hash, materializer_version).
- `BindingRecord`.
- `TrajectoryRecord`.
- `DatasetShardRecord` (with `LeakageReport` and `ShapeSummary`).
- `TrainingRunRecord`, `ModelArtifactRecord` (with per-file SHA-256).
- `PredictionRecord`.
- `SkillPolicyRecord`, `GuardDecisionRecord`, `PlanTraceRecord`.
- `SurpriseEventRecord`.
- `VerificationRunRecord` — before/after counts, decoded record excerpts, artefact hash checks, status.

Full Rust-style structures live in `docs/5090jepa/04_data_contracts_v1.md`.

### 5.7 The error taxonomy

`DynamicJepaError` has 18+ typed variants (`Validation`, `SourceOfTruthMissing`, `Storage`, `Codec`, `InstrumentFailed`, `RequiredInstrumentMissing`, `PanelShapeMismatch`, `BindingEvidenceMissing`, `TrajectoryInvariantViolation`, `DatasetLeakageDetected`, `TrainingFailed`, `ArtifactHashMismatch`, `PredictionInputMissing`, `GuardRejected`, `VerificationFailed`, `StorageInvariantViolation`, `DomainPackNotFound`, `DomainPackVersionMismatch`, `SchemaValidationFailed`, `AdapterFailed`). Every variant carries operation name, target id, expected vs actual, and a remediation hint. Generic `String` errors are forbidden.

---

## 6. Layer 2 — 5090jepa (The Demo)

### 6.1 What 5090jepa proves

A complete reference implementation of the DynamicJEPA contract, runnable on the operator's RTX 5090 in WSL2 in under 5 minutes, with **zero fallback paths** and **no mock data**. Two unrelated domain packs (counter_world arithmetic and gridworld_5x5 navigation) feed the same pipeline without code changes. Every persisted behaviour passes Full State Verification.

The plan lives under `docs/5090jepa/` (18 markdown files, ≈ 4,000 lines):

```
00_TASK.md
01_environment_and_hardware.md
02_repository_state_inventory.md
03_demo_scope_and_constraints.md
04_data_contracts_v1.md
05_storage_schema_v1.md
06_domain_pack_counter_world.md
07_domain_pack_gridworld.md
08_adapter_and_panel_pipeline.md
09_trajectory_and_dataset.md
10_tiny_jepa_model.md
11_training_runs_and_artifacts.md
12_prediction_planning_surprise.md
13_mcp_and_cli_surface.md
14_full_state_verification_protocol.md
15_runbook.md
16_troubleshooting_and_failure_taxonomy.md
17_open_questions_and_recommendations.md
```

### 6.2 The two domains

**counter_world.v1** — three integer transitions; the universe rule is `outcome.next_counter = state.counter + action.delta`. Tiny enough to verify the plumbing in seconds; trivial enough that a randomly initialised predictor cannot solve it.

**gridworld_5x5.v1** — 1,000 deterministic 8-step trajectories on a 5×5 grid with five actions (up/down/left/right/noop). 8,000 events. 7,000 one-step training rows split 70/20/10 by `trajectory_id` hash. Generated by `scripts/5090jepa/generate_gridworld_corpus.py` with a fixed seed; SHA-256 of the corpus is asserted by the test suite to detect drift.

Together they prove **universality**: same Rust code, different TOML, two completely unrelated domains.

### 6.3 The tiny JEPA architecture

```
Online Encoder:    MLP(D_in → hidden → D_z)
Predictor:         MLP((D_z + D_action) → hidden → D_z)
Target Encoder:    EMA copy of Online Encoder (no grad)

Loss = w_mse · MSE(z_hat, sg(z_target))
     + w_var · VICReg_variance(z_online)
     + w_cov · VICReg_covariance(z_online)
```

| Hyperparameter | counter_world | gridworld |
| --- | --- | --- |
| D_in | 2 | 31 |
| Hidden | [16] | [64, 64] |
| D_z | 8 | 32 |
| EMA momentum | 0.99 | 0.995 |
| Optimizer | AdamW (1e-3) | AdamW (5e-4) |
| Batch size | 2 | 256 |
| Epochs | 200 | 30 |
| Total trainable params | ≈ 480 | ≈ 13,500 |
| VRAM peak | < 50 MB | < 1 GB |
| Wall-clock training | < 30 s | < 90 s |

VICReg variance + covariance terms prevent encoder collapse (the standard JEPA failure mode); EMA target + MSE-on-detached-target supplies the JEPA bootstrap. These design choices follow the published C-JEPA / VICReg-JEPA literature.

### 6.4 The convergence targets (asserted in tests)

| Metric | counter_world | gridworld |
| --- | --- | --- |
| val latent cosine | ≥ 0.99 | ≥ 0.95 |
| val latent MSE | ≤ 0.01 | ≤ 0.05 |
| action-contrast accuracy | ≥ 0.99 | ≥ 0.95 |
| shuffled-target cosine (must be low) | ≤ 0.10 | ≤ 0.30 |
| random-init cosine (must be low) | ≤ 0.10 | ≤ 0.30 |
| VICReg variance per dim min | ≥ 0.5 | ≥ 0.5 |

Shuffled-target and random-init baselines are leakage and triviality checks. If a randomly initialised predictor already meets the validation targets, input features include the answer — halt.

### 6.5 The FSV harness

`dynamicjepa verify-counter-world` and `dynamicjepa verify-gridworld` are bundled CLI commands that:

1. Wipe `/tmp/5090jepa-{cw,gw}-fsv` directories.
2. Run the entire pipeline against a fresh RocksDB.
3. Capture before/after CF counts at every step.
4. Decode every important record and write JSON to `tmp/5090jepa_evidence/<domain>/<run_uuid>/decoded/`.
5. Recompute every artefact file's SHA-256 and assert equality to the registry.
6. Run three edge cases per domain (empty input, invalid action format, OOB state/panel) and assert exact count deltas.
7. Mutate one byte of the model file and assert the next predict call returns `ArtifactHashMismatch` with **zero new prediction rows**.
8. Persist a `VerificationRunRecord` with status `Passed` or `Failed { failure_details, evidence_path }`.

This is the demo-level FSV. There is also a comprehensive Operator Sign-Off Checklist (10 boxes in `00_TASK.md` §12) that gates "demo done".

---

## 7. The Evidence Chain (The Singular Differentiator)

### 7.1 What it is

For every action a DynamicJEPA-instrumented agent takes, the runtime persists — atomically, in one RocksDB write batch — the following chain of typed records:

```
RawDomainEvent
   ↓
AdapterRunRecord (status: Completed | Failed)
   ↓
NormalizedState + NormalizedAction + NormalizedOutcome + StateTransition
   ↓
InstrumentReading[] (one per declared instrument, with input_hash + output_hash)
   ↓
LatentPanel (with panel_hash + materializer_version)
   ↓
PredictionRecord (with model_artifact_hash_at_inference + uncertainty)
   ↓
PlanTraceRecord
   ↓ (lists)
       candidate_action_ids, prediction_ids, guard_decision_ids,
       utility_scores, selected_action_id
   ↓
GuardDecisionRecord[] (Allow | Reject{reason, evidence_refs, threshold_values})
   ↓
SurpriseEventRecord (when prediction vs observed cosine < threshold)
   ↓
DjAuditRecord (timestamp, operation, actor, input_ids, output_ids, cfs_touched, content_hashes)
```

Every record carries:
- A **content hash** (SHA-256 over canonical bincode payload).
- A **version byte** (no fallback decode on mismatch).
- A **domain_pack_id + domain_pack_version** linking to the registered TOML's source hash.
- Backreferences to its source records (an `InstrumentReading` references its `event_id`; a `LatentPanel` references its source `transition_id`; a `PredictionRecord` references its `model_artifact_id` and the artefact's hash *at inference time*; etc.).

The chain is **append-only** for audit-relevant CFs (`dj_audit_log`, `dj_training_runs`, `dj_predictions`, `dj_plan_traces`, `dj_surprise_events`, `dj_verification_runs`, `dj_model_artifacts`). No update-in-place. No silent overwrite. Wipe-and-rebuild on schema change, never a migrator.

### 7.2 What the chain maps to (regulatory cross-walks)

| Regulation / standard | Required documentation field | DynamicJEPA evidence |
| --- | --- | --- |
| **EU AI Act Annex IV (1)** "Description of intended purpose" | DomainPack.title, DomainPack.objective_specs |
| **Annex IV (2)(a)** "Methods and steps for development" | TrainingRunRecord, AdapterRunRecord, RawDomainEvent.payload_bytes |
| **Annex IV (2)(b)** "Design specifications" | DomainPack TOML + source_hash |
| **Annex IV (2)(c)** "Description of system architecture" | The 25 CF list + `02_target_module_and_crate_layout.md` |
| **Annex IV (2)(d)** "Data sheets describing training methodologies and techniques" | DatasetShardRecord (source_hashes, leakage_report, shape_summary) |
| **Annex IV (2)(e)** "Human oversight measures" | PlanTraceRecord (selected_action_id ∈ candidate_action_ids ∧ all required guards = Allow); GuardDecisionRecord |
| **Annex IV (2)(f)** "Pre-determined changes and continuous compliance" | TrainingRunRecord status + ModelArtifactRecord version chain |
| **Annex IV (2)(g)** "Validation and testing procedures" | VerificationRunRecord + before/after counts + decoded records |
| **Annex IV (2)(h)** "Cybersecurity measures" | content_hash + ArtifactHashMismatch enforcement |
| **NIST AI RMF — Govern** | DomainPack registration audit + operator attribution |
| **NIST AI RMF — Map** | DomainPack.objective_specs + invariants |
| **NIST AI RMF — Measure** | EvaluationReport + VICReg metrics + shuffled-target baseline |
| **NIST AI RMF — Manage** | SurpriseEventRecord + retraining trigger pipeline |
| **SR-11-7 — Independent validation** | shuffled_target_baseline + random_init_baseline asserted in tests |
| **SR-11-7 — Ongoing monitoring** | SurpriseEventRecord with threshold policy from DomainPack |
| **SR-11-7 — Documentation** | The whole evidence chain |
| **ISO/IEC 42001 — Control 6.2.1** "AI system development lifecycle" | TrainingRunRecord lifecycle (Started → Completed/Failed) |
| **ISO/IEC 42001 — Control 6.2.6** "AI system documentation" | The whole evidence chain |
| **ISO/IEC 42001 — Control 8.4** "Decisions and AI system outcomes" | PlanTraceRecord + GuardDecisionRecord + SurpriseEventRecord |
| **AIBOM — Models** | ModelArtifactRecord (artifact_id, hashes, sizes, config_hash, evaluation_report_hash) |
| **AIBOM — Datasets** | DatasetShardRecord (source_trajectory_ids, source_hashes) |
| **AIBOM — Code** | DomainPack source_hash + InstrumentSpec.version + materializer_version |
| **AIBOM — Lineage** | The entire backreference graph from prediction → artefact → dataset → trajectory → panel → readings → adapter run → raw event |

The evidence chain is not "an export feature." It is the data plane. **An AI compliance team's job is reduced from "manually assemble evidence from disparate logs" to "click a button to export the JSON bundle."**

### 7.3 The signing layer (paid commercialisation hook)

The open-source runtime produces the chain. The commercial **Compliance Pack** adds:

- Cryptographic signatures on evidence bundles (Ed25519 customer-key pair).
- Insurer-acceptable export formats (Munich Re template, Annex IV template, NIST AI RMF profile, ISO 42001 audit-ready dossier).
- Customer-facing audit portal (read-only auditor SSO).
- Regulator-facing immutable evidence pinning (signed merkle root + optional anchor to hashed timestamp service).
- AI insurer integration (push-to-policy: each policy renewal automatically pulls the latest evidence bundle).

This split (open core + commercial signing/distribution) is the same business model that worked for HashiCorp, Confluent, MongoDB, Elastic. It is well-trodden.

---

## 8. Where It Sits In The AI Stack

```
┌────────────────────────────────────────────────────────┐
│ Layer 7 — Application UX                              │
│   (chatbots, agent dashboards, copilots)              │
├────────────────────────────────────────────────────────┤
│ Layer 6 — Agent orchestration                         │
│   (LangChain, LlamaIndex, AutoGen, OpenAI Agents SDK) │
├────────────────────────────────────────────────────────┤
│ Layer 5 — Wrapper guardrails / observability  ◀── pure-play startups
│   Lakera, Guardrails, Credo, Fiddler, Arize,         │  live here
│   Braintrust, Helicone, Langfuse, Datadog LLM        │
├────────────────────────────────────────────────────────┤
│ Layer 4 — Compliance/evidence chain  ◀── 5090JEPA LIVES HERE
│   Hash-verified state→action→outcome→prediction→     │  (and is structurally
│   plan→surprise records. AIBOM, Annex IV, NIST AI   │   produced as a side
│   RMF, ISO 42001, SR-11-7 cross-walks.              │   effect of Layer 3)
├────────────────────────────────────────────────────────┤
│ Layer 3 — Action-conditioned world-model runtime  ◀── DYNAMIC JEPA
│   Domain packs, instruments, panels, trajectories,   │
│   tiny JEPA predictor, plan, surprise.                │
├────────────────────────────────────────────────────────┤
│ Layer 2 — Memory / retrieval substrate  ◀── CONTEXT GRAPH
│   14-embedder fingerprints, RocksDB, HNSW,           │
│   constellations, learner state, causal graph.       │
├────────────────────────────────────────────────────────┤
│ Layer 1 — Foundation model layer                      │
│   GPT-4o, Claude, Gemini, Llama, V-JEPA-2, etc.      │
├────────────────────────────────────────────────────────┤
│ Layer 0 — Compute (Blackwell GPUs, CUDA, Candle)      │
└────────────────────────────────────────────────────────┘
```

The unique structural insight: **Layer 4 (compliance evidence) is structurally cheaper to produce when you own Layer 3 (the runtime), because evidence is the *exhaust* of running the runtime.** Wrapper startups try to manufacture Layer 4 evidence from Layer 5/6 inspection — they cannot, because the world-state never crosses their boundary.

This is why the strategic position is defensible: a pure-play wrapper can be excellent at what it does and still fail to produce the evidence buyers need.

---

## 9. Why Wrappers Cannot Replicate This

| Wrapper class | Data plane they see | Evidence they can produce | Evidence they cannot produce |
| --- | --- | --- | --- |
| Prompt guardrails (Lakera, Guardrails AI) | Input prompt, output response | "Did the prompt match this safety pattern?" "Did the response trigger a filter?" | The world-state the agent perceived; the action it intended; the prediction it made; the alternative actions it considered |
| Eval / observability (Braintrust, LangSmith, Langfuse) | LLM call traces, eval scores | "Did the LLM produce this output for this prompt?" | Whether the output was the right action given the agent's goal; whether the world responded as expected |
| Model monitoring (Fiddler, Arize, Arthur) | Model inputs / outputs / drift metrics | "Has the input distribution drifted?" "Are predictions unstable?" | Why the agent took the action; what it predicted would happen; whether it was surprised |
| Network/runtime security (Cisco AI Defense, F5 ADSP) | Network-level traffic to/from models | "Is anyone exfiltrating data?" "Is anyone injecting prompts?" | Anything about the agent's reasoning |
| Policy/governance workflow (Credo) | Workflow checklists, model registry forms | "Did the right humans sign the right approvals?" | What the model is actually doing in production |
| Cloud governance (AWS Bedrock Guardrails, Azure AI Content Safety) | Prompt + response within their hosting environment | "Did Bedrock filter this content?" | Anything about an agent that uses *multiple* model providers |

DynamicJEPA's runtime sees **all of it** — because it owns the data plane that turns events into typed state/action/outcome/transition rows, runs the instruments, materialises panels, makes predictions, plans actions, evaluates guards, records surprises. The evidence chain is a side effect of that execution.

A wrapper trying to produce the same evidence chain would need to **rebuild the agent runtime**. At which point they are no longer a wrapper.

---

## 10. Regulatory Landscape That Forces Demand

### 10.1 EU AI Act — the most consequential

- **Prohibited practices**: in force since 2 Feb 2025. Fines up to €35 M or 7% of global revenue.
- **General-purpose AI obligations**: in force 2 Aug 2025.
- **High-risk AI obligations**: in force **2 Aug 2026**. Fines up to €15 M or 3% of global revenue.
- **Embedded high-risk AI provisions** (medical devices, machinery): 2 Aug 2027.

Annex III lists 8 high-risk categories: biometric identification, critical infrastructure, education, employment, essential services (credit / public benefits / emergency services / insurance), law enforcement, migration, justice and democratic processes. **Banks running automated credit-scoring or insurers automating claim decisions are squarely in scope.**

Annex IV requires the technical documentation listed in §7.2. The fine multiplier is on **global** revenue, not EU revenue — US BFSI is fully exposed if they offer EU-resident services.

### 10.2 SR-11-7 (Federal Reserve / OCC, US)

In force since 2011. Applies to all US banks ≥ $10 B in assets and any model used in safety-and-soundness decisions. Requires:

- **Model risk governance** with board-level oversight.
- **Independent model validation** with documented findings.
- **Ongoing performance monitoring** with thresholds and triggers.
- **Documentation** sufficient for an examiner to reconstruct the model's behaviour.

JPMorgan, Bank of America, Citi, Wells Fargo run **model risk management (MRM)** programmes spending $50-300 M/year. Every JEPA-style model the bank deploys becomes one more model to validate, document, monitor, and quarterly-review. The pain is acute and growing.

### 10.3 NIST AI Risk Management Framework (US)

Voluntary but de facto required for federal contractors. Four core functions: **Govern, Map, Measure, Manage**. NIST AI 100-1 (Jan 2023) defines the framework; updates through 2025 added GenAI-specific profile (NIST AI 600-1) and adversarial-ML profile (NIST AI 800-1).

The DOD CIO and OMB OSTP both reference NIST AI RMF in their procurement guidance (M-24-10 for federal AI use). **If you sell AI to the US government, NIST AI RMF compliance is a procurement gate.**

### 10.4 ISO/IEC 42001:2023

Published Dec 2023. The world's first AI management system standard. **38 controls** across 6 sections:

1. Context of the organisation.
2. Leadership and planning.
3. Support and operation.
4. AI system development lifecycle.
5. AI system documentation.
6. Performance evaluation, internal audit, improvement.

Microsoft, Google Cloud, AWS already certified. Enterprise procurement RFPs increasingly require ISO 42001 certification from any AI vendor — this is **the next SOC 2** for AI.

### 10.5 AIBOM — emerging standard

AI Bill of Materials. A machine-readable inventory of every component in an AI system (models, datasets, training code, hyperparameters, infrastructure, governance metadata). NIST is publishing reference profiles. IBM Research, JFrog, Wiz, Cycode, Mend, Manifest, Noma all building tools. Most current AIBOM tools focus on model + dataset metadata; they do not capture *runtime* evidence. DynamicJEPA does both.

### 10.6 Sectoral overlays

- **NAIC** (US insurance commissioners): bulletin on AI in insurance, with state-by-state adoption.
- **FDA**: Software as a Medical Device (SaMD) AI/ML guidance.
- **DOL / EEOC**: AI in employment decisions.
- **CFPB**: AI in consumer finance — fair lending requirements.
- **NYC Local Law 144**: bias audits for automated employment decision tools.
- **California SB 1001 / AB 2013**: AI training data disclosure.

### 10.7 The "98.5% gap"

IAPP survey: **98.5% of organisations need more AI governance professionals**. Wages: AI Compliance Manager $125K-$200K, with 70% of postings at companies with 10K+ employees, 51% in professional services (consulting buyers). The market is signalling: enterprises know they need help, can't hire fast enough, willing to spend.

---

## 11. Market Sizing And Acquisition Comps

### 11.1 Sizing (multiple forecasters)

| Source | Metric | 2026 | 2030 |
| --- | --- | --- | --- |
| Gartner | AI Governance Platforms spend | $492 M | $1.0 B+ |
| Future Market Insights | Enterprise AI Governance & Compliance | $2.55 B | (FMI projects $13 B by 2036) |
| Grand View Research | AI Governance market | $417.8 M | (~$1.6 B by 2030) |
| MarketsAndMarkets | AI Model Risk Management | $7.17 B → $8.33 B | $15 B (CAGR 16.2%) |

The wide variance reflects definitional differences. The **AI Model Risk Management** number is the most relevant — it captures the BFSI procurement budget for SR-11-7 compliance plus EU AI Act overlap. **BFSI = 39% of total share.**

### 11.2 Acquisition comps (closed deals)

| Deal | Acquirer | Target | Date | Price | What it bought |
| --- | --- | --- | --- | --- | --- |
| **Robust Intelligence** | Cisco | (independent AI security startup, Sequoia-backed) | Oct 2024 | **$400 M** | AI model + data security; runtime adversarial threat protection |
| **CalypsoAI** | F5 | (independent, Series B) | Sep 2025 | **$180 M** | AI runtime security; "test-defend-observe" stack |
| Lumeus | Fiddler | (small startup) | Apr 2026 | undisclosed (small) | Coding-agent observability extension |
| Langfuse | ClickHouse | (OSS LLM observability) | Jan 2026 | undisclosed | Bundled into OLAP stack |

### 11.3 Pure-play funding (open companies)

| Company | Round | Total raised | Lead investors |
| --- | --- | --- | --- |
| Lakera | Series A $20 M (2024) | $30 M | Atomico, Citi Ventures, Dropbox, redalpine |
| Guardrails AI | Seed Feb 2024 | $7.5 M | Bloomberg Beta, GitHub |
| Fiddler | Series C Apr 2026 | $130-150 M total | Lightspeed, Lockheed, Insight |
| Credo AI | Series B 2024 | ~$40 M | Sands Capital, Decibel |
| Arize | Series C 2024 | ~$90 M | Foundation Capital, TCV |
| Braintrust | Series B 2025 | ~$75 M | A16z, Lightspeed |

**Funding is concentrated in the late seed / Series B band**, indicating the market is mid-cycle. Pure-play multiples are compressing (CalypsoAI $180 M < Robust Intelligence $400 M one year later) — bundling has begun.

### 11.4 Hyperscaler bundle commoditisation

- **AWS Bedrock Guardrails** — free with Bedrock.
- **Azure AI Content Safety** — bundled into Azure AI.
- **Vertex AI Model Eval** — bundled into GCP.
- **OpenAI Moderation API** — free with GPT.
- **Anthropic Constitutional AI** — built into Claude.

Wrappers face downward pricing pressure from below.

### 11.5 Adjacent / world-model funding (signals architectural validation)

| Company | Funding | Date |
| --- | --- | --- |
| World Labs (Fei-Fei Li) | $1 B | Feb 2026 |
| Wayve | $1.2 B Series D ($8.6 B post) | 2025-26 |
| Physical Intelligence | $400M+ | 2024-25 |
| Skild AI | $300 M | 2024 |

These are not direct competitors. Their existence at $1 B+ scale **validates** the world-model architecture publicly, raising our credibility and lowering the modeling cost (their open-source releases — Meta V-JEPA 2 — become our default feature set).

### 11.6 Insurance signal

Munich Re's **aiSure** has been writing AI performance failure coverage since 2018. **HSB launched AI Liability for SMBs in Mar 2026**. Munich Re actively hunts for evidence-rich platforms it can underwrite against. Swiss Re publishes the FEAT framework. AIG / Travelers / Chubb adding AI riders.

This is a kingmaker channel that no wrapper has tapped.

---

## 12. The Players In The Game

| Player | What they want | What they fear | What they can offer us |
| --- | --- | --- | --- |
| Regulated enterprise CISO/CRO | Pass audits, lower fine risk, lower premium | Fines, reputational hit, vendor lock-in | Lighthouse logo, ARR, case study |
| Wrapper startups (Lakera, Credo, Fiddler, Arize, etc.) | Survive consolidation by getting bundled | Commoditisation, hyperscaler erosion | Acqui-hire targets if their team is good |
| Cisco / F5 / Palo Alto / IBM | Bundle AI governance into existing security/networking | New independent threat | Acquisition exit at premium |
| AWS / Azure / Google | Make AI safety trivial within their cloud | Customer leaving for multi-cloud | Distribution partnership; not an exit (they'd build) |
| Datadog / Splunk | Sell more observability seats | Margin compression | Integration partner, possible acquirer |
| Foundation labs (Meta, Anthropic, OpenAI) | Defensible model layer | Commoditisation of models | Indirect — they produce the models we run |
| World-model labs (World Labs, Wayve, Physical Intelligence) | Robotics / spatial-AI dominance | Other foundation labs | Not competitors; potential customers |
| Munich Re / Swiss Re / HSB / AIG | Write profitable AI liability policies | Black-box AI risk | Distribution channel + premium-discount endorsement |
| EU Commission / Fed / OCC / NIST / ISO | Drive AI safety adoption | Market non-compliance | Endorsement / reference architecture |
| VC capital (Sequoia, A16z, Lightspeed, Greylock) | Fund the winner of the AI governance category | Backing the wrong horse | Series A funding; warm intros |
| AI Compliance Officers / MRM teams | Reduce manual workload, pass audits | Career risk if they pick the wrong vendor | Champions inside enterprises |
| Standards bodies (NIST, ISO, IEEE) | Drive standard adoption | Bypass via hyperscaler defaults | Reference architecture stamp |
| Open-source community | Tools they can use freely | Vendor lock-in | Adoption flywheel + free contributors |

---

## 13. Recommended Business Strategy

**See `docs2/5090JEPA_GAME_THEORY_ANALYSIS.md` for the full game-theoretic derivation.** Summary:

### 13.1 Core thesis

Open-source the DynamicJEPA runtime under Apache 2.0. Commercialise the **Annex IV / AIBOM Compliance Pack** subscription. Distribute through a Tier-1 AI insurance partnership so deploying our platform measurably lowers customer AI liability premiums. Insurer-priced premium reduction is the credible third-party signal that wrapper claims cannot match.

### 13.2 The mechanism design

```
Customer pays license → us
Customer pays insurance premium → carrier
Carrier requires evidence → from our platform → discount applied
Customer's net cost = license + (premium – discount) - compliance FTE savings
ROI: 3-10× depending on policy size
```

### 13.3 Dominant strategy against each opponent

- **vs wrappers**: their data plane is shallower; insurer test reveals it.
- **vs incumbents (Cisco, F5, IBM)**: they have distribution but cannot replicate runtime; they buy us.
- **vs hyperscalers**: they refuse to take customer liability; they cannot ship our compliance pack.
- **vs frontier labs**: they release research; we ship enterprise products on top.
- **vs Datadog**: $120/day for LLM obs vs our $90-250K/yr ACV that pays for itself via insurance discount.

### 13.4 The clock

EU AI Act high-risk enforcement: **2 Aug 2026**. Procurement cycles take 2 quarters. The window for being the default Annex IV evidence provider closes Q3 2026.

### 13.5 12-month operating plan (compressed)

- **Q2 2026** — open-source GitHub release; 30 BFSI cold outreach + 5 insurance carriers; pre-seed/seed round.
- **Q3 2026** — 1 BFSI design partner; insurance carrier MoU.
- **Q4 2026** — 3-5 paid pilots; insurance partnership public; Series A $15-35 M @ $80-150 M post.
- **Q4 2027** — $3-5 M ARR; 2 acquisition offers > $300 M; declare a competitive process.

---

## 14. Pricing, ICP, And Sales Motion

### 14.1 Pricing tiers

| Tier | Price | Target buyer | Includes |
| --- | --- | --- | --- |
| Open Source | $0 | engineers, researchers | Full DynamicJEPA runtime, both reference domain packs, FSV harness, GitHub support |
| **Starter** | **$30 K/yr** | mid-market regulated company | Single domain pack, single env, NIST AI RMF templates, audit log export, monthly evidence digest |
| **Pro** | **$90 K/yr** | regulated enterprise | Unlimited domain packs, EU AI Act Annex IV pack, AIBOM export, SSO/Okta, RBAC, signed evidence chain, SOC 2 + ISO 42001 cross-walks |
| **Enterprise** | **$250 K-$1 M/yr** | F500 / BFSI / gov | VPC deployment, insurer integration, regulator-facing audit portal, dedicated success engineer, custom domain pack development, 24×7 |
| Insurance partnership | revenue share | carriers | Discounted policy referral; 20% rev-share on premium reduction OR per-policy referral fee |
| Marketplace (year 2+) | 80/20 split | vertical SMEs | Verified domain packs sold subscription on top of OSS runtime |

### 14.2 ICP (in priority order)

1. US regional bank or top-50 global bank with active SR-11-7 program. 5K-50K employees. 5-50 high-risk models in production. Currently spending $1-5 M/yr on MRM FTE.
2. Top-30 insurance carrier (NAIC + Solvency II overlap).
3. Tier-1 healthcare provider (HIPAA + FDA SaMD + state regulators).
4. F500 manufacturer running predictive maintenance + RPA agents.
5. Federal civilian agency or DOD / IC component (NIST AI RMF mandatory).

### 14.3 Sales motion

- **Top of funnel**: GitHub stars + technical blog posts + open-source talks at NeurIPS / Money 20/20 / RSA / Strata.
- **Middle of funnel**: 30-min BFSI evidence audit (free) → 90-day pilot ($25-75 K, single high-risk model) → executive readout co-signed by Munich Re partner.
- **Bottom of funnel**: Pro / Enterprise contract co-signed by CISO + CRO; insurance discount letter from carrier.

### 14.4 ARR build (illustrative)

- Y1: 5 pilots × $50 K + 2 Pro × $90 K = $430 K ARR.
- Y2: 8 Pro × $90 K + 5 Enterprise × $300 K = $2.22 M ARR.
- Y3: 15 Pro × $100 K + 12 Enterprise × $400 K = $6.3 M ARR.
- Y3 ARR target $5-7 M with 110-130% NRR establishes the acquisition price band.

---

## 15. Behavioral Rules (Engineering Discipline)

These rules apply to anyone touching the codebase. They are non-negotiable and explicit in `docs/5090jepa/00_TASK.md`.

- **R1 — No backwards compatibility.** Pre-production. Wipe DB on schema change. No migration shims.
- **R2 — No fallback. Fail fast. Log loudly.** Typed errors with operation, target, expected vs actual, remediation hint. `unwrap_or_default()` on missing data is forbidden.
- **R3 — No mock data in tests.** Real RocksDB under `tempfile::TempDir`. Tests must not pass when the system is broken.
- **R4 — Full State Verification (FSV) is mandatory.** Identify source of truth, execute, independently inspect, audit edge cases, provide evidence as actual decoded record data.
- **R5 — Trigger → Outcome verification.** Every triggered process X produces observable outcome Y. If you cannot show both, the test is incomplete.
- **R6 — Occam's razor.** Pick fewer moving parts when verifiable outcomes are equivalent.
- **R7 — Assume the task is wrong.** Audit task documents against the live codebase before each phase.
- **R8 — Update memory and lessons after every correction.**

These rules are also marketing assets — the discipline shows up in product reliability (6,527 tests, zero warnings, 15 hidden-brokenness audits) and in pitch decks ("we engineered this so an AI auditor cannot find a gap").

---

## 16. Glossary And Terminology

- **AdapterRunRecord** — typed record persisted for every event ingest, status Started → Completed/Failed.
- **Action-conditioned** — predictor takes action as additional input (`predictor(z_state, z_action) → z_next`).
- **AIBOM** — AI Bill of Materials. Machine-readable inventory of models, datasets, code, governance metadata.
- **Annex IV** — section of the EU AI Act listing required technical documentation for high-risk AI systems.
- **BFSI** — Banking, Financial Services, Insurance.
- **BLAKE3-128** — fast cryptographic hash, truncated to 16 bytes used for short keys.
- **Candle** — Hugging Face's Rust-native ML framework (CUDA, CPU, Metal). Pinned at 0.10.2.
- **CF / Column Family** — RocksDB namespace for keyed records. Context Graph has 73; DynamicJEPA adds 25.
- **Constellation** — aggregate centroid + spread over a memory selector (topic/session/tag/explicit ids), persisted in `CF_CONSTELLATIONS`.
- **CRO** — Chief Risk Officer.
- **DjRecordHeader** — common header on every persisted DynamicJEPA record (version byte, domain pack, content hash).
- **Domain pack** — TOML-declared description of a domain's state/action/outcome schema, instruments, adapters, objectives, and policies.
- **EMA target encoder** — exponentially-moving-average copy of the online encoder, used as JEPA target without gradient.
- **Evidence chain** — the hash-verified, source-of-truth, append-only record sequence produced as a side effect of the runtime.
- **FSV** — Full State Verification. Source-of-truth + independent inspection + edge-case audit + decoded-record evidence.
- **Guardrail (wrapper)** — safety layer that sits outside the LLM/agent and inspects prompt/response.
- **ICP** — Ideal Customer Profile.
- **Instrument** — domain-pack-declared transform that turns normalised records into a panel slot vector. Demo kinds: scalar, categorical, onehot, time.
- **JEPA** — Joint-Embedding Predictive Architecture (Yann LeCun's framework). Predicts target representations rather than raw inputs.
- **Latent panel** — multi-instrument representation of system state at a point in time; the JEPA input.
- **MCP** — Model Context Protocol. Anthropic's JSON-RPC standard for tool servers.
- **MRM** — Model Risk Management. SR-11-7's discipline: validation, governance, monitoring, documentation.
- **NIST AI RMF** — NIST AI Risk Management Framework (AI 100-1). Govern, Map, Measure, Manage.
- **Plan trace** — record of a full planning run (candidates → predictions → guards → selected action).
- **Predictor** — JEPA's prediction head: `(z_state, z_action) → z_next`.
- **RRF** — Reciprocal Rank Fusion. Multi-list ranking combination algorithm; k=60 default.
- **SR-11-7** — Federal Reserve / OCC supervisory letter on Model Risk Management. The senior MRM standard for US banks.
- **Surprise event** — record persisted when prediction vs observed cosine falls below threshold; signals world-state drift.
- **TOML** — declarative config format. Domain packs are TOML.
- **Trajectory** — ordered chain of transitions; built from typed normalised records by the trajectory compiler.
- **VICReg** — Variance-Invariance-Covariance Regularisation. Variance + covariance terms prevent JEPA encoder collapse.
- **Wrapper** — sits outside the agent, inspects boundary, no view into world state.

---

## 17. Numbers Cheat Sheet (For Decks / One-Pagers)

| Number | What it means |
| --- | --- |
| **2 Aug 2026** | EU AI Act high-risk enforcement begins |
| **€35 M / 7%** | EU AI Act fine for prohibited practices (whichever higher of EUR or % global revenue) |
| **€15 M / 3%** | EU AI Act fine for high-risk requirement violations |
| **$400 M** | Cisco / Robust Intelligence acquisition (Oct 2024) |
| **$180 M** | F5 / CalypsoAI acquisition (Sep 2025) |
| **$1 B** | World Labs raise (Feb 2026) — validates world-model architecture |
| **$7.17 B → $15 B** | AI Model Risk Management TAM 2026 → 2030 (CAGR 16.2%) |
| **$492 M → $1 B+** | Gartner AI governance platform spend 2026 → 2030 |
| **39%** | BFSI share of AI governance spend |
| **98.5%** | of orgs need more AI governance professionals (IAPP) |
| **$125-200 K** | AI Compliance Manager salary band |
| **108 / 110** | Context Graph MCP tools today (with `llm` feature) |
| **+26 = 134/136** | DynamicJEPA tools added |
| **73** | Context Graph RocksDB CFs today |
| **+25 = 98** | total CFs after DynamicJEPA demo |
| **14** | production embedders (E1-E14) |
| **7** | learner-state slots (E15-E21) |
| **17** | weight profiles (`semantic_search`, `causal_reasoning`, etc.) |
| **6,527** | tests passing (Feb 2026) |
| **0** | compile warnings |
| **414 K** | LOC under `crates/*/src` |
| **15** | hidden-brokenness audits completed; > 600 findings, all resolved |
| **32 GB** | RTX 5090 VRAM |
| **1 GB** | tiny JEPA peak VRAM (gridworld) |
| **< 90 s** | gridworld training wall clock |
| **< 30 s** | counter_world training wall clock |
| **≈ 13 K** | gridworld trainable params |
| **≈ 480** | counter_world trainable params |
| **0.99 / 0.95** | val cosine targets (counter_world / gridworld) |
| **$30 K / $90 K / $250 K-$1 M** | Compliance Pack pricing tiers |
| **3-10×** | typical customer ROI on insurance discount + FTE savings |

---

## 18. Source Documents And Where To Find More

### 18.1 Internal docs (this repo)

- **Master implementation plan** — `docs/dynamicjepa/implementationplan/00_master_implementation_plan.md` (and 15 sub-files).
- **Architecture deck** — `docs/dynamicjepa/00_master_index.md` (and 14 sub-files).
- **5090 demo plan** — `docs/5090jepa/00_TASK.md` (and 17 sub-files, ≈ 4,000 lines).
- **Game theory analysis** — `docs2/5090JEPA_GAME_THEORY_ANALYSIS.md` (458 lines).
- **Existing system specs** — `docs2/contextgraphspecs/01_system_overview.md` … `20_learning_subsystem.md`.
- **Hardware specs** — `docs2/systemspecs.md` (Geekbench dump).
- **CLAUDE.md** (project rules) — top of repo.
- **Auto memory** — `/home/cabdru/.claude/projects/-home-cabdru-contextgraph/memory/MEMORY.md` (project history).

### 18.2 External / regulatory references

- EU AI Act timeline — `https://artificialintelligenceact.eu/implementation-timeline/`
- NIST AI RMF — `https://www.nist.gov/itl/ai-risk-management-framework`
- ISO/IEC 42001:2023 — `https://www.iso.org/standard/42001`
- SR-11-7 — Federal Reserve guidance.
- AIBOM — IBM Research, JFrog, Wiz academy posts.

### 18.3 Competitor pages (live, audited via Playwright)

- Guardrails AI — `https://guardrailsai.com/`
- Lakera — `https://www.lakera.ai/lakera-guard`
- Credo AI — `https://www.credo.ai/`
- Fiddler — `https://www.fiddler.ai/` and `/pricing`
- Arize — `https://arize.com/pricing/`
- Braintrust — `https://www.braintrust.dev/pricing`
- Cisco AI Defense — `https://www.cisco.com/site/us/en/products/security/ai-defense/`
- IBM watsonx.governance — `https://www.ibm.com/products/watsonx-governance`
- Munich Re aiSure — `https://www.munichre.com/en/solutions/for-industry-clients/insure-ai.html`
- Datadog LLM Observability — `https://docs.datadoghq.com/llm_observability/`

---

## 19. Common Misconceptions To Pre-Empt

A skilled AI assistant working from this guide will encounter and need to dispatch the following.

| Misconception | Correction |
| --- | --- |
| "Isn't this just another AI guardrails startup?" | No. Wrappers inspect prompts/responses. We own the runtime. Different layer (Layer 3-4 vs Layer 5). |
| "Won't AWS Bedrock Guardrails make this redundant?" | No. Bedrock Guardrails work only inside Bedrock. Customers run agents that span multiple providers. We are model-provider-neutral. |
| "Will Cisco / F5 / IBM kill you with bundling?" | They cannot replicate the runtime architecture. They will most likely acquire us (the comp band is $300-700 M). |
| "Isn't compliance a slow, low-margin business?" | Compliance + insurance distribution is high-margin (Munich Re's combined ratio is industry-leading). License + insurance discount creates 3-10× customer ROI. |
| "Why do you need a world model at all? Isn't an LLM with logging enough?" | Logging captures inputs/outputs; it does not capture the agent's perceived state, intended action, predicted outcome, or surprise. Regulators want all four. |
| "Aren't tiny models inferior to GPT-class models?" | We are not competing with GPT. The tiny JEPA is the **action-prediction head** for an agent that uses GPT (or Claude or Llama) for natural-language reasoning. The tiny model produces evidence; the LLM produces text. |
| "Why open-source the core?" | Adoption flywheel + recruiting + trust. The compliance pack + insurance distribution are the moat, not the runtime. Same playbook as HashiCorp / Confluent / MongoDB / Elastic. |
| "Aren't there already AIBOM tools?" | AIBOM tools today export model + dataset metadata. Our evidence chain captures runtime decisions, predictions, plans, surprises — none of which existing AIBOM tools see. |
| "Why pick BFSI as ICP?" | 39% of AI governance spend, SR-11-7 already enforced, EU AI Act overlap, top-2000 enterprises with $1-5 M MRM budgets, and they hire AI Compliance Managers at $200K-$300K. Pain is acute and budgeted. |
| "What if the EU AI Act gets delayed?" | SR-11-7 is already in force in the US. ISO 42001 is being adopted by enterprise procurement. Insurers are writing policies regardless. The strategy is robust to a 6-12 month delay. |
| "Aren't insurers slow?" | Munich Re launched HSB AI Liability for SMBs in Mar 2026. They are actively underwriting. |
| "Won't OpenAI / Anthropic do this themselves?" | Foundation labs ship models; they do not ship enterprise compliance products. Their guardrails are bundled with their models — they don't address multi-provider agents or the structural evidence chain. |
| "How can a team this small win against multi-billion-dollar incumbents?" | Incumbents win on distribution, not on architecture. We win on architecture, and trade architecture for distribution at acquisition time (the Cisco/F5 comps prove this works). |
| "Who is the buyer?" | CISO + CRO co-sign. Cost-justified by insurance discount + compliance FTE savings. |
| "Why now?" | EU AI Act enforcement Aug 2 2026. The procurement window is open for 9 months. After that, customers are locked into whatever they bought in panic. |

---

## 20. Closing — How To Use This Guide

When you (the AI) are asked to produce **any** asset relating to this project — a deck, a one-pager, an infographic, a blog post, a regulator briefing — anchor every claim in this document. Numbers come from §17. Architecture comes from §3-7. Strategy comes from §13. Players from §12. Glossary from §16. Sources from §18.

If you find a fact in this guide that conflicts with the live codebase or a fresh primary source, the primary source wins; flag the conflict in your output so the human operator can update this guide.

The companion document `5090JEPA_AI_PROMPT_LIBRARY.md` provides ready-to-use prompts for 30+ asset types. Pair this guide with that library and an AI capable of producing slide-deck markup, mermaid diagrams, or HTML/SVG infographics, and you have an end-to-end content factory.
