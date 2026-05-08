# 01 — AgentDB Architecture (what it actually is)

> **🔒 Reference-only.** Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md` — never vendor, path-dep, or npm-depend on agentdb. Read this doc to understand the design; reimplement any borrowed primitive cleanly in our own crates.

## One-line definition

**AgentDB is a single-file `.rvf` cognitive container** — a TypeScript runtime that wraps a Rust HNSW vector engine (`ruvector`), a Thompson-Sampling adaptive solver (`@ruvector/rvf-solver`), and a self-learning trajectory tracker (`@ruvector/sona`), and exposes the result as a sql.js (or better-sqlite3) database with **41 MCP tools**.

Their pitch: "store vectors, give the database your reward signal, the database gets sharper at ranking next query." +36% search-quality lift from feedback alone, allegedly.

## The five layers

```
┌─────────────────────────────────────────────────────────────┐
│ LAYER 5 — MCP server (41 tools)                              │
│   src/mcp/agentdb-mcp-server.ts (2367 lines)                 │
│   tools: agentdb_*, reflexion_*, skill_*, causal_*,          │
│          recall_*, learning_*, attention_*                   │
└──────────────────────────────┬──────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────┐
│ LAYER 4 — Controllers (the cognitive primitives)             │
│   ReflexionMemory     1391 LOC  episodic replay              │
│   SkillLibrary         924 LOC  lifelong learning            │
│   CausalMemoryGraph    875 LOC  do-calculus over memory      │
│   CausalRecall         505 LOC  utility-based reranker       │
│   ExplainableRecall    746 LOC  Merkle provenance certs      │
│   NightlyLearner       664 LOC  doubly-robust uplift discovery│
│   LearningSystem      1287 LOC  9 RL algorithms              │
│   AttentionService    1020 LOC  Flash/Hyperbolic/MoE/Linear  │
│   ReasoningBank        675 LOC  pattern store + GNN enhance  │
│   MemoryController     461 LOC  unified memory API           │
│   MMRDiversityRanker   187 LOC  λ·rel − (1−λ)·max-sim        │
│   ContextSynthesizer   285 LOC  narrative summary from k mems│
│   MetadataFilter       280 LOC  MongoDB-style $eq/$gt/...    │
│   HNSWIndex            581 LOC  hnswlib-node wrapper          │
│   SparsificationService 492 LOC PPR / spectral / random walk │
│   MincutService        434 LOC  graph partition              │
└──────────────────────────────┬──────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────┐
│ LAYER 3 — RVF self-learning backend stack                    │
│   SelfLearningRvfBackend  487 LOC  the unified entry         │
│   RvfBackend              749 LOC  base HNSW + persistence    │
│   SonaLearningBackend     357 LOC  micro-LoRA + EWC++         │
│   ContrastiveTrainer      559 LOC  InfoNCE + hard negatives   │
│   AdaptiveIndexTuner      631 LOC  TemporalCompressor 5-tier  │
│   SemanticQueryRouter     456 LOC  intent → handler routing   │
│   FederatedSessionMgr     526 LOC  cross-session aggregation  │
│   RvfSolver               312 LOC  3-loop adaptive policy     │
│   SolverBandit            270 LOC  Thompson Sampling          │
│   NativeAccelerator       489 LOC  NAPI bridge (rvf-wasm)     │
└──────────────────────────────┬──────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────┐
│ LAYER 2 — Optional native deps (lazy-loaded)                 │
│   ruvector              0.1.30  Rust HNSW + SIMD              │
│   @ruvector/rvf         0.1.9   .rvf file format              │
│   @ruvector/rvf-solver  0.1.7   3-loop Thompson Sampling      │
│   @ruvector/sona        0.1.4   federated learning            │
│   @ruvector/attention   0.1.2   FlashAttention WASM           │
│   @ruvector/gnn         0.1.23  TensorCompress / GNN          │
│   @ruvector/router      0.1.15  semantic routing              │
│   @xenova/transformers  2.17.2  default embedder              │
└──────────────────────────────┬──────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────┐
│ LAYER 1 — Storage                                            │
│   sql.js (WASM SQLite, default) OR better-sqlite3 (NAPI)     │
│   schemas: schema.sql + frontier-schema.sql                  │
└─────────────────────────────────────────────────────────────┘
```

The whole stack is *gracefully degrading*: every native dep is in `optionalDependencies`, every controller has a "JavaScript fallback" path. The `.rvf` cognitive container claim is real but only when `@ruvector/rvf-node` is actually loaded; otherwise it's vanilla SQLite with extra ceremony.

## The core contract — `SelfLearningRvfBackend`

This is the load-bearing class. Every other capability bolts onto it. The flow:

```
insertAsync(id, embedding, metadata):
  RvfBackend.insertAsync                       ← persist vector + sql row
  recordMutationWitness("insert:<id>")         ← extend SHAKE-256 chain
  TemporalCompressor.compress(id, emb, freq)   ← tier the cold copy
  IndexHealthMonitor.recordInsert(latency)     ← track HNSW health

searchAsync(query, k):
  SemanticQueryRouter.route(query)             ← classify intent
  SonaLearningBackend.enhance(query)           ← micro-LoRA refine
  SonaLearningBackend.beginTrajectory()        ← open a trajectory
  RvfBackend.searchAsync(enhanced, k, ef)      ← actual HNSW
  SonaLearningBackend.addStep(traj, sims)      ← record step
  trajectoryId = activeTrajectories.set(...)   ← pending feedback
  return results

recordFeedback(queryId, quality):
  SonaLearningBackend.endTrajectory(traj, q)   ← finalize learning
  ContrastiveTrainer.createSamples(traj, q)    ← sample for InfoNCE
  FederatedSessionManager.recordTrajectory()   ← cross-session push

tick():                                        ← background heartbeat
  sona.tick()                                  ← micro-LoRA step
  trainer.trainBatch(buffer)                   ← contrastive batch
  solver.train({count: 50})                    ← 3-loop policy step
  compressor.updateFrequency()                 ← decay tiers
  healthMonitor.assess(indexStats)             ← propose rebuild
  federated.consolidate()                      ← LoRA aggregate
  if tickCount % 100 == 0:
    runAcceptanceCheck()                       ← A/B/C ablation
```

That's it. Every "magic" claim in the README reduces to one of these eight operations. Once you internalize this, the rest of the codebase is decoration.

## What the `RvfSolver` three-loop architecture actually is

ADR-006 is the most useful document in their tree. Summary:

```
┌──────────── SLOW (KnowledgeCompiler) ───────────┐
│ Distills observations into signature-keyed       │
│ "compiled configs". Once a query bucket is       │
│ stable enough, decisions become cache lookups,   │
│ no Thompson Sampling needed.                     │
└───────────────────┬──────────────────────────────┘
                    │
┌───────────────────▼──────────────────────────────┐
│ MEDIUM (PolicyKernel — Thompson Sampling)        │
│ 18 context buckets = 3 range × 3 distractor × 2  │
│ noise. Each bucket has Beta(α,β) per arm + cost  │
│ EMA. Speculative dual-path on high variance.     │
│ Picks ef_search per query.                       │
└───────────────────┬──────────────────────────────┘
                    │
┌───────────────────▼──────────────────────────────┐
│ FAST (Constraint propagation solver)              │
│ Executes the candidate, reports outcome back.    │
└───────────────────┬──────────────────────────────┘
                    │
┌───────────────────▼──────────────────────────────┐
│ SHAKE-256 witness chain (73 bytes/entry)          │
│ Tamper-evident proof of every operation.         │
└──────────────────────────────────────────────────┘
```

The clever bit: A/B/C acceptance ablation. They periodically re-run a holdout in three modes:
- **Mode A (fixed)** — static heuristic baseline.
- **Mode B (compiler)** — KnowledgeCompiler patterns only.
- **Mode C (full learned)** — Thompson + Compiler.

Mode C must beat both A and B on accuracy AND cost AND noise-robustness AND zero-violations to count as "passing." If it doesn't, the witness chain records the failure and the system rolls back. This is exactly the kind of "did learning actually improve us, or did we just memorize the prompts?" check that a self-modifying harness needs.

## The MCP surface (33 tools, README claims 41)

The README claims 41 tools across 6 families. Counting `name: '` in `src/mcp/agentdb-mcp-server.ts`, I see **33** distinct registered tools as of HEAD. The discrepancy is probably attention-tools-handlers.ts (586 LOC) registering more at runtime. Either way, the families are:

| Family | Examples | Notes |
|--------|----------|-------|
| Core CRUD | `agentdb_init`, `agentdb_insert`, `agentdb_search`, `agentdb_delete` | The "vector database" baseline |
| Pattern store | `agentdb_pattern_store`, `agentdb_pattern_search`, `agentdb_pattern_stats` | Backed by `ReasoningBank.ts` |
| Reflexion | `reflexion_store`, `reflexion_retrieve`, `reflexion_store_batch` | Episodes with critique + reward |
| Skill library | `skill_create`, `skill_search`, `skill_create_batch` | Successful patterns → reusable skills |
| Causal | `causal_add_edge`, `causal_query`, `recall_with_certificate` | Edges with uplift + Merkle cert |
| Learner | `learner_discover`, `agentdb_consolidate` | NightlyLearner pipeline |
| Learning system | `learning_start_session`, `learning_predict`, `learning_feedback`, `learning_train`, `learning_metrics`, `learning_transfer`, `learning_explain`, `experience_record`, `reward_signal` | 9 RL algos |
| Stats | `agentdb_stats`, `db_stats`, `agentdb_pattern_stats` | Counters |

What matters for transplant: every tool has a small, regular JSON schema. They don't try to be smart with the schema; the controllers behind do all the work. That's a good pattern to copy.

## Storage model

```sql
-- schema.sql
episodes (id, ts, session_id, task, input, output, critique, reward, success,
          latency_ms, tokens_used, tags JSON, metadata JSON)
episode_embeddings (episode_id, embedding BLOB, dimension)
skills (id, name, description, signature JSON, code, success_rate, uses,
        avg_reward, avg_latency_ms, created_from_episode, metadata JSON)
skill_embeddings (skill_id, embedding BLOB, dimension)
skill_links (parent_skill_id, child_skill_id, relationship, weight, metadata JSON)
causal_edges (id, from_memory_id, from_memory_type, to_memory_id, to_memory_type,
              similarity, uplift, confidence, sample_size, evidence_ids JSON,
              experiment_ids JSON, confounder_score, mechanism, metadata JSON)
causal_experiments (id, name, hypothesis, treatment_id, treatment_type,
                    control_id, start_time, end_time, sample_size,
                    treatment_mean, control_mean, uplift, p_value,
                    confidence_interval_low/high, status, metadata JSON)
causal_observations (experiment_id, episode_id, is_treatment, outcome_value,
                     outcome_type, context JSON)

-- frontier-schema.sql
recall_certificates (id, query_id, query_text, chunk_ids JSON, chunk_types JSON,
                     minimal_why JSON, redundancy_ratio, completeness_score,
                     merkle_root, source_hashes JSON, proof_chain JSON,
                     policy_proof, policy_version, access_level, latency_ms)
provenance_sources (id, source_type, source_id, content_hash, parent_hash,
                    derived_from JSON, creator, metadata JSON)
learning_sessions, learning_experiences, learning_policies, learning_state_embeddings (RL tables)
```

Total: ~12 tables. Compare to ContextGraph's 51 RocksDB CFs — agentdb is a much smaller-surface system. Whether that's "elegantly minimal" or "missing capabilities" depends entirely on whether you need 13-embedder fusion.

## Single-file `.rvf` claim

The "single-file cognitive container" sales pitch is structurally true: the `.rvf` format (from `@ruvector/rvf`) packs vectors + HNSW index + sql tables + learning state + audit log. Convenience for consumers, not a deep technical claim. ContextGraph's RocksDB-per-CF layout is more flexible and more transparent for FSV; the `.rvf` model is closer to "git-pack of the whole DB" — fine for distribution, harder to introspect.

## What's notably missing

- **No multi-embedder fusion.** Single embedder per DB. No E1+E5+E7 weighted RRF.
- **No real causal discovery.** `learner_discover` is doubly-robust uplift estimation against episode outcomes — useful for "did skill X actually cause higher reward?" but not for the structural-causal-graph extraction that ContextGraph's E5 + LLM agent does.
- **No GGUF inner actor.** This is a memory layer, not a model-running harness.
- **No SWE-bench / official-evaluator concept.** Reward is a number you record; nothing validates correctness against an external oracle.
- **No CUDA path.** Their "native" is NAPI + WASM; no CUDA, no Tensor cores, no candle integration.

These omissions are why you wouldn't replace ContextGraph with agentdb. They're also why the *ideas* in agentdb are much more transferable than the *code*.

## Read next

- `02_CAPABILITIES_INVENTORY.md` for the full capability list, line-by-line.
- `03_REALITY_LOOP_APPLICATIONS.md` for the named transplants to ccreality.
- `04_JEPA_APPLICATIONS.md` for the named transplants to ME-JEPA / 5090jepa.
