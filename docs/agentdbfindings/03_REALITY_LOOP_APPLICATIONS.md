# 03 — Reality-Loop Applications (ccreality)

> **🔒 Reference-only.** Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md` — every transplant below is a **clean-room reimplementation**, not a vendor or path-dep. Effort estimates assume Rust ports against our own crate boundaries.

This is the action doc. Every section is a named, scoped transplant from agentdb to the ccreality outer-optimizer harness, with:

- the agentdb capability being borrowed (with file ref),
- the ccreality doc(s) it would change,
- the concrete Rust shape it would take in your workspace,
- the expected effect on resolved-instance throughput on the 300-instance SWE-bench Lite suite,
- effort + risk.

References to ccreality docs use the existing structure under `docs/ccreality/`. SMAP integration follows `CLAUDE.md §6`. Path discipline (§8) is honored throughout — none of these proposals touch `swebench/**`, `tests/oracle_**/**`, `crates/*/src/reward_specs/**`, or `crates/*/src/verifiers/**`.

---

## 1. Adopt the Reflexion Episode schema for `attempt-summary` records

**Borrowing:** `src/controllers/ReflexionMemory.ts:21-50` (Episode shape) + the 1391-LOC controller's retrieval API.

**Changes:**
- `crates/context-graph-cli/src/reality_loop/attempts.rs` — extend `AttemptSummary` to carry an explicit `critique: Option<String>` + `outcome_kind` enum.
- `docs/ccreality/13_STRUCTURED_OUTPUT_SCHEMA.md` — add `critique`, `success`, `outcome_kind` fields to `attempt-summary.json`.
- `docs/ccreality/03_MCP_REALITY_TOOLS_SPEC.md` — add a new MCP tool family that lets Claude Code query attempts as Reflexion-style episodes.

**Concrete schema (extend, don't replace):**
```rust
pub struct AttemptSummary {
    // existing fields preserved
    pub attempt_number: u32,
    pub status: AttemptStatus,
    pub error_code: Option<String>,
    pub patch_sha256: Option<String>,
    pub ledger_counts: LedgerCounts,
    pub score: Score,

    // NEW (mapped from agentdb's Episode)
    pub task_id: String,                    // SWE-bench instance id, == sessionId
    pub task_summary: String,               // = `task` field, used for embedding
    pub input_excerpt: Option<String>,      // = `input`, problem statement excerpt
    pub output_excerpt: Option<String>,     // = `output`, final agent message excerpt
    pub critique: Option<String>,           // = `critique`, structured failure analysis
    pub reward: f32,                        // = score.correctness, in [0,1]
    pub success: bool,                      // = score.correctness >= 1.0
    pub latency_ms: Option<u64>,
    pub tokens_used: Option<u64>,
    pub tags: Vec<String>,                  // ['lint-fail', 'import-error', 'flaky-test', ...]
}
```

**New MCP tools (proposed under `mcp__cgreality__attempts_*`):**

| Tool | Maps to agentdb | Purpose |
|------|------------------|---------|
| `attempts_query_reflexion` | `reflexion_retrieve` | k-NN over attempt summaries with `only_failures`, `only_successes`, `min_reward`, `time_window_days` filters |
| `attempts_critique_summary` | `agentdb_critique_summary` | "What failure patterns recur in the last N attempts on parked tasks?" |
| `attempts_success_strategies` | `agentdb_success_strategies` | "What worked across the last N successful attempts? Surface the common factors." |

**Effect on throughput:** today the optimizer reads attempts via `attempts_history_query`, which returns rows. Adding semantic retrieval over critiques + tags lets the optimizer answer "have we seen this exact failure mode before, and what did we change to fix it?" in a single tool call. Concretely: when a parked task gets unparked because a generic harness change shipped, the optimizer wants to find every other parked task with the same failure tag and unpark them too. Without this, you scan the queue manually.

**Effort:** 1 day for the schema extension, 1 day for the new MCP tools (read-only, no harness mutation).

**Risk:** low. This is additive to existing artifacts.

**Anti-overfit guard (CLAUDE.md §11.4):** the `tags` field must be GENERIC failure-mode tags (`import-error`, `type-mismatch`, `missing-fixture`, `flaky-test`, `oracle-mismatch`) — never task-specific. This is the same rule the optimizer already follows for harness changes.

---

## 2. Put the trigger / harness / next-prompt-delta decisions under Thompson Sampling control

**Borrowing:** `src/backends/rvf/SolverBandit.ts` (270 LOC) + `RvfSolver`'s 18-bucket context model.

**Currently in ccreality:**
- The trigger decision (`mcp__cgreality__reality_trigger_decision`) is rule-based.
- The choice of which `next_inner_prompt_delta` to ship is operator/optimizer judgment.
- The choice of `efSearch` for any vector search inside `ContextGraph` is a static config.

**Proposed:**

Wrap each repeated decision in a `SolverBandit` arm-set, log the resulting `correctness` as the reward, let Thompson Sampling pick the next arm.

| Decision point | Context bucket | Arms |
|----------------|----------------|------|
| Which generic harness "tactic" to apply next when diagnosing a failed attempt | `(failure_tag × repo_family × parked_count_bucket)` | `add-cargo-test-context`, `widen-search-radius`, `tighten-test-attribution`, `add-import-graph-context`, `expose-pyproject-toml`, `none` |
| Which `next_inner_prompt_delta` template to use | `(failure_tag × token_budget_bucket)` | a small set of 5-8 generic prompt-fragment templates — see ccreality `12_PROMPT_CONTRACT.md` |
| Which retrieval strategy to use when summarizing past attempts for the optimizer | `(prompt_phase × attempts_so_far)` | `e1_only`, `multi_space`, `pipeline`, `embedder_first` |
| Which `efSearch` to use for HNSW retrieval inside the optimizer | `(retrieval_strategy × corpus_size_bucket)` | `50, 100, 200, 400` |

**Concrete Rust surface:**
```rust
pub struct SolverBandit {
    contexts: HashMap<String, HashMap<String, ArmStats>>,
    cost_weight: f32,
    cost_decay: f32,
    exploration_bonus: f32,
}

impl SolverBandit {
    pub fn select_arm(&self, ctx: &str, arms: &[&str]) -> String;
    pub fn record_reward(&mut self, ctx: &str, arm: &str, reward: f32, cost: Option<f32>);
    pub fn save(&self) -> Vec<u8>;          // serde_json
    pub fn load(state: &[u8]) -> Result<Self>;
}
```

Persist to a new RocksDB CF `CF_OPTIMIZER_BANDIT`. Each `optimizer_record_decision` MCP call updates the bandit; each `optimizer_record_recommendation` reads the chosen arm out of the bandit.

**Effect on throughput:** **the most leveraged change in this whole document.** The optimizer is currently making 4-6 categorical choices per turn from generic priors (Claude reasons fresh each time about which tactic is most likely to help). With Thompson Sampling, those choices become *learned* — the harness improves itself across turns, not just within a turn. Per CLAUDE.md §11.13 you already have the tooling to record per-turn arms; what's missing is a policy that picks them.

**Anti-overfit guard:** every arm in every bandit MUST be a generic action that applies across all 300 instances. Per CLAUDE.md §11.4: if "tighten-test-attribution" only helps astropy tasks, it's wrong; reformulate. The bandit will *correctly* under-pull arms that don't generalize, because their reward EMA stays low.

**Effort:** 0.5 day for the SolverBandit Rust port, 1 day to wire it into the four decision points, 0.5 day for persistence.

**Risk:** medium — there's real reasoning required in choosing the context buckets. Get them wrong (too coarse → arms don't differentiate; too fine → never enough samples) and the bandit is uniformly random. The agentdb 18-bucket model is a useful prior; mimic the structure.

**Read for context:** `docs/ccreality/15_MODEL_POLICY.md` (the optimizer is opus and decides; the bandit lets opus *learn from prior decisions* across sessions without retraining).

---

## 3. CausalRecall utility-reranker for "which past recommendation should I revisit on this parked task?"

**Borrowing:** `src/controllers/CausalRecall.ts:90-140` (the α·sim + β·uplift − γ·cost formula).

**Use case:** when the optimizer's diagnostician returns "we've seen this exact failure 3 times before, all parked," the optimizer wants to re-read the highest-utility past recommendation — not just the most semantically similar one.

**Signal definitions:**
- `similarity` — embedding cosine between the current failure's `critique + tags` and a past recommendation's `body`.
- `uplift` — `(success_rate_after_recommendation_shipped − success_rate_before)`, computed per recommendation. Stored on the recommendation row.
- `latencyCost` — `wall_clock_seconds_to_apply_recommendation` (cargo build time + verification time).

**Rerank formula (extend `optimizer_record_recommendation` reading API):**
```
utility = 0.7 * similarity_to_current_failure
        + 0.2 * recommendation.uplift_against_baseline
        - 0.1 * cost_to_reapply
```

α/β/γ themselves are bandit arms (per §2 above).

**Changes:**
- New MCP tool: `mcp__cgreality__optimizer_recall_recommendations(failure_summary, k=5)` — runs the reranker, returns top-k past recommendations sorted by utility, each with a Merkle-style provenance certificate (see §4).
- `docs/ccreality/03_MCP_REALITY_TOOLS_SPEC.md` — document the new analyze tool.

**Effect on throughput:** dramatic on parked tasks. Today, the diagnostician proposes a fix from scratch each time a parked task is re-touched. With this, the diagnostician proposes "re-apply recommendation #34, which had +0.18 uplift on similar failures last month, at cost ~5 min."

**Effort:** 1 day (uplift is the only new field; everything else is existing data).

**Risk:** low. The rerank is read-only.

---

## 4. Merkle provenance certificates on every optimizer recommendation

**Borrowing:** `src/controllers/ExplainableRecall.ts:37-90` (RecallCertificate + MerkleProof + ProvenanceSource).

**The certificate is the bridge between the recommendation and FSV.** Today, `optimizer_record_recommendation` writes a JSON artifact citing the source-of-truth files it consulted. There's no offline way to verify the recommendation was made against *those exact bytes* of those files. A future drift detection (or operator audit) has to trust the citation.

A Merkle proof chain over `(file_path, content_sha256)` pairs at recommendation time gives that verification for free.

**Proposed certificate shape (Rust):**
```rust
pub struct RecommendationCertificate {
    pub id: Uuid,
    pub recommendation_id: String,
    pub turn_number: u32,

    // What was consulted
    pub source_files: Vec<SourceRef>,           // (path, content_sha256)
    pub artifacts_consulted: Vec<ArtifactRef>,  // attempt-summary, reality-signal, etc.

    // Merkle proof
    pub merkle_root: [u8; 32],
    pub proof_chain: Vec<MerkleProof>,          // for each leaf

    // Justification
    pub minimal_why: Vec<String>,               // minimum subset of evidence_packet ids
                                                // that justify the recommendation
    pub redundancy_ratio: f32,                  // len(consulted) / len(minimal_why)
    pub completeness_score: f32,                // fraction of declared remaining_risks addressed

    // Anti-tamper
    pub witness_chain_offset: u64,              // SHAKE-256 chain offset (see §5)

    pub created_at: i64,
    pub access_level: AccessLevel,              // matches agentdb's enum
}
```

**MCP additions:**
- `optimizer_record_recommendation` already returns `recommendation_id`; add `certificate_id` to the response.
- New tool: `mcp__cgreality__optimizer_verify_certificate(certificate_id)` — recomputes the Merkle root by re-reading the cited files and re-hashing them. Returns `{valid, drifted_paths: [...]}`. If files have moved or changed since the recommendation was made, you'll know.

**Effect on FSV:** the existing FSV protocol (`docs/ccreality/18_FSV_PLAN.md`) wants every persisted decision to be independently re-verifiable. Today, recommendation re-verification means "re-read the JSON, re-read the cited files, hope they match what was cited." With certificates, a single tool call validates or invalidates.

**Anti-overfit guard:** none needed. Certificates are pure cryptography over already-existing data.

**Effort:** 2 days (Merkle tree code is small but FSV-grade testing is non-trivial). 

**Risk:** low. Independent of correctness — even if everything else fails, the certificate is just additional metadata.

---

## 5. SHAKE-256 witness chain for the optimizer's own decision stream

**Borrowing:** `RvfSolver`'s `73-byte/entry` SHAKE-256 chain (`src/backends/rvf/RvfSolver.ts` + the witness-chain accessor in `SelfLearningRvfBackend`).

**Today:** `optimizer_record_recommendation` and `optimizer_record_decision` write JSON artifacts. The artifacts themselves can be tampered with (rewriting a JSON file leaves no trace).

**Proposed:** a single append-only SHAKE-256 chain at `<runtime_root>/claude-code-optimizer/witness-chain.bin`. Every recorded decision/recommendation appends a 73-byte entry: `(timestamp, op_type, content_hash, prev_hash, signature)`. The chain is rebuildable but not editable — to alter it you'd have to rebuild every subsequent entry.

**Schema:**
```
struct WitnessEntry {
    timestamp: u64,            // 8B
    op_type: [u8; 1],          // 1B  (0=decision, 1=recommendation, 2=harness_transition)
    content_sha256: [u8; 32],  // 32B
    prev_chain_hash: [u8; 32], // 32B
}
// total: 73B per entry, exactly matching agentdb's 73-byte stride
```

**MCP additions:**
- `optimizer_witness_chain_verify` — replay the chain, recompute hashes, return `{valid, entries: u64, last_op: ...}`. Reuses agentdb's structural check pattern: `chain.len() % 73 == 0 && every prev_chain_hash matches`.
- `optimizer_witness_chain_diff(since_offset)` — list new entries since a given offset, useful for the readback skill.

**Effect on cohesion with existing audit log:** ContextGraph already has an audit-log infrastructure (`audit-log-architect` agent's CF_AUDIT_LOG). The witness chain is a *cryptographic* layer on top — the audit log records "what happened," the witness chain records "and here's a hash chain proving the sequence wasn't reordered." Same way modern git commits sign every commit's tree: redundant in the happy path, indispensable when something goes weird.

**Effort:** 1 day for the chain code + Rust port, 0.5 day for the verifier MCP tool.

**Risk:** low. Independent of everything else.

---

## 6. NightlyLearner-style "every N turns, propose harness causal edges" pipeline

**Borrowing:** `src/controllers/NightlyLearner.ts:98-140` + the doubly-robust uplift estimator.

**Currently:** there's no explicit periodic consolidation. Each turn diagnoses one attempt; whether a pattern recurs is implicit in the optimizer's working memory.

**Proposed:** a new MCP tool family `mcp__cgreality__optimizer_consolidate` that runs the NightlyLearner pipeline against the last N attempts:

1. **Discover** failure patterns: cluster the last N `attempt-summary.tags` + `critique` embeddings; surface clusters with ≥3 members. For each cluster, propose a (failure_pattern → candidate_harness_fix) hypothesis.
2. **Mark candidate experiments**: tag the next M attempts of those tasks as "treatment" (apply candidate fix) vs "control" (don't), and let the inner actor run.
3. **Calculate uplift** with doubly-robust estimator over outcomes:
   ```
   τ̂ = E[reward | treatment] − E[reward | control]
       + correction_terms
   ```
4. **Prune low-confidence hypotheses** below `confidenceThreshold` (default 0.6 from agentdb).
5. **Update bandit priors** in §2's SolverBandit for arms that survived.

**Cadence:** kicked off automatically every 25 attempts (configurable). Runs as a background task; doesn't block the per-turn loop.

**Anti-overfit guard:** every candidate fix must be expressible as a generic harness change (per CLAUDE.md §11.4). The pipeline rejects task-specific fixes at step 1.

**Changes:**
- `crates/context-graph-cli/src/reality_loop/consolidate.rs` — new module.
- `docs/ccreality/03_MCP_REALITY_TOOLS_SPEC.md` — add the new tool.
- `docs/ccreality/14_TRIGGER_AND_INVOCATION_POLICY.md` — add a periodic trigger.

**Effect on throughput:** the optimizer goes from "react to one failure at a time" to "every 25 attempts, surface the dominant failure pattern + propose a fix + A/B test it." This is precisely the meta-harness loop in CLAUDE.md §11.

**Effort:** 3 days. The doubly-robust estimator is genuinely subtle; lean on a battle-tested implementation if available.

**Risk:** medium. The hypothesis generation step needs strong filters — surface clusters of 3+ failures (not 1+), require generic phrasing, run dry-run first.

---

## 7. MMR diversity reranker for "show me 5 different past failures" queries

**Borrowing:** `src/controllers/MMRDiversityRanker.ts` (187 LOC).

**Use case:** when the diagnostician asks "what are the 5 most relevant past failures?" — today the answer is the top-5 cosine matches, which can be 5 variants of the same failure. With MMR (`λ=0.5`), the answer is 5 *diverse* recent failures that span the failure space.

**Concrete change:** extend `mcp__cgreality__attempts_query_reflexion` with a `diversity_lambda: f32 = 0.5` parameter. When `diversity_lambda < 1.0`, run MMR over the top-2k candidates and return diverse top-k.

**Effect on diagnosis quality:** the optimizer makes better generalization decisions when it sees variance, not redundancy.

**Effort:** 0.5 day. The MMR algorithm is a 50-line greedy.

**Risk:** trivial.

---

## 8. ContextSynthesizer-style deterministic "summarize last N attempts" tool

**Borrowing:** `src/controllers/ContextSynthesizer.ts` (285 LOC, *no model calls*).

**Use case:** at the start of every turn (per CLAUDE.md §11.8), the optimizer reads recent reality. Today it reads raw artifacts. With ContextSynthesizer, it gets a structured `{summary, patterns, successRate, averageReward, recommendations, keyInsights}` object — fully deterministic, no LLM call needed.

**Why deterministic matters here:** the SessionStart hook injects state. If that state goes through an LLM summarizer, the summary is non-reproducible across turns. ContextSynthesizer gives a hash-stable summary.

**Concrete change:**
- New MCP tool: `mcp__cgreality__attempts_synthesize(last_n: u32)`.
- Return shape mirrors agentdb's `SynthesizedContext`.

**Effect:** one tool call replaces 3-5 raw-artifact reads at turn start. Faster, cheaper, more uniform across turns.

**Effort:** 1 day. The heuristics are pure Rust string ops.

**Risk:** trivial.

---

## 9. MetadataFilter $-operator query language for attempt searches

**Borrowing:** `src/controllers/MetadataFilter.ts` (280 LOC, MongoDB-style).

**Use case:** the existing `attempts_history_query` has fixed parameters. With $-operators, callers can express:
```json
{
  "tags": { "$contains": "import-error" },
  "score.correctness": { "$lt": 0.5 },
  "ts": { "$gt": 1730000000 },
  "repo_family": { "$in": ["astropy", "django"] }
}
```

**Effect:** richer queries without expanding the tool schema. The optimizer can answer "show me all parked astropy tasks with import errors in the last 7 days, where the inner actor's score was below 0.5" in one tool call.

**Effort:** 1 day. The compiler from $-ops to RocksDB seek+filter is straightforward; SQL compilation is harder if needed.

**Risk:** low. Read-only.

---

## 10. Optional: federated-style aggregation across parallel optimizer sessions

**Borrowing:** `src/backends/rvf/FederatedSessionManager.ts` (526 LOC).

**Today:** one Claude Code session, one optimizer.

**Possible future:** if you ever run *multiple* Claude Code sessions in parallel (unattended overnight runs from different shells, per `docs/ccreality/14_TRIGGER_AND_INVOCATION_POLICY.md`), federated SolverBandit aggregation lets each session contribute reward signals to a shared bandit.

**Skip until you actually parallelize.** Listed for completeness.

---

## Priority order — pick from the top

If you ship one transplant, ship **§2 (SolverBandit + 4 decision points)**. It's the highest-leverage change because it makes the optimizer self-improving across turns — what every other §-borrowing builds on.

If you ship two, add **§4 (Merkle certificates)**. Independent of §2, dramatically improves FSV.

If you ship three, add **§1 (Reflexion episode schema)**. Unlocks §3, §6, §7, §8.

If you ship four, add **§3 (CausalRecall utility reranker)**. The α·sim+β·uplift−γ·cost primitive is small and you'll want it eventually.

The remainder (§5, §6, §7, §8, §9) are individual quality-of-life wins that can come in any order, but §6 (NightlyLearner) is the deepest integration — leave it for last.

---

## Cross-references

- ccreality §11 (CLAUDE.md) — the optimizer role this all extends.
- `docs/ccreality/03_MCP_REALITY_TOOLS_SPEC.md` — the spec these new tools fit into.
- `docs/ccreality/13_STRUCTURED_OUTPUT_SCHEMA.md` — the schema files these proposals extend.
- `docs/agentdbfindings/05_QUICK_WINS_VS_DEEP_INTEGRATION.md` — effort triage of these proposals.
- `docs/agentdbfindings/06_RISKS_AND_NON_TRANSFERABLE.md` — what NOT to take.
