# Appendix C — Glossary

Companion to [`whitepaper.md`](whitepaper.md). Project-specific terms appear in alphabetical order; a few cross-discipline terms are included where the project's usage diverges from common practice.

---

**`additionalProperties: false`.** A JSON Schema enforcement rule applied to every MCP tool schema in ContextGraph. When a request carries a property that is not in the schema, the server silently drops it. Together with explicit handler validation, this provides defense in depth against schema/handler drift. *Lesson:* always audit schema↔DTO alignment when adding tool fields.

**Adapter run.** A persisted record (in `CF_DJ_ADAPTER_RUNS`) describing one execution of a registered adapter on one raw event. The adapter normalises the raw event into a `NormalizedState`/`NormalizedAction`/`NormalizedOutcome` triple plus instrument readings.

**Append-only.** A discipline applied to several column families (`audit_log`, `audit_by_target`, `dj_audit_log`, `merge_history`, `importance_history`). Records are added via `append`, never `update`; deletion is forbidden by convention and unenforced by RocksDB but enforced at the application layer. Supports replay-based state reconstruction.

**Artefact.** Either (1) a file on disk produced by an attempt (e.g. `attempt-summary.json`, `model.safetensors`), or (2) a row in the BlackBoxFix SQLite ledger keyed to a run. Both senses are used in this paper; context disambiguates.

**Asymmetric embedder.** A dense embedder that produces *two* vectors per input — one for the "left" semantic role (cause, source, paraphrase) and one for the "right" semantic role (effect, target, context). E5 (causal), E8 (graph), E10 (multimodal/contextual) are asymmetric. *Lesson:* both vectors must be quantised independently; cloning one to the other destroys the asymmetric signal.

**Attempt.** The atomic unit of work the Reality Loop orchestrates. One attempt: validate manifest, build prompt, call model, parse patch, apply patch, run official evaluator, persist artefacts. The attempt counter increments per attempt; in `--run-until-resolved` mode it is uncapped until the success predicate or a fail-closed error.

**Audit log.** Append-only, permanent record of every operation. Reality Loop writes to BlackBoxFix SQLite + reality-events.jsonl; ME-JEPA writes to `dj_audit_log` (RocksDB column family); the teleological store writes to `audit_log` and `audit_by_target` (RocksDB column families).

**`base_commit`.** A 40-character hex SHA pinning the exact git commit at which the SWE-bench task is defined. The Reality Loop refuses to run if `git rev-parse HEAD != base_commit` after reset (Gate 14).

**BlackBoxFix.** The SQLite ledger system used by the Reality Loop and the gemmafrontend evidence console. `LEDGER_SCHEMA_VERSION = 14`. Stores `runs`, `models`, `tools`, `verifiers`, `scores`, `rewards`. Accessed via `target/debug/context-graph blackbox` subcommand.

**`bincode + skip_serializing_if = BROKEN`.** A foundational invariant. Bincode serialisation is positional — fields are written in declaration order and decoded by position. Adding `#[serde(skip_serializing_if = ...)]` produces a record whose layout depends on whether the field was present, making the binary unreadable. *Rule:* use JSON for any record that has optional fields; reserve bincode for vectors and dense records.

**Byte-equal readback.** The runtime invariant that every persisted artefact is re-read after write and compared (structurally for JSON, byte-for-byte for text) against the in-memory record. Implemented as `write_*_checked` helpers throughout the Reality Loop and as `put_*_with_audit_batch` paths in ME-JEPA. Catches filesystem failures and serialisation bugs before they propagate.

**Candle.** A Rust-native ML framework (version 0.10.2 in this codebase) used for all GPU inference and training. Replaces PyTorch in the Rust crates; produces `.safetensors` model files. Cross-platform but in this project mandatorily CUDA.

**Causal direction.** A property of E5 vectors. `CausalDirection::Cause` indicates the embedder is encoding text-as-cause; `CausalDirection::Effect` indicates text-as-effect. Each E5 instruction prefix (`"search_query: Identify the cause in: "` vs `"search_query: Identify the effect of: "`) is hard-coded.

**ccreality.** Internal project name for the outer Claude Code optimizer that drives the Reality Loop interactively. Implemented as MCP tools under prefix `mcp__cgreality__` and as Claude Code skills `/ccreality-iterate`, `/ccreality-smoke`, `/ccreality-trigger-check`, `/ccreality-readback`. Not the subject of this white paper but referenced where its tooling intersects with the Reality Loop's evidence streams.

**Claim scope.** The set of statements the system commits to defending. The Reality Loop's claim scope is precisely one: *the official Docker SWE-bench report's `resolved_ids` array contains the target instance*. Three explicit non-claims accompany this: no full-repo proof, no human-intent proof, no success without official Docker.

**Column family (CF).** A RocksDB construct: an independent, named keyspace inside a single database. ContextGraph uses 101 CFs in the production database. Each CF has its own options (block size, compression, bloom filter, prefix extractor, write buffer size).

**Constellation.** A compiled, queryable summary of a set of memories selected by topic, session, tag, time range, or explicit ID list. Three distinct stores share the name: content (`CF_CONSTELLATIONS` + secondary index), learner (`CF_LEARNER_CONSTELLATIONS`), and DynamicJEPA (`CF_DJ_CONSTELLATIONS`). They are *not* interchangeable.

**Contract (measurement).** The JSON object the Reality Loop writes to `<run_root>/measurement-contract.json` before any model is called. Pins the entire claim scope, the oracle, the immutable boundaries, the model-editable scope, the model-forbidden scope, and the fail-closed policy.

**Cross-correlation centroid.** A 91-dimensional vector (= C(14, 2)) derived from per-memory topic-profile alignments. Stored in `Constellation.cross_correlation_centroid`; provides a compact "shape" of the constellation across embedder-pair correlations.

**DJ.** The two-letter prefix used throughout the codebase for DynamicJEPA-related identifiers (record types, column families, audit operations). The prefix prevents collision with the teleological / content data plane.

**Domain pack.** A strict TOML registration of a DynamicJEPA / ME-JEPA domain — its state schema, action schema, outcome schema, instrument specs, adapter specs, objective specs, dataset policy, planner policy, constellation, and verification policy. Persisted in `CF_DJ_DOMAIN_PACKS`. The four ship-with packs are `counter_world`, `gridworld_5x5`, `career_taxonomy`, and `counter_to_grid` (a cross-domain transfer pack).

**DynamicJEPA.** The codebase's name for ME-JEPA. The "Dynamic" qualifier emphasises that the production variant predicts the *next* latent panel from `(panel, action)` — the system's dynamics, not its appearance. Implemented in `crates/context-graph-dynamicjepa/`.

**EMA target encoder.** The non-trainable counterpart of the online encoder in JEPA architectures. Updated only via exponential moving average: `ξ ← m·ξ + (1-m)·θ`. Receives no gradient; provides stable prediction targets without the trivial collapse "encode everything as constant."

**Embedder.** A model that produces a vector representation of input text. ContextGraph uses 14 production embedders (E1-E14) plus 7 learner-state embedders (E15-E21). E1-E14 cover content; E15-E21 cover learner state and never enter content fusion.

**Epistemic posture.** A way of relating to truth claims. The Reality Loop's posture is "no claim without external verification"; ME-JEPA's posture is "every prediction is hash-bound to the artefact that produced it"; together, they enable a posture in which capability is a function of measurement, not of model self-confidence.

**Fail-closed.** The discipline of treating any unmet precondition as an abort condition. The opposite of "best-effort" or "fall back gracefully". Every gate enumerated in [`appendix_b_invariants.md`](appendix_b_invariants.md) is fail-closed by design.

**Fingerprint.** ContextGraph's term for the per-memory aggregation of all 14 embedder outputs plus metadata. `TeleologicalFingerprint` wraps `SemanticFingerprint` (the 14-embedder vectors) with metadata, importance, and access tracking.

**flock.** The Unix file-locking primitive used to enforce single-writer discipline. ContextGraph holds an exclusive non-blocking flock on `<storage.path>/mcp.pid`. On non-Unix systems the lock is not acquired; multiple writers WILL corrupt RocksDB (logged warning `E_PID_NO_LOCK`).

**G_τ check.** A guard sub-decision in ME-JEPA. Projects the panel/action into each modality, computes cosine to the modality centroid, compares against the calibrated threshold τ, and rejects if any required modality fails. Stored in `GuardDecisionRecord` alongside the bounds-check decision.

**Governed path policy.** The set of paths the ccreality harness is allowed to edit. Defined in `config/ccreality_governed_paths.json`. The `allow` list enumerates permitted paths; the `deny` list enumerates paths that may never be edited. The hook system enforces this on every Edit/Write call.

**HNSW.** Hierarchical Navigable Small World — an approximate-nearest-neighbour graph index used for dense vector similarity search. ContextGraph maintains 15 HNSW indexes (one per dense embedder + Matryoshka-128 + asymmetric duals), all backed by `usearch 2`. Indexes are persisted in `CF_HNSW_GRAPHS` and rebuilt on startup if missing.

**Joint-Embedding Predictive Architecture (JEPA).** A self-supervised learning approach in which the model predicts a *latent* representation of one input from another, rather than reconstructing pixels or tokens. Distinguishing feature: the target encoder is detached (no gradient) and updated via EMA. Coined by Yann LeCun (2022).

**Latent panel.** ME-JEPA's term for a vector representation of a `(state, action, outcome, time)` tuple. Stored in `CF_DJ_LATENT_PANELS` (a large-blob CF). Each panel references multiple `instrument_reading_ids` and `pairwise_reading_ids`.

**LEARNER_*.** A naming convention prefix used for learner-state records and column families. `LEARNER_RECORD_VERSION = 1` covers all 11 learner CFs.

**Loop ID.** A UUID identifying a single Reality Loop run. Used as the root of all reality branch IDs (e.g. `<loop_id>:<model>:<instance>:attempt-NNN`). Allows multiple loops on the same database to be disambiguated in event logs.

**MCP.** Model Context Protocol — a JSON-RPC 2.0 protocol for AI agents to discover and invoke tools provided by a server. ContextGraph's MCP server (`context-graph-mcp`) exposes 141-145 tools across 27 categories. Transports: stdio (default) and TCP.

**ME-JEPA.** The name used in this paper for what the codebase calls DynamicJEPA. The "ME" reading is not formally defined in source; we use the term to emphasise the architecture's role as the system's *internal* world model in opposition to the Reality Loop's *external* oracle.

**Measurement contract.** See *Contract*.

**Model artefact.** A persistent, hash-bound model file produced by `train_dataset`. Persisted as `model.safetensors` in `<base_root>/<domain_id>/<training_run_id>/`, registered in `CF_DJ_MODEL_ARTIFACTS` with per-file SHA-256. Every prediction record carries the artefact's `model_file_sha256` so predictions can be traced to the exact bytes that produced them.

**`no silent fallback`.** A discipline. When a required input is missing, the system refuses to proceed rather than substituting a default. Pre-production policy is "wipe and rebuild on schema change," not "migrate." When a model file is missing, the load fails fast.

**Official evidence.** A JSON file (`official-success-evidence.json` or `official-failure-evidence.json`) carrying the official SWE-bench evaluator's verdict for one attempt. Contains `resolved`, `completed`, `incomplete`, `empty_patch`, `errored`, `previous_patch_sha256`, `source_of_truth`, and the full `official_report` payload. Failure evidence becomes the next prompt's `official_evidence` input.

**Online encoder.** The trainable counterpart of the EMA target encoder. Receives gradient updates from AdamW. Its output `z_online` is one of the inputs to the predictor.

**Oracle.** The process that defines success. The Reality Loop's oracle is the official Docker SWE-bench evaluator. The system explicitly disowns "success" claims that lack the oracle's confirmation.

**Pre-production.** ContextGraph's stated maturity level. Pre-production policy is "synthetic data only; wipe DB on schema change; no migration shims." This affects what kinds of operator behaviours are permitted (e.g. database wipes are routine).

**Prompt contract.** The structured specification a compiled prompt must satisfy: 8 quality gates, three template kinds, byte-equal readback on every output. Implemented in `crates/context-graph-cli/src/bin/reality-loop.rs` (the previous Python compiler was deleted to prevent split ownership).

**Provenance.** The chain of source-of-truth references that ties every artefact back to its inputs. Every persisted record in ContextGraph carries a `content_hash` (or `payload_sha256`) and references its parent records by ID. Provenance is queryable through `mcp__cgreality__reality_*` tools and the `blackbox` CLI.

**Read-after-write.** See *Byte-equal readback*.

**Reality branch.** A namespace for events in the Reality Loop's audit substrate. Five layers: root, task-root, attempt-NNN, prompt-candidate-NNN, and (optionally) operator turn. Each event references its parent branch ID, allowing reconstruction of the run's tree from a flat JSONL stream.

**Reality event.** A row in `reality-events.jsonl`. Fields: `loop_id`, `loop_layer`, `event_type`, `status`, `model`, `instance_id`, `attempt`, `reality_branch_id`, `parent_reality_branch_id`, `promotion_status`, `payload`, `payload_sha256`. Each row is byte-equal-verified on append.

**Reality-shift principle.** The runtime invariant that "no alteration without current state, no alteration without a delta back into perception." Before any edit, perceive (open the file, get the SHA); after any edit, observe the delta (re-read, compute new SHA, append shift-log entry). Defined in `docs/ccreality/01b_reality_shift_principle.md`.

**Reward spec.** A weighted scoring function applied to verified outcomes. ContextGraph's default is `gemmafrontend/reward_spec.verified_outcome.json` (six components, bounded [-10, +10]). The Rust loop forwards the spec file to `target/debug/context-graph blackbox reward compile --input <reward_spec>` after every attempt.

**RocksDB.** The embedded key-value store underlying ContextGraph's storage layer. Single database holds 101 column families. Pre-production policy: no migrations; schema changes wipe the database.

**Source of truth.** A path-prefix convention. Source-of-truth file paths in tool output always start with `file:`. The convention exists so a third-party reader can grep an arbitrary report and locate every cited artefact deterministically.

**Soft delete.** ContextGraph's deletion model for memories. A `soft_deleted::{uuid_string}` key in `CF_SYSTEM` marks the row as deleted; the underlying fingerprint stays in `CF_FINGERPRINTS` until garbage collection. Default GC retention: 7 days. *Lesson:* soft-delete state must be persisted to RocksDB; the in-memory HashMap is cleared on restart.

**Surprise event.** A persisted record (in `CF_DJ_SURPRISE_EVENTS`) capturing a moment when the world model's prediction diverged from observed reality. Created only when `cosine(predicted, observed) < threshold`. The threshold comes from `domain.planner_policy.surprise_threshold`. Surprise events are the system's primary signal that the world model needs updating.

**SWE-bench Lite.** The benchmark dataset (`princeton-nlp/SWE-bench_Lite`) the Reality Loop targets. A subset of SWE-bench (Jimenez et al., ICLR 2024) restricted to instances solvable in a single Docker container. The Reality Loop is hard-bound to this dataset; loops on other datasets must be different binaries.

**Teleological.** ContextGraph's term for the union of (a) cross-embedder synergy/fusion and (b) learner-state UTL. Both layers share the prefix and the storage path; they are documented separately because they are different concerns sharing infrastructure.

**Topic profile.** A 14-dimensional vector encoding a memory's alignment with the active topic clusters. Stored in `CF_TOPIC_PROFILES` (56 bytes per memory). `Constellation.has_topic_profile_signal()` returns false until the clustering pipeline has populated this CF — important when consumers condition on topic-centroid trustworthiness.

**Typed edge.** A directed edge between two memories carrying an explicit relationship type. ContextGraph defines 8 edge types: `SemanticSimilar`, `CodeRelated`, `EntityShared`, `CausalChain`, `GraphConnected`, `ParaphraseAligned`, `KeywordOverlap`, `MultiAgreement`. Stored in `CF_TYPED_EDGES` (32-byte composite key) with secondary index `CF_TYPED_EDGES_BY_TYPE`.

**Universal Theory of Learning (UTL).** The motivating framework for ContextGraph's learning subsystem. Models learning as ΔS · ΔC · ΔE composition: ΔS = surprise, ΔC = consolidation, ΔE = engagement, L = product. Implemented in `crates/context-graph-core/src/learner/mod.rs`.

**VICReg.** Variance-Invariance-Covariance Regularisation. A non-contrastive self-supervised loss family that uses variance and covariance terms to prevent representation collapse. ME-JEPA's loss is `latent_mse + vicreg_variance + vicreg_covariance`.

**Warm provider.** The GPU singleton that holds the 14 production embedders (and optionally the 7 learner heads) resident in VRAM. `LazyMultiArrayProvider` defers the load so the MCP server can come up before all embedders finish loading. `ProductionMultiArrayProvider` is the loaded form.

**`weighted RRF` (Reciprocal Rank Fusion).** ContextGraph's default fusion strategy. `RRF_K = 60`. Implemented in `crates/context-graph-core/src/fusion/mod.rs` and `crates/context-graph-storage/src/teleological/rocksdb_store/fusion.rs`.

---

## Cross-References

For deeper exploration of any term:

| Term | Source document |
|---|---|
| Reality Loop, measurement contract, fail-closed gates | `docs2/contextgraphspecs/22_reality_loop_subsystem.md` |
| ME-JEPA / DynamicJEPA, VICReg, surprise events | `docs2/contextgraphspecs/21_dynamicjepa_subsystem.md` |
| 14 embedders, asymmetric vectors, warm provider | `docs2/contextgraphspecs/05_embedding_subsystem.md` |
| 101 column families, schema versions, append-only logs | `docs2/contextgraphspecs/04_database_schema.md` |
| Search strategies, RRF, weight profiles | `docs2/contextgraphspecs/06_search_and_fusion.md` |
| Constellations (3 stores) | `docs2/contextgraphspecs/17_constellation_subsystem.md` |
| Learner state E15-E21, UTL, ΔS/ΔC/ΔE/L | `docs2/contextgraphspecs/20_learning_subsystem.md` |
| Teleological synergy and fusion | `docs2/contextgraphspecs/13_teleological_subsystem.md` |
| Reality-shift principle | `docs/ccreality/01b_reality_shift_principle.md` |
| ccreality optimizer skills | `.claude/skills/ccreality-*/SKILL.md` |
