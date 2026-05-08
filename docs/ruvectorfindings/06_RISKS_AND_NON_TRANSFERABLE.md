# 06 — Risks & Non-Transferable Pieces

The honest list. Vendor-vs-path-dep tradeoffs, license considerations, things that look stealable from RuVector but aren't actually a fit, and failure modes once you adopt.

---

## Vendor vs path-dep — pick once, document it

The two ways to consume RuVector code:

### Vendor (copy into ContextGraph tree)

**Pros:**
- Stable. Your build doesn't depend on `ruvector/` repo state.
- Auditable. You see exactly what's in your tree.
- License-clean. The MIT/Apache notices live in your tree where reviewers expect them.
- Works after `git clean` of `ruvector/`.

**Cons:**
- Manual upgrades. When RuVector ships a fix, you re-vendor.
- Diverges over time. Your vendor copy and upstream will drift.
- Larger PRs. The vendor commit is hundreds of files.

### Path-dep (`Cargo.toml` `path = "../ruvector/crates/..."`)

**Pros:**
- Tracks upstream automatically.
- Smaller commits.
- Easier to upgrade individual crates.

**Cons:**
- Build breaks on upstream churn. RuVector commits multiple times per day; one bad commit upstream breaks `cargo build`.
- Path-dep across the repo boundary is fragile. CI workers need both repos checked out.
- The upstream `ruvector/` repo isn't in your `.gitignore` policy. If someone deletes it, your build dies.
- Some RuVector crates are in workspace `exclude` (e.g., everything under `crates/rvf/*`); path-dep against an excluded sub-workspace requires pointing at the sub-workspace's own `Cargo.toml`, which has its own quirks.

### Recommendation per crate

| Crate | Choice | Why |
|-------|--------|-----|
| `rvf-types` + `rvf-crypto` | **Vendor** | Critical-path security; want stability + license-clean inclusion. |
| `sona` | **Vendor** | Long-running learning state; you don't want upstream churn changing trajectory semantics mid-experiment. |
| `ruvector-solver` | **Path-dep** | Pure algorithms; upstream improvements are wins, not surprises. |
| `ruvector-temporal-tensor` | **Path-dep** | Zero-deps crate; minimal churn risk. |
| `ruvector-mincut` | **Path-dep** | Same. |
| `ruvector-core::advanced_features::*` | **Path-dep** | Each module is small; track upstream. |
| `ruvllm_retrieval_diffusion` | **Path-dep** | Brand new (PR #450); will see active churn. Stay current to get bugfixes. |
| `ruvector-graph-transformer` | **Selective port** | Only the pieces you need (Granger, causal masking) — avoid the full feature-flag surface. |
| `prime-radiant` | **Vendor if adopted** | Heavy tree; pinning a snapshot keeps your build deterministic. |
| `ruvector-domain-expansion` | **Path-dep** | Small surface. |

---

## License considerations

RuVector is **MIT** (per `LICENSE` and the README badge). Permissive — code copy + clean-room ports are both fine.

When vendoring:

- Preserve the MIT `LICENSE` file alongside the vendored code at `vendor/<crate>/LICENSE`.
- Add a header comment to each vendored file:
  ```rust
  // Vendored from https://github.com/ruvnet/RuVector
  // Source: crates/<crate>/src/<file>.rs at commit ef5274c2
  // Original: MIT-licensed; see vendor/<crate>/LICENSE
  ```
- If you modify the file in your tree, add a second comment line documenting the modification.

When path-dep'ing: less ceremony, but document in your `Cargo.toml`:

```toml
# Track upstream RuVector at commit ef5274c2 (2026-05-08).
# Re-pin via `git -C ../ruvector pull && git -C ../ruvector log -1 --oneline`
ruvector-solver = { path = "../ruvector/crates/ruvector-solver" }
```

The dependency tree of `ruvector-*` crates can include some non-MIT optional deps (e.g., `argon2` is Apache-2.0). Run `cargo license` after vendoring; surface anything non-MIT/Apache to the operator.

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

### H. `ruvector-solver` algorithm selection by router

**The trap:** `ruvector-solver/src/router.rs` automatically picks an algorithm based on matrix properties. For a sparse PPR query, it picks Forward Push; for a dense Laplacian, BMSSP. Wrong picks just mean slow convergence — but you might not notice until a tight latency budget.

**Mitigation:** for ccreality-specific use cases (PPR over (failure_tag, recommendation, attempt) graphs), call `ForwardPushSolver::new(...)` directly rather than the router. Skips the auto-selection.

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

### L. Vendoring `sona` ties you to its ABI

**The trap:** SONA's `SonaConfig`, `LearningSignal`, `QueryTrajectory` types are public API. If you vendor at commit X and upstream evolves the schema at commit Y, your safetensors-exported state isn't loadable by upstream's commit Y consumers. Cross-version interoperability is *not* free.

**Mitigation:** if you only consume SONA's safetensors, no concern. If you ever federate with another consumer of SONA, pin to the same upstream commit. Surface this in `./memory/decisions/sona-version-pin.md`.

---

### M. The cloned `ruvector/` repo is 11,137 files / multi-GB

**The trap:** cloning the repo into `ContextGraph/ruvector/` adds significant disk + working-tree size. Path-deps mean you can't rm -rf the clone.

**Mitigation:**
- Add `ruvector/` to `.gitignore` (don't commit the clone — it's a tool checkout, not a source dependency).
- Vendor the crates you actually need into `vendor/` — your tree reflects only what's used.
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
