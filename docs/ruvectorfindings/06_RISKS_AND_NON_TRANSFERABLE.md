# 06 — Risks & Non-Transferable Pieces

> **🔒 LOCKED 2026-05-08 — Clean-room reimplementation only.** Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md` — the Vendor vs path-dep section that originally lived here is OBSOLETE. We do neither. The replacement section ("Clean-room reimplementation discipline") lays out the actual policy.

> **🎯 Master plan:** [`09_END_GOAL_REVIEW_REPLACEMENT.md`](./09_END_GOAL_REVIEW_REPLACEMENT.md) §8 ("Open Research Questions") is the canonical risk register for the code-domain pack — generalization across repos, panel dimensionality adequacy, per-language vs unified models, mutation-corpus diversity, novel-API OOD behavior, λ_ewc tuning, private-codebase generalization, cross-cycle disagreement. The risks below are about the *clean-room reimplementation discipline itself* (don't copy code, don't vendor, etc.); doc 09 §8 is about the *system's empirical risks*. Both apply. **Doc 09 §3.5–§3.8 introduces additional empirical risks** that belong in the risk register: (i) auxiliary-head α-weight mis-tuning (over-weighting `α_runtime` could distract the predictor from oracle pass/fail; under-weighting `α_overrides` wastes gold-tier supervision); (ii) shift-log subscriber falling behind during heavy edit bursts (lag > 500 shifts triggers alert but predictions stale until catch-up); (iii) instrument-cache thrashing on rapid file churn (4 GB LRU may not be enough for hot repos); (iv) determinism-flag false negatives on semi-deterministic signals (walltime drift under contention); (v) operator-override schema drift (capturing reasons as free text loses signal vs. structured categorical labels); (vi) over-collection storage bloat (each new instrument adds ~5-10% to per-attempt panel storage). When this doc disagrees with doc 09, doc 09 wins.

The honest list. Clean-room reimplementation discipline, license-comment requirements, things that look stealable from RuVector but aren't actually a fit, and failure modes once we ship.

---

## Current implementation state (updated 2026-05-08, post commits c18c694e → a9ca688e)

The clean-room policy has been applied to:

- `crates/context-graph-witness/` — witness-chain core + canonical format manifest + legacy reconciliation + repair tooling + atomic durability primitives. Wired into ccreality optimizer files, DynamicJEPA audit witness rows, and per-write preflight.
- `crates/context-graph-solver/` — weighted Forward-Push PPR + Conjugate Gradient + CSR validation. Provenance headers on every Rust file.
- **`optimizer_compute_influence` MCP tool** (c18c694e) — Forward-Push PPR over attempt-influence graph; persists audit row; preflights chain integrity before write.
- **MMR diversity rerank** (`diversity.rs`, f6ffeaeb) — wired into `optimizer_recall_recommendations` with witnessed recall audits via `recommendation_certificate.rs`.
- **Witness-chain preflight on every write** (bfcc060e) — bandit / influence / optimizer writes verify chain integrity before appending; `optimizer_preflight_fsv.rs` covers the FSV.

**Live operational concern observed 2026-05-08:** the runtime root's `claude-code-optimizer/witness-chain.bin` has surfaced `CCREALITY_WITNESS_PREV_HASH_MISMATCH` at offset 0 (actual `prev_hash` is non-zero where the canonical first entry must be all-zero). This indicates either pre-canonical-format chain bytes still on disk OR a partial-write recovery scenario. The shipped repair tooling (`witness_chain_repair.rs` + `witness_chain_repair_fsv.rs`) is the remediation path; legacy reconciliation (`witness_chain_legacy.rs`) is the migration path. Surface in operator notes before the next reality-loop attempt.

Remaining policy-sensitive work:

- Do not add BMSSP by wrapping CG and calling it multigrid. It must be a real clean-room V-cycle implementation or remain explicitly pending.
- ME-JEPA-Code's instruments / panel materializer / predictor / mutation-corpus generator / aux heads / `mejepa-shift-subscriber` (doc 09 §3.5–§3.8) — every clean-room file MUST start with the standard provenance header citing the inspirational RuVector source.
- Keep checking for forbidden `agentdb`/`ruvector` path deps and npm deps before each merge.

---

## Clean-room reimplementation discipline (the only path)

The locked rule says we never vendor and never path-dep. That leaves clean-room reimplementation: read the RuVector source as inspiration, then write our own implementation in our own crate. Discipline:

### What "clean-room" means here

- **No copy-paste.** Not a single line of RuVector source ends up in our tree. If you find yourself wanting to copy, stop and re-read the code until you understand the algorithm well enough to write it from scratch.
- **Our own naming.** Their `WitnessEntry` becomes our `WitnessEntry` (same name is fine if it describes the same concept), but our field types are ours, our error variants are ours, our crate structure is ours.
- **Our own dependencies.** They use `sha3` from crates.io? Fine — we use `sha3` too, that's a public Rust crate, not part of the locked rule. They use `@ruvector/sha3-internal`? We don't.
- **Our own tests.** Don't copy their tests. Write tests that match our use cases.
- **Our own error types.** Don't copy their `Result` aliases or error enums.

### Provenance comments are required

Every file in our tree that's inspired by a RuVector file MUST start with a header comment in this form:

```rust
// Inspired by ruvnet/RuVector crates/<crate>/src/<file>.rs at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.
```

This is **provenance only** — it cites the inspiration so future agents can find the original. It is NOT a license attribution, because no code was copied.

### Where to put the new crates

Default placement:

| Capability | New crate or module |
|------------|---------------------|
| Witness chain + signatures | new crate `crates/context-graph-witness/` |
| Sublinear solvers (PPR, BMSSP, CG) | `crates/context-graph-solver/` exists with PPR + CG; BMSSP pending |
| 8/7/5/3-bit tier compression | new module under existing quantization crate |
| MMR diversity rerank | new module under existing retrieval crate |
| Training-free retrieval LM + diffuser | new crate `crates/context-graph-retrieval-diffusion/` |
| Conformal prediction | new module under existing retrieval crate |
| SONA-style adaptive learner | new crate `crates/context-graph-sona/` |
| Verified training certificates | new crate `crates/context-graph-verified-training/` |
| Stoer-Wagner mincut | new crate `crates/context-graph-mincut/` |
| Granger causality test | new module under existing analysis crate |

Crate naming convention: `context-graph-*`. No `ruv*` or `rvf*` prefixes in our tree.

### Tests that the policy is being followed

Before any PR that adds code under `crates/context-graph-*`, run:

```bash
# No path-dep on agentdb/ or ruvector/
! grep -rn 'path.*=.*"\.\./\(agentdb\|ruvector\)' Cargo.toml crates/*/Cargo.toml

# No vendored ruvector/agentdb files
! find crates -name '*.rs' -exec grep -l '@ruvector\|ruvnet/agentdb' {} \;

# No npm deps from the ruvnet ecosystem
! grep -E '"(agentdb|ruvector|@ruvector/)' package.json

# Every clean-room file has a provenance header
for f in $(find crates/context-graph-{witness,solver,sona,...} -name '*.rs'); do
    head -1 "$f" | grep -q 'Inspired by ruvnet/RuVector' || echo "MISSING PROVENANCE: $f"
done
```

If any check fails, the change violates this locked decision.

---

## License considerations

RuVector is **MIT** per `LICENSE`. Since we never copy code, we never redistribute their code, and the MIT obligations don't apply to our tree.

The provenance comments described above are *not required by the MIT license* (we copied nothing) — they exist for our own engineering hygiene, so future agents can find the inspirational source.

If the rule ever changes and we *do* copy code, the MIT terms kick in: we'd preserve their `LICENSE` file alongside the copied code, add an attribution header. Until then, no license obligations to RuVector exist.

---

## Things that look stealable but aren't

### A. The full `crates/rvf/` single-file format

**Why it looks tempting:** the 25-segment-type RVF format is genuinely impressive — VEC, INDEX, KERNEL, EBPF, WASM, COW_MAP, WITNESS, CRYPTO, and 17 more segments packed into a single addressable file. "Boots as a microservice in 125ms."

**Why don't take it (beyond `rvf-crypto`):**

1. **Mismatch with RocksDB.** Your storage is RocksDB-per-CF. Adopting `.rvf` packs duplicates the persistence story. Your FSV protocol (open the CF, read the row, recompute the hash) becomes harder when bytes live in nested segments.

2. **Format isn't fully documented.** The crate code is the canonical spec. Adopting an undocumented binary format on the critical path imports a future maintenance risk you can't measure.

3. **No SWE-bench / DynamicJEPA story exists in `rvf-runtime`.** It would be a parallel persistence path with no immediate reward.

**Take just `rvf-crypto`.** The witness chain + lineage + attestation primitives are the value. The runtime + manifest + wire formats are not.

---

### B. The `ruvllm` LLM runtime

**Why it looks tempting:** Rust-native GGUF loader, Metal + CUDA + WebGPU kernels, BitNet, MoE, paged attention, TurboQuant — built-in alternative to llama-cpp-2.

**Why don't take it:**

1. **No sm_120 kernels.** `ruvllm` uses candle for CUDA; candle doesn't ship sm_120 kernels by default. Your existing custom CUDA path in ContextGraph compiles native `sm_120a` cubin (per CLAUDE.md §3 5090jepa env). Switching to ruvllm regresses performance until someone writes the sm_120 kernels — and that's *your* job, not RuVector's.

2. **Different runtime model.** `ruvllm` is built around GGUF + paged attention + a serving layer. Your inner-actor invocation is a much simpler "load model once, generate, exit" pattern. You'd be importing complexity you don't need.

3. **Active development churn.** PR #450 was merged 2026-05-08 with new attention crates; the runtime layer evolves fast. Pinning vs tracking is a real ongoing cost.

**Defer.** If you ever need a Rust-native serving layer for, say, a verifier-prompt LLM, revisit. The inner actor stays on llama-cpp-2.

---

### C. The full `ruvector-graph-transformer` crate

**Why it looks tempting:** 8 verified module families — physics, biological, manifold, temporal, economic, self-organizing, sublinear, proof-gated. Sounds comprehensive.

**Why don't take all of it:**

1. **Most modules are research-grade.** Hamiltonian flow attention, gauge equivariance, Hebbian/STDP, morphogenetic fields, Nash equilibrium attention — all interesting, none directly applicable to ccreality or 5090jepa Phase 1.

2. **Each module is feature-gated.** Building only what you need means a long `features = [...]` list; debugging which feature pulls which sub-dep is its own project.

3. **The transitive dep tree is deep.** `verified_training` pulls `ruvector-verified` pulls custom proof DSL machinery. Adopting it means owning that dep chain forever.

**Selectively port the pieces you need:**
- Causal masking from `temporal.rs` — port the `MaskStrategy` enum.
- Granger causality test from `temporal.rs` — port the test function (~300 LOC).
- `verified_training` `TrainingCertificate` shape — port the certificate struct + verification logic, leave the proof-environment plumbing.

---

### D. `ruvector-consciousness`, `ruQu`, `ruvector-nervous-system`

**Why it looks tempting:** "IIT4 Φ computation," "quantum coherence error correction via mincut," "5-layer bio-inspired adaptive system with spiking nets and BTSP learning" — impressive labels.

**Why don't take any of them:**

1. **Research-grade by their own admission.** No production deployment story.
2. **No alignment with ccreality / 5090jepa.** Φ-computation isn't asking a question your loop wants answered.
3. **Heavy dependencies on `ruvector-mincut`.** If you adopt them, you commit to a particular mincut style; if you later prefer a different cut algorithm, you've painted yourself in.

**Skip unless an explicit research side-quest opens.**

---

### E. `agentic-robotics-*`, `neural-trader-*`, `thermorust`, `ruvix`, `ruvector-kalshi`

Domain-specific. Safe to ignore.

---

### F. The `mcp-brain*` family + the `pi.ruv.io` integration

**Why it looks tempting:** the embedded `CLAUDE.md` (loaded at session start when reading the ruvector clone) talks about "pi.ruv.io" — a hosted Google Cloud Run brain at SSE endpoint with 1,500+ memories, 350K+ graph edges, witness audit, differential privacy ε=1.0, accessible via MCP tools `brain_status / brain_search / brain_share / brain_partition`.

**Why don't blindly adopt:**

1. **It's a hosted service.** Adding `pi.ruv.io` to ContextGraph's MCP gate means your reality-loop now depends on someone else's deployed service. CLAUDE.md §11.9 says "never use any HTTP API directly (no curl, wget, fetch)." This rule was written for the inner actor's Docker eval, but the spirit applies: the optimizer's substrate should be local.

2. **Differential privacy noise.** Their embeddings are noised at ε=1.0. For an optimizer making decisions about harness changes, noised retrieval might be fine, but for FSV-grade evidence linkage, it isn't.

3. **It's not the operator's substrate.** Operator wants ContextGraph + RocksDB + MCP — not "ContextGraph + RocksDB + MCP + a remote brain."

**The crate code (`mcp-brain`, `mcp-brain-server`) is fine to read for ideas** — it's a reference implementation of "how to build a witness-chain-audited MCP-attached collective brain." The hosted service is a separate concern; don't depend on it.

---

## Risks if you DO adopt

### G. SONA EWC++ task-boundary detection might mis-fire

**The trap:** SONA's automatic task-boundary detection (per `crates/sona/src/ewc.rs`) uses gradient mean+variance + a threshold (`boundary_threshold: 2.0` default). Across SWE-bench Lite tasks, the optimizer's decision-stream gradient distribution might trigger boundaries spuriously (every other turn looks like a "new task") OR not at all (every turn looks like the same task).

**Mitigation:**
- Run with `RUST_LOG=sona=debug` for the first 50 turns.
- Inspect `samples_seen` at each task-boundary detection event.
- If boundaries fire too often → raise `boundary_threshold` (try 3.0 or 5.0).
- If never → lower to 1.5.
- Once tuned, write the value to `./memory/decisions/sona-config-tuned.md` per CLAUDE.md §6.

---

### H. Solver algorithm selection / integration

**The trap:** the RuVector reference has a router, but ContextGraph does not. Our current `context-graph-solver` exposes explicit Forward-Push and CG calls. A future router could accidentally pick the wrong algorithm, or worse, label a CG solve as BMSSP/multigrid.

**Mitigation:** for ccreality-specific use cases (PPR over failure/recommendation/attempt graphs), call `ForwardPushSolver` directly. Keep BMSSP unimplemented until a real multigrid V-cycle lands with FSV evidence.

---

### I. `ruvector-temporal-tensor` quantization is lossy

**The trap:** Cold-tier (3-bit) quantization is lossy. Round-tripping a JEPA panel through cold-tier and back loses ~5-10% of the original signal magnitude. Acceptable for retrieval; **not acceptable for training**.

**Mitigation:** keep training tensors at the hot tier (8-bit) or uncompressed. Cold-tier is for inference-time aux-memory retrieval only. Document this rule in `./memory/discoveries/temporal-tensor-training-vs-retrieval.md`.

---

### J. `ruvllm_retrieval_diffusion` Diffuser depends on corpus quality

**The trap:** the Diffuser is training-free, which means it's exactly as good as your corpus. A corpus of 100 successful gridworld trajectories will give okay coverage. A corpus of 10 successful trajectories will give garbage — the diffuser will retrieve those 10 trajectories no matter what the query.

**Mitigation:**
- Bootstrap the corpus with random play (1000 random trajectories, keep the high-reward ones).
- Iteratively add SONA-trained policy trajectories.
- Set a minimum-corpus-size guard (`if corpus.len() < 50 { return None; }`).

---

### K. The witness chain is append-only, including mistakes

**The trap:** the witness chain doesn't know when an entry is "wrong." If the optimizer records a recommendation it later regrets, the regret can't unwrite the entry. You can append a *retraction* entry, but the original is forever in the chain.

**Mitigation:** this is the design intent — tamper-evident audit. But communicate it: write `./memory/decisions/witness-chain-is-append-only.md` documenting that mistakes are part of the chain, retractions are explicit, and the chain is *evidence* not *current state*.

---

### L. Reimplementing SONA still creates state-schema compatibility risk

**The trap:** SONA's `SonaConfig`, `LearningSignal`, `QueryTrajectory` types are useful reference shapes, but our clean-room `context-graph-sona` state will be our own public API. If that safetensors schema changes without explicit migration, older optimizer/JEPA state will not load cleanly. Cross-version interoperability is *not* free.

**Mitigation:** version our SONA state schema from day one, write migration tests against real safetensors files, and surface any incompatible change in `./memory/decisions/sona-state-schema.md`.

---

### M. The cloned `ruvector/` repo is 11,137 files / multi-GB

**The trap:** cloning the repo into `ContextGraph/ruvector/` adds significant disk + working-tree size. The clone is useful only as a read-only reference; it must never become a build dependency.

**Mitigation:**
- Add `ruvector/` to `.gitignore` (don't commit the clone — it's a tool checkout, not a source dependency).
- Do not vendor crates into `vendor/`; reimplement the bounded subset in our own `crates/context-graph-*` modules.
- Periodically (weekly) `git -C ruvector fetch --shallow-since='1 month ago'` to keep the local clone manageable.

---

### N. `rvf-crypto` no_std means it's careful about heap usage

**The trap:** `no_std` crates avoid `std::HashMap`, `std::Vec` (just `alloc::Vec`), and `std::time`. The witness chain doesn't time-stamp from `SystemTime::now()`; it expects you to pass `timestamp_ns: u64`. If you pass time inconsistently (mixing UTC + local + offset-applied), entries timestamps drift and any time-window query lies.

**Mitigation:** centralize time in `crates/context-graph-cli/src/time.rs::now_ns()`. Document UTC. Test on system-clock-jump scenarios.

---

## Watch list

If you adopt Tier 1 transplants per `05_QUICK_WINS_VS_DEEP_INTEGRATION.md`, the things that will earliest tell you something is wrong:

| Symptom | Probable cause |
|---------|----------------|
| Witness chain `verify` returns `valid: false` on a fresh run | Either chain genuinely tampered (a real bug) OR your time-stamp logic diverges from `rvf-crypto` expectations (port bug). Diff against `crates/rvf/rvf-crypto/src/witness.rs:66-79`. |
| `ruvector-solver` PPR returns NaN scores | Graph has self-loops or zero-weight edges; check edge construction. |
| `ruvector-temporal-tensor` reconstruction error >5% on 8-bit tier | TierPolicy mis-configured. Run unit-test with synthetic uniform random vectors first. |
| SONA `tickCount` stays at 0 after 100 turns | Background loop isn't being driven; check `engine.tick()` call site. |
| `ruvllm_retrieval_diffusion::Diffuser` returns the same output for any query | Corpus too small or too uniform. Verify `corpus.len() > 50` and entropy of corpus tokens > some threshold. |
| Conformal prediction set is consistently empty | `alpha` too small or calibration set too small. Defaults are 0.1 + 20% of total data; adjust. |

Each of these is fast to detect with the right telemetry. Plan the telemetry up front — write `./memory/discoveries/ruvector-failure-modes.md` with a checklist after Tier 1 ships.

---

## Things you should NOT lift verbatim from the docs

### O. The README's marketing claims

- "150× faster than SQLite" — same caveat as agentdb; bad baseline.
- "+36% search quality from feedback" — synthetic benchmark.
- "8.2× faster than hnswlib" — measured on `crates/ruvector-hyperbolic-hnsw` (excluded sub-crate); not generalizable.
- "98.2% coherence across 24 validation runs" — internal validation; not externally reproducible.

Don't quote these in `./memory/decisions/` or in operator-facing surfaces.

### P. The "75 features across 10 categories" pitch

It's marketing, not architecture. Several listed "features" overlap (e.g., "ColBERT multi-vector" and "MaxSim retrieval" are the same thing). Don't treat the list as a checklist.

### Q. The CES 2026 award context

Operationally meaningless. Don't let award branding influence engineering decisions.

---

## Summary — don't-port list

| Item | Reason |
|------|--------|
| Full `crates/rvf/` (beyond rvf-crypto) | RocksDB FSV mismatch |
| `ruvllm` runtime | No sm_120 kernels |
| `ruvector-attention` Flash/MLA/Mamba/sheaf | Same — no sm_120; you have candle |
| `ruvector-graph` Cypher engine | You have E5 causal graph |
| `ruvector-postgres` | RocksDB-based workspace |
| `ruvector-consciousness`, `ruQu`, `nervous-system` | Research-grade, no alignment |
| `agentic-robotics`, `neural-trader`, `thermorust`, `ruvix`, `ruvector-kalshi` | Domain mismatch |
| `mcp-brain*` (the hosted pi.ruv.io flavor) | External hosted service; not local-substrate |
| `ruvector-fpga-transformer` | No FPGA |
| `ruvector-hailo*` | No Pi 5 + AI HAT+ |
| `ruvllm_sparse_attention` no_std mode | No ESP32-S3 |
| Marketing perf claims | Synthetic benchmarks |

---

## Read next

- `07_GIT_HISTORY_LAST_2_DAYS.md` for the commit-by-commit signal.
- `08_HONEST_SUMMARY.md` for the verdict.
