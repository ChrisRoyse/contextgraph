# 06 — Risks & Non-Transferable Pieces

> **🔒 Reference-only.** Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md` — the License-risk discussion below is **moot for code copying** because we never copy code. Provenance comments citing the inspirational source file + commit are still required when reimplementing a borrowed idea.

The honest list of what NOT to take from agentdb, what's risky if taken, and what looks tempting but isn't actually a fit. This is the "sanity check before committing" doc.

---

## Things that look stealable but aren't

### A. The `.rvf` cognitive container format

**What it is:** a single-file format packing vectors + HNSW index + sql tables + learning state + audit log. Backed by `@ruvector/rvf` 0.1.9.

**Why it looks tempting:** "single-file ML state" is genuinely useful for distribution. Drop a `.rvf` somewhere, anyone can load it.

**Why don't take it:**

1. **Your storage substrate is RocksDB-per-CF, not SQLite.** A `.rvf` adapter for RocksDB would be a parallel persistence story — which means two storage paths, twice the FSV surface, twice the corruption modes. CLAUDE.md §14 ("Simplicity first; minimal impact") rules this out.
2. **It hides what FSV needs to inspect.** Your FSV protocol (`docs/ccreality/18_FSV_PLAN.md`, `backupdocs/5090jepa/14_full_state_verification_protocol.md`) is built around "open the CF, read the row, recompute the hash." Packing everything into one binary blob is the *opposite* of what FSV wants.
3. **The format isn't actually documented.** `@ruvector/rvf-wasm` 0.1.6 ships a 132 KB WASM blob and the README. There's no spec. Adopting an undocumented binary format on the critical path is exactly the kind of single-source-of-truth dependency CLAUDE.md §11 warns against.

**If you ever want single-file packaging:** roll your own from RocksDB's existing snapshot/checkpoint mechanism + a tar file + a manifest JSON. You already have all the parts.

---

### B. The 9 RL algorithm implementations

**What it is:** Q-Learning, SARSA, DQN, Policy Gradient, Actor-Critic, PPO, Decision Transformer, MCTS, Model-Based RL — all in `src/controllers/LearningSystem.ts`.

**Why it looks tempting:** "9 algos in one file" sounds comprehensive.

**Why don't take it:**

1. They're toy implementations. Q-Learning is a `Map<(state, action), qValue>`. DQN is a small MLP with a replay buffer. Decision Transformer is a typed wrapper over an MLP. None of these are competitive with stable-baselines3 / Tianshou / cleanRL.
2. The RL surface area is genuinely deep — getting any of these right (correct discounting, target-network synchronization, advantage estimation, PPO clipping) is its own project.
3. **You don't currently use online RL.** Your reality-loop optimizer is opus-the-LLM doing reasoning + a planned SolverBandit (per `03 §2`). DynamicJEPA's planner is going to be a JEPA-predictor argmax with optional bandit. Neither needs DQN.

**If you ever need real RL:** `tch-rs` or `burn` or candle. Don't port from agentdb's TS implementations.

---

### C. AttentionService Flash/Hyperbolic/MoE/Linear via `@ruvector/attention`

**Why it looks tempting:** the configuration schema is nice and the name-list is impressive.

**Why don't take it:**

1. **No CUDA path.** `@ruvector/attention` is NAPI + WASM. Your RTX 5090 with sm_120 needs candle CUDA kernels. agentdb's attention runs on CPU/SIMD or in browsers — neither helps you.
2. **Your candle integration is already working.** `Cargo.toml` confirms `candle-core 0.10.2` with `cuda` feature, sm_120a kernels. Bringing in NAPI `@ruvector/attention` would be a strict regression.
3. The HyperbolicAttention claim is a research-grade feature flag. It's gated `default: false` in agentdb's own code. Read the paper, not the code.

**Steal:** the configuration schema (`AttentionConfig`) is a good forcing function for what `dynamicjepa::attention::config` should look like.

---

### D. ReasoningBank as a parallel-to-Episodes schema

**Why it looks tempting:** "patterns" sound like a different thing from "episodes."

**Why don't take it:** they're not different. A ReasoningPattern is `{taskType, approach, successRate, uses, avgReward, embedding}`. An Episode is `{task, input, output, critique, reward, success, embedding}`. Maintaining two parallel CFs for substantially the same data is a maintenance trap. Pick one schema (Episodes is cleaner) and use it for both.

---

### E. The 8-head GNN attention claim ("+12.4% recall")

**Why it looks tempting:** numbers in a README.

**Why don't take it:**

1. The claim sources to `benchmarks/attention-performance.ts` in agentdb — but that benchmark uses MiniLM-encoded synthetic data and a single corpus. Whether the 12.4% lift transfers to your 13-embedder fusion + your real corpora is unknown.
2. The implementation lives in `@ruvector/gnn` (NAPI). Same dependency-tree concern as C.
3. ContextGraph's existing E8 graph embedder + RuVector HNSW + RRF fusion is a different story; head-to-head benchmarking would be the right experiment, not a copy-paste of the +12.4% claim.

---

## Risks if you DO take recommended transplants

### F. SolverBandit context-bucket design (covered in `03 §2`, `04 §2`)

**The trap:** if buckets are too coarse, arms don't differentiate; if too fine, no arm gets enough samples to converge.

**Mitigation:** start with 4-6 buckets per decision point; raise log granularity if mean Beta variance stays high after 100 samples. Telemetry the Beta variance per (context, arm) and surface in `./memory/journal/` weekly.

**Specific risk for ccreality:** if you bucket by `(failure_tag × repo_family)`, the 300 SWE-bench Lite tasks span ~10 repos and ~5 dominant failure tags = 50 buckets. With 6 arms each = 300 (context, arm) cells. With a 300-task suite running once per turn, you're running ~150 turns to get 1 sample per cell. That's on the edge of useful. Coarser buckets might be necessary.

---

### G. NightlyLearner doubly-robust uplift estimator

**The trap:** doubly-robust estimation is *very* sensitive to extreme propensity scores `e(x)`. Near 0 or 1, the variance blows up.

**Mitigation:** clip propensities to `[0.05, 0.95]` (this is what agentdb does and it's standard). Log unclipped propensities so you can see if your model is over-extreme.

**Specific risk for ccreality:** with only ~300 attempts in the suite, statistical power is low. A τ̂ estimate with confidence < 0.6 should never trigger an arm-change.

**Specific risk for JEPA skill promotion:** outcome models μ₀, μ₁ need real cross-validation. A skill that "appears" to lift but came from 5 trajectories isn't ready to promote.

---

### H. Merkle certificates over file content

**The trap:** if a file's content drifts (formatter run, comment edit, line ending change), the certificate goes invalid even though semantically nothing changed.

**Mitigation:** decide what counts as "changed." For ccreality recommendations, semantic equality of source-of-truth files is what matters. Either:
- hash post-`rustfmt`/post-normalization, OR
- accept that the certificate is byte-identity (any whitespace change invalidates) and rely on FSV to surface drift.

The latter is correct for ccreality — the optimizer cited *exact bytes*, and a drift means the citation is now stale.

For JEPA, the bytes-of-`model.safetensors` are what matters; whitespace is a non-issue.

---

### I. The 5-tier TemporalCompressor

**The trap:** decompressing PQ-4 or binary tiers loses information. A JEPA latent is a vector — re-compressing then re-decompressing is fine for HNSW search but not for downstream training.

**Mitigation:** for JEPA panel storage, never train against decompressed tiers. Compression is for *retrieval*, not for *training data*. Keep an uncompressed copy of training tensors out-of-band.

---

### J. The 73-byte SHAKE-256 witness chain

**Possible trap:** all proposals say "73 bytes/entry, exactly matching agentdb's stride." That's *agentdb's* layout: `(timestamp 8 + op_type 1 + content_hash 32 + prev_hash 32) = 73`. If you change any field width, you must change the verifier check from `chain.len() % 73 == 0` to your new width. Don't accidentally inherit "73" as a magic number.

**Mitigation:** define `WITNESS_ENTRY_SIZE: usize` once, at the head of `witness_chain.rs`. Every check uses the constant.

---

## License risk

agentdb is **MIT OR Apache-2.0**. ContextGraph is whatever-you-decided. MIT/Apache is permissive — copying ideas + clean-room Rust ports is fine. *Verbatim code copy* still needs attribution per the license terms.

If you port code from agentdb verbatim:
- include a header comment citing `https://github.com/ruvnet/agentdb/blob/<sha>/<path>` and the LICENSE file in agentdb's repo,
- keep the MIT/Apache license headers if any exist,
- prefer Rust *idiomatic rewrites* over line-for-line transliteration. Rewrites are cleaner code and reduce the attribution burden.

The `@ruvector/*` optional deps are separately licensed — most appear MIT, some don't ship a LICENSE in their tarball. **Don't depend on `@ruvector/*` packages from a Rust workspace.** They're TypeScript-first; their CI doesn't test against `cargo build`. If you import their algorithms, port them.

---

## Architectural mismatches

### K. TypeScript / Node assumptions

agentdb assumes a Node event loop:
- `Promise<T>`-based async everywhere,
- `tickIntervalMs` for background heartbeats,
- `tokio`-style timer cancellation isn't trivially mappable to `setInterval`.

When porting to Rust:
- async cancellation through `tokio::select!` + `CancellationToken` instead of `clearInterval`.
- Background ticks via `tokio::spawn` + `tokio::time::interval` (not just `setInterval`).
- Rust lifetimes will refuse to compile some patterns that TS lets you do (e.g. capturing `&mut self` in a long-lived closure). This is mostly an asset — it surfaces design issues.

---

### L. SQLite-centric data layout

agentdb uses sql.js (or better-sqlite3). All its `JOIN`s and `WHERE`s are SQL. ContextGraph uses RocksDB.

**This bites in two places:**

1. **MetadataFilter's "compile to SQL" path is wrong for you.** You want "compile to RocksDB seek + filter." The shape of the `$`-operator is portable; the compiler isn't.
2. **Causal experiment treatment/control selection** is a SQL `WHERE tag IN (...)`. In RocksDB land, you need either a secondary index keyed on tag, or a full scan with filter. Decide upfront.

---

### M. The "graceful fallback" pattern

agentdb's `optionalDependencies` + try/catch-around-imports pattern is *not* compatible with the no-fallback rule (`backupdocs/5090jepa/00_TASK.md` R2: "No fallback. Fail fast. Log loudly").

When porting, **delete the fallback paths**. If the native dep isn't available, return a structured error. agentdb's "fall back to JS implementation" pattern hides bugs.

Concretely: when porting `SemanticQueryRouter`'s "fallback to brute-force search if `@ruvector/router` is unavailable" — drop the fallback. Either you have the equivalent (RuVector) or you don't, and if you don't, the build fails.

---

## Things you should NOT lift verbatim from the docs

### N. "+36% search quality from feedback" claim (README L19)

The number sources to `benchmarks/ruvector-performance.test.ts`. It's a synthetic benchmark with their own choice of "quality" metric. Don't quote it as if it generalizes — for SWE-bench Lite, "quality" is `resolved_ids` count, period. agentdb has no SWE-bench data.

### O. "150x faster than SQLite" claim (README L106)

Compares HNSW + SIMD against a SQLite cosine-loop, which is the wrong baseline (no one writes vector search as a SQLite cosine loop in production). Don't quote.

### P. Roadmap items that haven't shipped

`CHANGELOG.md` shows v1.7.0 (QUIC) and v2.0.0 (HNSW + Hybrid + Quantization + Multi-DB Coord) as "Planned/Future." But `package.json` says version `3.0.0-alpha.14` and HNSW + Hybrid + Quantization clearly DID ship — the CHANGELOG is just stale. Don't trust the roadmap section as a status signal; trust the source tree.

---

## Don't-port summary

| Item | Reason |
|------|--------|
| `.rvf` single-file format | Mismatch with RocksDB FSV |
| 9 RL toy algorithms | Use real RL libraries when needed |
| `@ruvector/attention` Flash/Hyperbolic/MoE | No CUDA / sm_120 path |
| `@ruvector/gnn` 8-head attention | Same — port the idea via candle, don't import |
| ReasoningBank parallel schema | Episodes already cover it |
| HNSWIndex (hnswlib-node) | RuVector is more capable |
| `+36%` / `150x` perf claims | Synthetic, non-generalizable |
| Graceful-fallback patterns | Conflicts with no-fallback rule |
| MetadataFilter SQL compile path | Port the `$`-operators, not the SQL compiler |
| FederatedSessionManager | Single-agent today; defer |

---

## Watch list

If you adopt Tier 1+2 transplants per `05_QUICK_WINS_VS_DEEP_INTEGRATION.md`, the things that will earliest tell you something is wrong:

- **Bandit win-rate ratio < 1.2 vs uniform-random** at 100 turns → context buckets are mis-designed (re-tune per `06 §F`).
- **Witness-chain verifier reports `valid: false`** on a fresh run → either the chain was reordered (a real bug) or your `prev_chain_hash` computation diverges from agentdb's (a port bug). Diff against `src/backends/rvf/SelfLearningRvfBackend.ts:`325-330.
- **MMR returning fewer than `k` results** when `lambda < 1.0` → input candidate pool too small; ensure you fetch ≥ 2k candidates before MMR-ing.
- **NightlyLearner promoting > 5 hypotheses per cycle** → confidence threshold too loose. Raise from 0.6 to 0.75.

Each of these failure modes is fast to detect with the right telemetry. Plan the telemetry up front.
