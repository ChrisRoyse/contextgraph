# Training-Data Export Plan — Implementation Todo

**Plan**: `docs/TRAINING_DATA_EXPORT_PLAN.md`
**Session**: training-impl-2026-04-14
**Coordination**: all agents store/retrieve memories via context-graph MCP with `sessionId="training-impl-2026-04-14"`. Final summary memories are tagged with `IMPL-PHASE-<N>-DONE` in the content so later agents can `search_graph` for them.

## Phase 1 — TrainingRecord v2 foundation (Agent A)
- [ ] CF_TRAINING_RECORDS column family: const, options, descriptor list, counts bumped 51→52 and 20→21, tests updated
- [ ] `uuid_prefixed_cf_options` helper introduced and reused
- [ ] `context_graph_core::training::{TrainingRecord, TrainingEdge, KnnNeighbor, CausalLabel, TopicMembership}`
- [ ] Topic profile **read from `CF_TOPIC_PROFILES`**, not derived from vector magnitudes
- [ ] `CausalLabel` stores `rel_id: Uuid` (always) + `related_memory_id: Uuid` (peer when resolvable else nil)
- [ ] Session filter does `get_source_metadata(id)` exactly once per memory
- [ ] `includeIncomingEdges=true` builds reverse index once (`HashMap<Uuid, Vec<TrainingEdge>>`)
- [ ] `list_training_records(includeShape=true)` uses `multi_get_cf` batch reads
- [ ] `arm_doomsday_watchdog` logs-and-continues on spawn failure (no panic)
- [ ] Version byte + bincode encoding with shared `encode_v1` / `decode_v1`
- [ ] MCP surface: `export_training_corpus`, `list_training_records`, `get_training_record`, `count_training_records`
- [ ] FSV-1 through FSV-6 green

## Phase 5 — Temporal labels (folded into Agent A)
- [ ] `TemporalLabels { stored_hour_utc, day_of_week, month, age_seconds, session_sequence, relative_position, periodic_bucket, e2/e3/e4 norms }`
- [ ] `PeriodicBucket` enum (Weekday/Weekend × Morning/Afternoon/Evening/Night)
- [ ] `TrainingRecord.temporal_labels: Option<TemporalLabels>` populated when `includeTemporalLabels=true`
- [ ] FSV-19..FSV-22 green

## Phase 2 — Constellation compiler (Agent B, reads Phase-1 memories)
- [ ] `CF_CONSTELLATIONS` + `CF_CONSTELLATION_BY_SELECTOR` CFs
- [ ] `context_graph_core::constellation::{Constellation, ConstellationSelector, EmbedderStats, VectorKind}`
- [ ] Welford running-stats + 1024-sample reservoir for percentiles
- [ ] MCP: `compile_constellation`, `list_constellations`, `get_constellation`, `score_against_constellation`, `delete_constellation`
- [ ] FSV-7..FSV-10 green

## Phase 3 — Contrastive pair miner (Agent C, reads Phase-1 memories)
- [ ] `CF_CONTRASTIVE_PAIRS` + `CF_CONTRASTIVE_BY_KIND` + `CF_CONTRASTIVE_BY_ANCHOR`
- [ ] `ContrastivePair`, `AnomalyKind` enum
- [ ] Miner reuses existing `search_cross_embedder_anomalies` logic
- [ ] MCP: `mine_contrastive_pairs`, `list_contrastive_pairs`, `get_contrastive_pair`, `count_contrastive_pairs`
- [ ] FSV-11..FSV-14 green

## Phase 4 — Tucker-core export (Agent D, reads Phase-1 memories)
- [ ] `CpuTuckerCompressor` via streaming HOSVD (never materialize full 13×13×1024 tensor)
- [ ] `TrainingRecord.tucker_core: Option<TuckerCore>` populated when `includeTuckerCore=true`
- [ ] FSV-15..FSV-18 green
- [ ] GPU path deferred unless trivial; document the deferral

## Phase 6 — File-format CLI (Agent E, reads all prior phases)
- [x] `context-graph-cli export training-corpus --format parquet --out <path>`
- [x] Parquet first (HuggingFace datasets ecosystem), add HDF5/safetensors later
- [x] CLI reads from CFs via the shared helpers (no duplicate logic)
- [x] FSV: dump to file, reload via `parquet` reader, assert roundtrip

## Phase 7 — Code-simplifier review (Agent F)
- [ ] Reviews all of Phases 1-6
- [ ] Findings applied before final FSV

## Final — Full-state verification (me)
- [ ] Build: `cargo build --release -p context-graph-mcp` with `CUDARC_CUDA_VERSION=13000` — zero errors, zero warnings
- [ ] Tests: `cargo test --workspace` — all green
- [ ] Zombie: `python3 tests/test_no_zombies.py` — 4/4 pass
- [ ] Harness: `python3 tests/fsv_harness.py` — happy path + all edge cases pass
- [ ] Live MCP verification: ingest synthetic data → export → read back → math identities hold
- [ ] All 22 FSV items (FSV-1..FSV-22) green

## Review / retrospective
(To be filled after each phase — what worked, what needed rework.)
