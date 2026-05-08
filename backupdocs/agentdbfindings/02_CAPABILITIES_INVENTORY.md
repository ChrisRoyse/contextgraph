# 02 — Capabilities Inventory

> **🔒 Reference-only.** Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md` — every "steal" / "port" verdict in the table at the bottom of this doc means **clean-room reimplement the idea**, never copy code or path-dep to agentdb's tree.

A capability-by-capability catalog of what agentdb actually offers, with file refs, real algorithm names, and honest assessment of whether it's interesting or filler. Roughly ordered by "load-bearing" → "convenient" → "decorative."

---

## A. Episodic / Reflexion memory

**File:** `src/controllers/ReflexionMemory.ts` (1391 LOC)
**Schema:** `episodes` + `episode_embeddings` (or graph adapter)
**Reference:** Shinn et al., "Reflexion: Language Agents with Verbal Reinforcement Learning" (arXiv:2303.11366)

**Episode shape:**
```typescript
{ id, ts, sessionId, task, input?, output?, critique?, reward, success,
  latencyMs?, tokensUsed?, tags?, metadata? }
```

**Operations:**
- `storeEpisode(e)` — embed `task`, persist + index. Returns numeric id.
- `retrieveRelevant({task, k, minReward, onlyFailures, onlySuccesses, timeWindowDays})` — k-NN over `episode_embeddings` with optional reward / outcome / time filter.
- Caches via `QueryCache` keyed on category `'episodes'`; writes invalidate.
- Persists to SQLite even when graph backend is primary (issue #128 fix — they call this out as "data must survive process restarts").

**Useful traits:**
- The schema is *clean*. `task / input / output / critique / reward / success` is the right minimal episode shape.
- Failure-only and success-only retrieval are first-class — same query path, different filter.
- Cache is invalidated by *category*, not individual row, which sidesteps the whole "what does this query depend on" problem.

**Mediocre traits:**
- One embedder. No fusion.
- "Critique" is just a string — no structured guidance about *what* went wrong.
- Time-window filter is days, not steps or attempts.

---

## B. Skill library

**File:** `src/controllers/SkillLibrary.ts` (924 LOC)
**Schema:** `skills` + `skill_embeddings` + `skill_links`
**Reference:** Wang et al., "Voyager: An Open-Ended Embodied Agent with Large Language Models" (arXiv:2305.16291)

**Skill shape:**
```typescript
{ id, name, description, signature: { inputs, outputs }, code,
  successRate, uses, avgReward, avgLatencyMs, createdFromEpisode, metadata }
```

**SkillLink shape:**
```typescript
{ parentSkillId, childSkillId,
  relationship: 'prerequisite' | 'alternative' | 'refinement' | 'composition',
  weight, metadata }
```

**Operations:**
- `createSkill` — auto-embed `name + description + signature` and persist.
- `searchSkills({task, k, minSuccessRate, preferRecent})` — semantic match.
- `composeSkills(parent, child, relationship)` — chain into a graph.
- Bandit-driven composition selection (Thompson sample over `(taskType, skillSequence)` pairs) — *if* SolverBandit is wired in.

**Useful traits:**
- The four relationship types (prerequisite/alternative/refinement/composition) are exactly the chord types you'd want for chaining harness "tactics" or JEPA "skills."
- Skills are first-class persistent objects with success-rate + usage stats — not just text.

**Mediocre traits:**
- No real composition execution — just a graph; you still have to build the runtime to chain skills.
- The "bandit picks composition" claim is half-implemented; the bandit is generic and the link to SkillLibrary is light.

---

## C. Causal memory graph (do-calculus over memories)

**File:** `src/controllers/CausalMemoryGraph.ts` (875 LOC)
**Reference:** Pearl's do-calculus + uplift modeling

**CausalEdge shape:**
```typescript
{ fromMemoryId, fromMemoryType: 'episode'|'skill'|'note'|'fact',
  toMemoryId, toMemoryType,
  similarity, uplift /* E[y|do(x)] - E[y] */, confidence, sampleSize,
  evidenceIds, experimentIds, confounderScore, mechanism, metadata }
```

**CausalExperiment shape:**
```typescript
{ name, hypothesis, treatmentId, treatmentType, controlId,
  startTime, endTime, sampleSize,
  treatmentMean, controlMean, uplift, pValue,
  confidenceIntervalLow, confidenceIntervalHigh, status }
```

**Operations:**
- `addCausalEdge` — embed mechanism, persist edge with uplift.
- `runExperiment` — A/B over a hypothesis, persist treatment vs control means + p-value.
- `queryCausalEffect(intervention, outcome)` — Cypher-style traversal.
- `findIndirectCauses(node)` — multi-hop traversal with confidence decay.
- Optional **HyperbolicAttention** (Poincaré embeddings) for tree-structured causal chain retrieval — feature-flagged, default off.

**Useful traits:**
- Storing **uplift + confidence + sample size** as first-class edge attributes is the right schema for "did action X actually cause outcome Y, or was it correlation?"
- A/B experiment table with treatment/control means and p-values is one short hop from a real causal-inference workflow.

**Mediocre traits:**
- ContextGraph's E5 causal-asymmetric embedder is a more rigorous structural-causal model than agentdb's "embed the mechanism string." Don't downgrade.
- Hyperbolic attention is feature-flagged and basically untested in their CI — treat as research code.

---

## D. CausalRecall — utility-based reranker

**File:** `src/controllers/CausalRecall.ts` (505 LOC)

**The reranking formula (this is the load-bearing primitive):**
```
Utility(candidate) = α · similarity(candidate, query)
                   + β · uplift(candidate)
                   − γ · latencyCost(candidate)
```

Defaults: `α=0.7, β=0.2, γ=0.1, minConfidence=0.6`.

**Pipeline:**
```
recall(queryId, queryText, k=12, requirements?, accessLevel?):
  1. embed query
  2. vectorSearch(queryEmbedding, k*2)            — fetch 2k for reranking
  3. loadCausalEdges(candidates.map(c => c.id))   — for uplift scores
  4. rerankByUtility(candidates, edges)           — apply α/β/γ formula
  5. issueCertificate(...)                        — Merkle proof
  6. return { candidates, certificate, metrics }
```

**Why this primitive matters:** any time you have (a) a similarity score, (b) some "did this work last time?" reward signal, and (c) some cost, you can rank by `α·sim + β·uplift − γ·cost` and the only question is how you tune α/β/γ. agentdb just hard-codes the defaults; the better move is to put α/β/γ themselves under bandit control.

---

## E. ExplainableRecall — Merkle provenance certificates

**File:** `src/controllers/ExplainableRecall.ts` (746 LOC)

**Certificate shape:**
```typescript
{ id, queryId, queryText,
  chunkIds, chunkTypes,
  minimalWhy,                  // minimal hitting set of facts justifying the answer
  redundancyRatio,             // len(chunks)/len(minimalWhy)
  completenessScore,           // fraction of stated requirements met
  merkleRoot, sourceHashes, proofChain,
  policyProof, policyVersion, accessLevel,
  latencyMs }
```

**Operations:**
- `issueCertificate(query, retrievedChunks, requirements?)` — compute Merkle root over `(chunkId, contentHash)` pairs, build proof chain, store.
- `verifyCertificate(certId)` — recompute proof chain, compare to stored.
- `extractMinimalWhy(chunks, requirements)` — minimum hitting set (greedy approximation).
- Optional **GraphRoPE** for hop-distance-aware retrieval — feature-flagged.

**Why this matters for both ccreality and JEPA:** "every retrieval comes with a certificate that proves *why* this particular set of memories justifies this answer" is exactly the FSV-style evidence requirement that ME-JEPA Stage 0 demands. The Merkle root + proof chain is independent re-readback machinery — same shape as `ledger.sqlite` but content-addressed.

---

## F. NightlyLearner — automated causal discovery & consolidation

**File:** `src/controllers/NightlyLearner.ts` (664 LOC)

**Doubly-robust learner formula:**
```
τ̂(x) = μ₁(x) − μ₀(x) + a·(y − μ₁(x))/e(x) − (1−a)·(y − μ₀(x))/(1 − e(x))
```
where μ₀, μ₁ are control/treatment outcome models and e(x) is the propensity score.

**Pipeline (`run()`):**
1. **Discover** new causal edges from episode patterns (similarity-cluster → edge).
2. **Run A/B experiments** on promising hypotheses (auto-select treatment/control episodes by tag).
3. **Calculate uplift** for completed experiments using doubly-robust estimator.
4. **Prune low-confidence edges** (`< confidenceThreshold`, default 0.6).
5. **Update rerank weights** based on per-edge performance.

**Optional FlashAttention consolidation** — block-wise computation for large episode buffers; feature-flagged.

**Why this matters:** the ccreality optimizer effectively wants this *exact* loop — "every N attempts, scan history, propose new (failure-mode → harness-fix) causal edges, A/B test the next 10 attempts on those edges, prune what didn't help." agentdb has it as a single-class pipeline. Stealable.

---

## G. LearningSystem — 9 RL algorithms

**File:** `src/controllers/LearningSystem.ts` (1287 LOC)

The 9: Q-Learning, SARSA, DQN, Policy Gradient, Actor-Critic, PPO, Decision Transformer, MCTS, Model-Based RL.

**Honest assessment:** these are *toy implementations* of named algorithms. Q-Learning is `Map<(state, action), qValue>`. DQN is "store experience, train a tiny MLP off-line." Decision Transformer is barely more than a typed wrapper. They are **not** competitive with stable-baselines3 / Tianshou / cleanRL.

**What's useful here:** the *interface* is clean.
```typescript
session.predict(state) → { action, confidence, qValue, alternatives }
session.feedback(state, action, reward, nextState, success)
session.train({ epochs, batchSize, learningRate }) → { epochs, finalLoss, avgReward, convergenceRate }
session.transfer(sourceSession, targetSession, { minSimilarity, transferType })
session.explain(query, { explainDepth, includeConfidence, includeEvidence, includeCausal })
```

That interface generalizes well. Steal the interface, implement against your own real RL backend.

---

## H. AttentionService — Flash / Hyperbolic / MoE / Linear / Multi-head

**File:** `src/controllers/AttentionService.ts` (1020 LOC)
**Backed by:** `@ruvector/attention` (NAPI) + `ruvector-attention-wasm` (WASM fallback)

Supported mechanisms:
- **MultiHeadAttention** — standard transformer attention.
- **FlashAttention** — memory-efficient blockwise.
- **HyperbolicAttention** — Poincaré ball, for tree-structured retrieval.
- **MoEAttention** — mixture-of-experts gating.
- **LinearAttention** — O(n) complexity.

**Why this matters less than it seems:** for ccreality this is irrelevant — you don't compute attention in the optimizer. For JEPA this *might* matter as an alternative implementation reference, but you already have your own candle-based attention path on RTX 5090 and `@ruvector/attention` won't have an sm_120 CUDA kernel.

**Read for:** the `AttentionConfig` schema (it covers FlashAttention, sparsification, partitioning all in one config object) is a nice forcing function for what your own DynamicJEPA attention config should expose.

---

## I. SemanticQueryRouter — intent routing

**File:** `src/backends/rvf/SemanticQueryRouter.ts` (456 LOC)

**Pattern:** register N intents (each with embedded exemplars), route every query to top-k matching intents using HNSW, return the intent name + score + metadata. Used by `SelfLearningRvfBackend` to decide which `route` to attribute a search to (so trajectories carry an intent label).

**Why this matters:** if you have multiple harness "modes" (test-failure vs lint-failure vs docstring-only attempts; gridworld vs counter_world domain packs), an intent router is the cleanest way to dispatch without writing a giant `match` statement.

**ContextGraph already does this implicitly** through the 4 search strategies + weight profiles. Their version is just smaller and more explicit.

---

## J. TemporalCompressor — 5-tier access-frequency-based vector compression

**File:** `src/backends/rvf/AdaptiveIndexTuner.ts` (631 LOC) — `TemporalCompressor` class.

Tier table (from access frequency):
```
freq >= 0.8: none      0% savings   (hot)
freq >= 0.6: half     50% savings   (warm) — float16
freq >= 0.4: pq8      75% savings   (cool) — 8-bit scalar quant
freq >= 0.2: pq4      87.5% savings (cold) — 4-bit scalar quant
freq <  0.2: binary   96% savings   (frozen) — 1-bit sign
```

Decay: `freq *= 0.99` per tick. Bandit picks the tier per id under uncertainty.

Optional **Matryoshka-style dimensional truncation** for cold tiers (truncate from 1024D → 256D for `pq4` etc.). The `truncatedDim` field is recorded so the index knows to pad on rehydrate.

**Why this matters:** ContextGraph already has PQ-8 quantization in the `quantization/` module and Matryoshka128 indexes. agentdb's contribution is the **5-tier policy with bandit-picked tier**, which is a generalization over ContextGraph's binary "quantized? yes/no" choice. Stealable as a refinement.

---

## K. ContrastiveTrainer — InfoNCE + hard negatives + curriculum

**File:** `src/backends/rvf/ContrastiveTrainer.ts` (559 LOC)

**Loss:** InfoNCE.
**Mining:** hard-negative selection by similarity threshold per curriculum stage.
**Optimizer:** AdamW (native via `NativeAccelerator` if available, else JS fallback).
**Curriculum:** 3 stages (default), each with `negativeCount`, `hardNegativeThreshold`, `batches`.

Trains a small linear projection on top of the base embedder — does NOT modify the base embedder weights.

**Why this matters:** you have your own contrastive training surface (LoRA-on-embedder in `crates/context-graph-embedding`) which is more powerful. But the **lightweight projection layer** that sits between the base embedder and HNSW search — that's a useful architectural pattern. It's training-data-cheap, doesn't risk catastrophic forgetting, and can be re-trained fast when the reward distribution shifts.

---

## L. SolverBandit — Thompson Sampling

**File:** `src/backends/rvf/SolverBandit.ts` (270 LOC)

Generic contextual multi-armed bandit:
```
selectArm(contextKey: string, armKeys: string[]): string
recordReward(contextKey, armKey, reward, cost?)
```

Per-arm state: `{ alpha, beta, pulls, totalReward, costEma }`. Sample from `Beta(α, β)`, subtract `costEma · costWeight`, take argmax. Unknown arms get `Math.random() + explorationBonus`.

**Serializable** — `getState() / loadState()` round-trip JSON.

**Use cases inside agentdb:**
1. Pick the **HNSW `efSearch`** value per context bucket (50/100/200/400).
2. Pick the **compression tier** for cold vectors (none/half/pq8/pq4/binary).
3. Pick the **skill** to compose with another (Thompson over `(taskType, skill)` pairs).
4. Pick the **RL algorithm** for a session (Thompson over `(taskType, algorithm)` pairs).

This is the **most universally portable primitive in the entire repo**. 270 lines, zero deps, JSON-serializable. Steal it verbatim; rewrite in Rust in an afternoon.

---

## M. RvfSolver three-loop adaptive policy

**File:** `src/backends/rvf/RvfSolver.ts` (312 LOC)
**Backed by:** `@ruvector/rvf-solver` v0.1.6 (132 KB WASM, no_std, pure CJS).

Already documented in `01_AGENTDB_ARCHITECTURE.md`. The killer feature is the A/B/C acceptance ablation that periodically re-validates that learning is actually helping (Mode C must beat Mode A and Mode B on accuracy AND cost AND noise-robustness).

The 73-byte SHAKE-256 witness chain is independently useful — it's a tiny, append-only, tamper-evident ledger of every "the policy made decision X" event.

---

## N. FederatedSessionManager — cross-session LoRA aggregation

**File:** `src/backends/rvf/FederatedSessionManager.ts` (526 LOC)

Pattern:
1. Each agent session has its own micro-LoRA + EWC++ state.
2. At session end, the session's LoRA delta is recorded.
3. The coordinator periodically averages high-quality LoRA deltas (quality threshold), produces a consolidated adapter, broadcasts.
4. Quality filtering: drop sessions below `minQuality`, cap at `maxAgents`.

**Why this matters less for ccreality:** you have one optimizer (Claude Code) and one inner actor (Qwen3.6-27B). No federation.

**Why this matters more for JEPA:** if you eventually run multiple parallel domain-pack training sessions (counter_world, gridworld, ME-JEPA recruiting corpus, more), federated aggregation of read-out-head deltas is a real story. Especially if you're training on the same RTX 5090 in alternation.

---

## O. MMRDiversityRanker

**File:** `src/controllers/MMRDiversityRanker.ts` (187 LOC)

```
MMR(Di) = argmax [ λ · sim(Di, Q) − (1−λ) · max_{Dj∈S} sim(Di, Dj) ]
```

Iterative greedy selection. λ default 0.5 (balanced), λ=1 is pure relevance, λ=0 is pure diversity.

**Why this matters:** for any "give me k related-but-not-redundant memories" query — exactly the situation when Claude Code is reading past failed attempts to plan a harness change. Don't show me 10 variants of "import error in test_units.py"; show me 1 import error + 1 type error + 1 missing-fixture + ... etc.

ContextGraph has nothing like MMR right now. Stealable as a single 187-LOC controller.

---

## P. ContextSynthesizer — narrative summary from k memories

**File:** `src/controllers/ContextSynthesizer.ts` (285 LOC)

Takes a list of `MemoryPattern` (essentially Episodes) and produces:
```typescript
{ summary, patterns, successRate, averageReward,
  recommendations, keyInsights, totalMemories }
```

Pure heuristics — counts, common-substring extraction, threshold-based "most failures had X" patterns. Not LLM-backed.

**Why this is interesting:** it's a fully-deterministic narrative generator. No model call, no context window concerns. The output shape is what every "summarize past attempts" tool wants. For ccreality, this is the right tool to summarize the last N attempts before a recommendation — fast, cheap, reproducible.

---

## Q. MetadataFilter — MongoDB-style query operators

**File:** `src/controllers/MetadataFilter.ts` (280 LOC)

10 operators: `$eq, $ne, $gt, $gte, $lt, $lte, $in, $nin, $contains, $exists`. Nested-path support. Compiles to either an in-memory predicate or a SQL `WHERE` clause.

**Why this is interesting:** for any retrieval query that has both vector similarity AND metadata constraints, this is the right surface. The conversion to SQL means you don't need to fetch-then-filter. ContextGraph has session-id filtering and tag filtering as ad-hoc post-filters; lifting them into a single declarative `$`-operator language is a UX win for tool callers.

---

## R. HNSWIndex (hnswlib-node wrapper) — *already covered by RuVector*

**File:** `src/controllers/HNSWIndex.ts` (581 LOC)

Lazy-loads `hnswlib-node`. Three metrics: cosine, l2, ip. Persistent index storage. Rebuild threshold for when too many updates degrade graph quality.

**Skip this** — ContextGraph's RuVector HNSW is more capable.

---

## S. SparsificationService + MincutService — graph algorithms

**Files:** `src/controllers/SparsificationService.ts` (492 LOC), `MincutService.ts` (434 LOC)

- **PPR (Personalized PageRank)** — Bahmani 2011 fast Map-Reduce variant.
- **Random walk sampling** — for top-k node importance.
- **Spectral sparsification** — Spielman-Srivastava effective resistances.
- **Degree-based** — baseline.
- **Mincut partitioning** — for sub-graph isolation.

WASM/NAPI bindings with JS fallback.

**Why this matters for JEPA more than ccreality:** ME-JEPA has the constellation/anti-pole compiler and the trajectory-leakage check. If those are graph-shaped (and they are), PPR for "which past episodes are most relevant to this trajectory" is a more principled scoring than cosine-only similarity. Mincut for "split this trajectory into independent training shards without leakage" is the right primitive.

---

## T. ReasoningBank — pattern store with optional GNN enhancement

**File:** `src/controllers/ReasoningBank.ts` (675 LOC)

Schema overlap with episodes — basically "reasoning patterns" are summarized successful approaches with `taskType`, `approach` text, `successRate`, `uses`, `avgReward`. Optional `LearningBackend.enhance(query, neighbors, weights)` wraps a query with GNN-aware refinement.

The 8-head GNN attention claim is from `@ruvector/gnn`. Not a self-contained agentdb capability — you'd be importing `@ruvector/gnn` to use it, which means a node-side dependency tree.

**Skip the GNN piece.** Steal the `(taskType, approach, successRate, uses, avgReward)` schema for storing harness "patterns" or JEPA "skills" if you don't want to model them as full Episodes.

---

## U. MerkleProof + ProvenanceSource pattern

**Conceptual unit, not a single class** — used inside `ExplainableRecall`.

```typescript
ProvenanceSource: { sourceType, sourceId, contentHash, parentHash,
                    derivedFrom, creator, metadata }
MerkleProof: { hash, position: 'left' | 'right' }
```

Every recall result carries:
- a content-hashed identifier per chunk,
- a Merkle root over the result set,
- a proof chain that lets a verifier re-build the root from any chunk's leaf hash.

**Why this is independently useful:** any artifact you produce that needs to be independently re-verifiable (training-run results, attempt evidence, FSV reports) can carry one of these and be verified offline by recomputing hashes. For ME-JEPA's Stage 0 Legal/Privacy gate, this is the right shape for the "approved data-use matrix" certificate.

---

## V. RVF / cognitive container format

**Backed by:** `@ruvector/rvf` 0.1.9 + `@ruvector/rvf-node` 0.1.7 + `@ruvector/rvf-wasm` 0.1.6.

Single-file format packing: vectors, HNSW index, sql tables, learning state, audit log.

**Skip this for ContextGraph** — your RocksDB-per-CF layout is more transparent. Possibly relevant for a JEPA *export* format (snapshot a trained model + its training-data manifest into one file you can ship), but only if you ever need to package a model for distribution.

---

## Summary table — what to steal, what to read, what to ignore

| Capability | Verdict | Effort to steal (Rust port) |
|------------|---------|-----------------------------|
| Episode schema | **Steal** | 0.5 day (already have most fields) |
| SkillLink relationships (4 types) | **Steal** | 0.5 day |
| CausalEdge `(uplift, confidence, sampleSize, mechanism)` | **Steal** | 1 day (extend existing causal CFs) |
| **CausalRecall α·sim+β·uplift−γ·cost reranker** | **Steal — high priority** | 1 day |
| **ExplainableRecall Merkle certs** | **Steal — high priority** | 2 days |
| NightlyLearner pipeline (5-step run) | Steal pattern, port loop | 3 days |
| LearningSystem 9-RL interface | Steal interface only | 1 day for interface, RL impls separate |
| AttentionService config schema | Read | n/a (you have candle) |
| SemanticQueryRouter intents | Steal | 1 day |
| TemporalCompressor 5-tier policy | Steal as refinement | 2 days |
| ContrastiveTrainer projection-layer pattern | Read | n/a (you have LoRA) |
| **SolverBandit (Thompson Sampling)** | **Steal — highest priority** | 0.5 day |
| RvfSolver 3-loop / SHAKE-256 witness chain | Steal witness-chain idea | 1 day |
| FederatedSessionManager | Defer (single-agent for now) | n/a |
| **MMRDiversityRanker** | **Steal** | 0.5 day |
| **ContextSynthesizer (deterministic narrative)** | **Steal** | 1 day |
| MetadataFilter ($eq/$gt/...) | Steal | 1 day |
| HNSWIndex | Skip (have RuVector) | n/a |
| SparsificationService PPR | Steal for JEPA | 2 days |
| MincutService | Steal for JEPA leakage check | 2 days |
| ReasoningBank | Skip schema; the rest is Episode-equivalent | n/a |
| `.rvf` format | Skip | n/a |

Total "high priority" steals: ~5 working days for Rust ports.
