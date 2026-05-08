# 05 — Quick Wins vs Deep Integration

A triage of every transplant proposed in `03_REALITY_LOOP_APPLICATIONS.md` and `04_JEPA_APPLICATIONS.md`, sorted by effort × leverage. Pick from the top down.

---

## Effort scale

| Tier | Effort | What it means |
|------|--------|---------------|
| **S** | < 1 day | Single file, single function, isolated test. |
| **M** | 1-3 days | New module, MCP tool addition, schema migration. |
| **L** | 3-7 days | Cross-module change touching CLI, MCP, optimizer, persistence. Single ADR. |
| **XL** | > 7 days | Multi-ADR effort, requires coordination, real risk of regression. |

## Leverage scale

| Tier | Effect |
|------|--------|
| **★★★** | Changes the per-attempt success rate or the auditability story directly. |
| **★★** | Quality-of-life improvement that makes the optimizer / verifier noticeably better. |
| **★** | Useful but easily lived-without. |

---

## Tier 1 — Ship this week (S effort, ★★★ leverage)

| # | Transplant | Where | Effort | Leverage | Notes |
|---|------------|-------|--------|----------|-------|
| 1.1 | **SolverBandit Rust port** | shared (ccreality §2 + JEPA §2) | S (0.5 day) | ★★★ | The single highest-leverage primitive. 270 LOC TS → ~400 LOC Rust + tests. Once it's built, all the bandit-powered transplants become 1-day wires. |
| 1.2 | MMR diversity reranker | ccreality §7, JEPA §10 | S (0.5 day) | ★★ | 50-line greedy. Steals from `MMRDiversityRanker.ts`. Bonus: improves diagnosis quality immediately. |
| 1.3 | Reflexion Episode `critique` field | ccreality §1, JEPA §11 | S (0.5 day) | ★★ | Schema additions on existing records. No migration risk (pre-production per memory). |
| 1.4 | Retrieval result `tags: Vec<String>` | ccreality §1 | S (0.5 day) | ★★ | Same op as 1.3. Unlocks tag-filter queries. |
| 1.5 | ContextSynthesizer narrative summary | ccreality §8, JEPA §9 | S/M (1 day) | ★★ | Fully deterministic — replaces 3-5 raw artifact reads at turn start. |

**Tier 1 total effort: ~3-4 days. Tier 1 ROI: massive.**

The optimizer materially improves after Tier 1: bandits start learning across turns (1.1), retrieval is more diverse (1.2), every attempt has structured failure tags (1.3, 1.4), and turn-start summary is deterministic (1.5).

---

## Tier 2 — Ship within two weeks (M effort, ★★/★★★ leverage)

| # | Transplant | Where | Effort | Leverage | Notes |
|---|------------|-------|--------|----------|-------|
| 2.1 | **Wire SolverBandit into 4 ccreality decision points** | ccreality §2 | M (1 day post 1.1) | ★★★ | Trigger / next-prompt-delta / retrieval-strategy / efSearch all under bandit control. |
| 2.2 | **Wire SolverBandit into JEPA planner head** | JEPA §2 | M (2 days post 1.1) | ★★★ | The predictor's argmax becomes a Thompson-sampled selection — actively explores. |
| 2.3 | CausalRecall α·sim+β·uplift−γ·cost reranker | ccreality §3, JEPA §4 | M (1 day) | ★★ | Single rerank function. Plug in everywhere a similarity-only rank exists. α/β/γ default to (0.7, 0.2, 0.1); tune via 2.1's bandit later. |
| 2.4 | MetadataFilter `$eq/$gt/...` query operators | ccreality §9 | M (1 day) | ★★ | Generalizes `attempts_history_query`. Useful for debugging too. |
| 2.5 | New MCP tools: `attempts_query_reflexion`, `attempts_critique_summary`, `attempts_success_strategies`, `attempts_synthesize` | ccreality §1, §8 | M (1.5 days) | ★★★ | Builds on 1.3-1.5. Each is a thin wrapper. |
| 2.6 | SHAKE-256 witness chain (shared) | ccreality §5, JEPA §1 | M (1 day) | ★★★ | Shared 73-byte chain, two consumers (optimizer + JEPA training-runs). The verifier MCP/CLI tools are another 0.5 day each. |

**Tier 2 total effort: ~7-8 days. Cumulative with Tier 1: ~12 days.**

After Tier 2 the loop is structurally complete: bandits decide, certificates audit, narratives summarize, queries are richer, and the witness chain proves nothing got reordered.

---

## Tier 3 — Ship as standalone ADR-grade work (L effort, ★★/★★★ leverage)

| # | Transplant | Where | Effort | Leverage | Notes |
|---|------------|-------|--------|----------|-------|
| 3.1 | **ExplainableRecall Merkle certificates** | ccreality §4, JEPA §5 | L (2 days base + 2 days verifier + 1 day FSV) | ★★★ | Single Merkle module, two consumers. Adds a strong FSV story for both subsystems. Requires its own ADR because it touches the recommendation surface. |
| 3.2 | **NightlyLearner pipeline** | ccreality §6, JEPA §3 | L (3-4 days) | ★★★ | Doubly-robust estimator + dependency on bandit + dependency on Reflexion schema. Real benefit, real complexity. |
| 3.3 | TemporalCompressor 5-tier policy | JEPA §8 | L (3 days) | ★★ | Adds rich tier semantics on top of existing PQ-8. Requires bit-exact dequant tests for JEPA latents. |
| 3.4 | SemanticQueryRouter intents | ccreality §implicit | L (2-3 days) | ★ | Useful when there's > 1 retrieval mode; right now ContextGraph already routes via 4 search strategies, so leverage is lower. |
| 3.5 | LearningSystem `transfer` semantics for JEPA | JEPA §12 | L (3 days) | ★★ | Cross-domain-pack skill transfer. Deeply tied to instrument projection — sequence after the JEPA dynamics are stable. |
| 3.6 | SparsificationService PPR | JEPA §6 | L (2-3 days) | ★★ | Trajectory-importance scoring for JEPA training sampler. Worth doing; not blocking. |
| 3.7 | MincutService for leakage check + train/test split | JEPA §7 | L (2-3 days) | ★★ | Same engine, two graphs. Solid value. |

**Tier 3 total effort: ~16-20 days. Each is standalone ADR-grade.**

Sequence Tier 3 by what's blocking real workstreams. If FSV is the bottleneck → 3.1 first. If JEPA training quality is the bottleneck → 3.2/3.3/3.6/3.7. If the bandit is well-tuned and you want richer routing → 3.4.

---

## Tier 4 — Defer or skip

| # | Transplant | Why deferred |
|---|------------|--------------|
| 4.1 | FederatedSessionManager | No parallel optimizer sessions today. Listed in ccreality §10 for completeness. |
| 4.2 | ContrastiveTrainer projection layer | You have LoRA-on-embedder. The projection layer is a *lower-risk alternative*; not needed if LoRA is working. |
| 4.3 | AttentionService Flash/Hyperbolic/MoE/Linear | Read for ideas. Don't import — you have your own candle CUDA path on RTX 5090. |
| 4.4 | ReasoningBank (pattern store) | Episodes already cover this. No reason to maintain a parallel schema. |
| 4.5 | HNSWIndex (hnswlib-node wrapper) | RuVector is more capable. |
| 4.6 | `.rvf` cognitive container format | Single-file packaging is convenient for npm distribution; not needed for ContextGraph's RocksDB layout. |
| 4.7 | LearningSystem 9-RL toy implementations | Steal interface (`predict / feedback / train / transfer / explain`), implement against your own backend. |
| 4.8 | RvfSolver 3-loop full architecture | Steal SolverBandit (1.1) + witness chain (2.6) — those are the two parts that earn their keep. The KnowledgeCompiler's pattern cache is overkill for the optimizer's small decision space. |

---

## Recommended one-week sprint plan

If the operator wants to start tomorrow with maximum leverage and bounded risk:

**Day 1**
- Implement SolverBandit in Rust (1.1). Single file `crates/context-graph-cli/src/optimizer/solver_bandit.rs` + tests. JSON-serializable state in `CF_OPTIMIZER_BANDIT`. ~400 LOC + tests.
- Add `critique` + `tags` fields to `AttemptSummary` (1.3 + 1.4). Schema-only.

**Day 2**
- Implement MMR diversity reranker (1.2). Single function in `crates/context-graph-cli/src/optimizer/mmr.rs`. ~80 LOC.
- Implement ContextSynthesizer-equivalent narrative summary (1.5) under `crates/context-graph-cli/src/optimizer/synthesize.rs`. ~250 LOC + tests.

**Day 3**
- Wire SolverBandit into the 4 ccreality decision points (2.1):
  - trigger decision (rule-based today → bandit-augmented)
  - next-prompt-delta template selection
  - retrieval strategy
  - efSearch
- Each wire is ~50 LOC + tests.

**Day 4**
- Implement CausalRecall reranker (2.3) in `crates/context-graph-cli/src/optimizer/causal_recall.rs`. ~150 LOC.
- Implement MetadataFilter `$`-operator compiler (2.4). ~300 LOC + tests.

**Day 5**
- Wire new MCP tools (2.5): `attempts_query_reflexion`, `attempts_critique_summary`, `attempts_success_strategies`, `attempts_synthesize`. Each is a thin adapter from existing infrastructure.

**Day 6**
- Implement SHAKE-256 witness chain (2.6) in `crates/context-graph-cli/src/witness_chain.rs`. ~250 LOC + tests + verifier CLI.

**Day 7 — verification**
- Run a full smoke test sequence:
  - `mcp__cgreality__reality_run_smoke` to confirm engine pipeline.
  - One real attempt on a fresh task to confirm bandit arms get pulled and rewards recorded.
  - Verify witness chain by reading the chain back.
- Write SMAP report to `./memory/decisions/agentdb-tier-1-and-2-adopted.md` per CLAUDE.md §6.

End of sprint: bandits learning, certificates next sprint, NightlyLearner sprint after that.

---

## Risk gating

Before each tier:

| Tier | Required prerequisite |
|------|------------------------|
| 1 | None — independent of everything. |
| 2.1, 2.2 | Tier 1.1 (SolverBandit) shipped + verified. |
| 2.5 | Tier 1.3, 1.4 (schema fields) shipped. |
| 2.6 | None — independent. |
| 3.1 | Tier 2.6 shipped (Merkle certs reuse the chain's hash machinery). |
| 3.2 | Tier 2.1 + 2.5 shipped (NightlyLearner depends on bandit + reflexion query). |
| 3.3 | Independent. |
| 3.5 | DynamicJEPA trajectory schema stable. |
| 3.6, 3.7 | DynamicJEPA Phase 0 shipped. |

---

## Ship signal

You'll know each tier earned its keep if:

- **After Tier 1**: turn-start latency drops (deterministic synthesis), and the optimizer's recommendations cite tag-filtered failure summaries.
- **After Tier 2**: SolverBandit per-context win-rate logs show >1.5x improvement over uniform-random selection on at least one of the four decision points within ~50 turns.
- **After Tier 3.1**: the FSV protocol shows a 1-call recommendation re-verification path; certificate verify time < 200ms.
- **After Tier 3.2**: at least one auto-discovered (failure_pattern → harness_fix) hypothesis gets promoted via uplift > 0.05 with confidence > 0.7. (If none promote in 200 attempts, the consolidation pipeline is mis-tuned.)

If a tier doesn't hit its ship signal, surface a high-severity blocker in `./memory/blockers/` per CLAUDE.md §6 and re-tune before adding the next tier on top.
