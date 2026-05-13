---
namespace: discoveries
agent_id: agent-205-research-historical-docs
timestamp: 2026-05-10T00:00:00Z
status: active
tags: ruvectorfindings, historical, audit
---

# Historical RuVectorFindings Audit (docs 01-06)

Read-only research over `docs/ruvectorfindings/01_*` through `06_*`. Each doc already carries a 2026-05-09 PIVOT banner and a 2026-05-08 LOCKED clean-room banner; both supersede the body in places. Findings below summarize what each doc still teaches vs. what's been overtaken by docs 09 (master plan), 10 (pivot statement), 11 (UTML learning formula), 12-14 (specs reorg + AST chunking).

---

## 01_RUVECTOR_ARCHITECTURE.md

**Summary.** Map of RuVector's 135-crate, 9-layer Rust workspace (vector storage → crypto/RVF → solvers → mincut → attention → graph-transformer → SONA self-learning → ruvllm runtime → MCP/specialty/applications). Uses RuVector strictly as read-only inspiration for clean-room reimplementation; documents `rvf-crypto` (73-byte SHAKE-256 witness chain), `ruvector-solver` (Forward Push / BMSSP / CG), `ruvector-mincut`, `sona`, and the freshest commit `ruvllm_retrieval_diffusion` (PR #450 2026-05-08). Closes with a ContextGraph-vs-RuVector comparison table.

**Still-actionable.** (1) `rvf-crypto` witness chain is the highest-leverage clean-room target — confirmed already shipped per `context-graph-witness`; lineage/attestation/ed25519 still cited as remaining. (2) `ruvector-solver` Forward Push + CG is canonical reference — partially shipped in `context-graph-solver`. (3) Architecture-layer index of where to look in the upstream tree when reimplementing a primitive remains useful as a navigation map.

**Stale.** (1) Comparison row claims ContextGraph "depends on RuVector HNSW" — pre-pivot, now retired. (2) Cites `ruvllm` as relevant runtime substrate — pivot retires inner-actor LLM, ruvllm is reference-only. (3) Pre-pivot framing of ContextGraph as "purpose-built for ccreality + DynamicJEPA" is overtaken by the ME-JEPA-Code single-deliverable framing in doc 10.

**Obsolete sections.** Layer 6 (ruvllm + reasoning) and Layer 7 hardware backends (Hailo / FPGA / ESP32) — not on critical path; doc itself flags as skip. The CES 2026 + marketing-claim bullets in the workspace stats block.

---

## 02_CAPABILITIES_INVENTORY.md

**Summary.** Per-crate capability catalog (sections A-R) covering ruvector-core retrieval (HNSW/DiskANN/PQ/OPQ/MMR/Matryoshka/Graph-RAG/conformal), `sona` (μLoRA + EWC++ + REINFORCE + ReasoningBank + federated + safetensors export), `rvf-crypto` (SHAKE-256 witness + ed25519 + ML-DSA-65 + lineage + attestation), `ruvector-solver`, `ruvector-mincut`, `ruvector-attention` (~50 mechanisms), 8 graph-transformer modules, temporal-tensor 8/7/5/3-bit tier compression, `ruvllm`, retrieval-diffusion, `prime-radiant`, and the rvf sub-workspace. Verdict table at the bottom maps each crate to "reimplement / read / skip".

**Still-actionable.** (1) §C `rvf-crypto` capabilities (witness 73-byte layout, lineage, attestation) remain the canonical inspiration map — directly feeds doc 09's `E_Witness` instrument. (2) §H temporal-tensor tier table (8/7/5/3-bit, 4-10× ratios) maps to doc 09 §6.2 panel cold storage — confirmed shipped 2026-05-09c per MEMORY.md. (3) §G.4/G.9 (temporal causal masking + verified_training certificates) inform Phase 2 predictor and Phase 3 verified-training contracts in doc 09.

**Stale.** (1) §K `ruvllm` "verdict: not in scope post-pivot" — flagged correctly but bullet still discusses TurboQuant / GGUF / paged attention as if relevant. (2) §B sona effort estimate "5-7 days clean-room" — superseded by doc 09 phasing; SONA is now scaffolding for Phase 6 self-healing, not a standalone Tier-2 deliverable. (3) §J router/tiny-dancer FastGRNN routing is labeled "quality-of-life" — under the single-outer-loop model there is no inner intent dispatcher to optimize.

**Obsolete sections.** §K ruvllm runtime, §L `ruvllm_sparse_attention` no_std, §M retrieval-diffusion's "DynamicJEPA gridworld planner" framing — all assume an inner-actor or sibling JEPA pack that's been retired; ME-JEPA-Code is non-generative and code-domain only.

---

## 03_REALITY_LOOP_APPLICATIONS.md

**Summary.** Reality-loop transplant catalog framed for the *pre-pivot* outer-Claude + inner-Qwen architecture. 10 numbered targets (PPR influence, witness chain, SONA, MMR/Matryoshka/Graph-RAG/conformal, tier compression, mincut splits, FastGRNN router, coherence-tuning, Granger, retrieval diffusion). Includes a current-state block showing witness + Forward-Push + CG + MMR + influence MCP tool already shipped through commits c18c694e → a9ca688e.

**Still-actionable.** (1) §1 PPR-over-failure-graph + §2 witness-chain wiring patterns remain valid as ME-JEPA aux-feature plumbing (the influence ranking primitive is generic). (2) §4d conformal prediction for calibrated recommendation confidence aligns with doc 09 §3.5 conformal calibration phase. (3) §6 mincut for clean A/B / leakage splits maps directly to ME-JEPA train/calibration/holdout split (already shipped 2026-05-09b per MEMORY.md).

**Stale.** (1) Whole framing of "ccreality optimizer + inner Qwen attempts" — pivot retired the inner actor; references to `inner_actor_correctness`, "harness-transitions/round-NN.json" attempt artifacts, Qwen response files. (2) §3 SONA's reward signal `final_quality = inner_actor_correctness` — the reward source no longer exists; replace with oracle pass/fail or operator override per doc 09. (3) §3.6 "verify_ai_output gates" framing is explicitly SUPERSEDED by doc 09 banner: gates are *input features* to ME-JEPA, not standalone gates.

**Obsolete sections.** Code examples that wire SONA into an "inner-actor correctness" loop. The legacy `<runtime_root>/<task_id>/qwen3.6-27b/attempts/...` artifact paths in §1c. The two-loop language used throughout — replace conceptually with single Claude Code outer loop feeding ME-JEPA via `mejepa-shift-subscriber`.

---

## 04_JEPA_APPLICATIONS.md

**Summary.** Twelve transplant targets for the pre-pivot JEPA family (ME-JEPA + DynamicJEPA + 5090jepa). Top of doc clarifies post-pivot ME-JEPA-Code is the sole deliverable and DynamicJEPA + 5090jepa references are sibling illustrations. Targets: witness chain, PPR trajectory importance, SONA planner, tier compression, Stage 0/F certificates, Stoer-Wagner mincut, cross-pack transfer, Granger + retrocausal safety, verified_training certificates, retrieval-diffusion gridworld planner, prime-radiant coherence, manifold cross-instrument transfer.

**Still-actionable.** (1) §1 witness-chain enum + Stage0/StageF/TrainingEpoch witness types directly inform doc 09's `E_Witness` and per-epoch training cert architecture. (2) §6 Stoer-Wagner mincut for actor/leakage-aware splits — shipped 2026-05-09 (Tier 2 entry in MEMORY.md). (3) §9 verified_training per-step `TrainingCertificate` (LossStability/WeightNorm/Lipschitz/Energy invariants, fail-closed delta-apply) is the right primitive for ME-JEPA Phase 3 training-certificate machinery.

**Stale.** (1) §3 SONA-as-planner-head and §3a EWC++ "across domain packs (gridworld + counter_world)" — DynamicJEPA pack architecture is no longer this project's deliverable. (2) §10 retrieval-diffusion-for-gridworld is sibling-domain only; not on ME-JEPA-Code critical path. (3) §11 prime-radiant coherence as anti-pole alternative — ME-JEPA-Code uses VICReg + frozen-instrument-target loss instead per doc 09 §3; the anti-pole framing comes from a different JEPA variant.

**Obsolete sections.** All gridworld_5x5 / counter_world_5x5 references; §7 "counter_world ↔ gridworld smoke" prerequisite for cross-pack transfer; §10a/10b vocab-≤-254 plumbing for DynamicJEPA action vocab. ME-JEPA recruiting-corpus references in §4b — recruiting domain is sibling, not target.

---

## 05_QUICK_WINS_VS_DEEP_INTEGRATION.md

**Summary.** Sprint-level effort triage for Tier 1-4 reimplementation targets identified in docs 03/04. Tier 1 (~15-19d): witness, solvers, tier compression, MMR, retrieval diffusion, conformal. Tier 2 (~24-31d): SONA, verified_training, Stage 0/F, mincut, Matryoshka + Graph-RAG, Granger. Tier 3 (~40+d): cross-pack, causal masking, coherence-tuning, FastGRNN router, prime-radiant, manifold. Tier 4: skip (ruvllm, attention, Cypher, postgres, consciousness, robotics, FPGA, Hailo, ESP32). Includes ship-signal checklist per tier and a "current sprint status" block.

**Still-actionable.** (1) Tier table sequencing (1.1 → 1.2 → 1.3 → ...) remains a useful effort-cross-check against doc 09's 8-phase plan. (2) Risk-gating prerequisites table (Tier 2.1 needs Tier 1.1, 2.2 needs 1.1+2.1, etc.) is a generic dependency map that survives the pivot. (3) Tier 4 skip-list (no FPGA / Hailo / ESP32 / Cypher / postgres / `.rvf` runtime) remains correct policy.

**Stale.** (1) Tier 1.5 "training-free retrieval LM + diffuser for gridworld" — gridworld pack is sibling, not target; demote from Tier 1. (2) "Two-week sprint plan" for Tier 1 — overtaken by doc 09's 8-phase Phase 0 → Phase 8 plan in `specs/implementation-plan.md`. (3) The 4.2 row "qwen3.6:27b via llama-cpp-2" effort triage is N/A — banner explicitly flags this; pivot retired the inner actor.

**Obsolete sections.** "Recommended two-week sprint plan" Days 12-15 (retrieval diffusion for gridworld) and the entire `sitrep/` reference (banner notes `sitrep/` tree was removed in commit 6f72a02e). "Tier 4.2 ruvllm runtime" rationale (no sm_120) remains correct but the underlying need is gone — ME-JEPA is non-generative.

---

## 06_RISKS_AND_NON_TRANSFERABLE.md

**Summary.** Honest risk register: clean-room reimplementation discipline (no copy-paste, mandatory provenance comments, naming/test/error-type independence), MIT licensing (no obligations because no code copied), don't-port list (full RVF format, ruvllm runtime, full graph-transformer crate, consciousness/quantum/nervous-system, mcp-brain hosted pi.ruv.io, marketing claims), risks-if-adopted (SONA EWC++ mis-fire, solver router mis-routing, lossy cold-tier quant for training, retrieval-diffusion corpus quality dependency, append-only mistakes, schema drift, 11K-file clone disk usage, no_std time-stamping). Live operational concern noted: `CCREALITY_WITNESS_PREV_HASH_MISMATCH` at offset 0 from pre-canonical-format chain bytes.

**Still-actionable.** (1) Clean-room discipline (provenance header template, test commands grepping for forbidden path-deps, crate naming `context-graph-*`) — load-bearing policy and applies to every new ME-JEPA primitive. (2) Append-only-witness-chain risk K (mistakes are forever; retractions are explicit) — directly relevant to doc 09's audit-trail design. (3) `.gitignore ruvector/` clone-management practice + weekly shallow fetch for the local read-only reference.

**Stale.** (1) Risk G "SONA EWC++ task-boundary detection" tuning advice — pre-pivot SONA-as-optimizer assumption; doc 09 uses EWC++ for predictor self-healing not optimizer learning. (2) "Inner actor on llama-cpp-2 / verifier-prompt LLM / ruv.io HTTP" risks — banner flags as no longer load-bearing. (3) Risk J retrieval-diffusion corpus-size guard — sibling-domain concern (gridworld), not ME-JEPA-Code.

**Obsolete sections.** Don't-port section B "ruvllm LLM runtime" rationale around sm_120 kernels — true but moot post-pivot. Section F "mcp-brain + pi.ruv.io" — the operator never wanted a hosted brain; section is now historical context only. Risk H "solver router could pick wrong algorithm" — current `context-graph-solver` exposes explicit Forward-Push and CG calls; no router shipped.

---

## Cross-cutting observation

All 6 docs already carry pivot + clean-room banners, and the bodies retain pre-pivot framing as historical context. The key generic primitives they catalog (witness chain, Forward-Push PPR, MMR, Stoer-Wagner mincut, tier compression, Granger, conformal prediction, verified_training certificates) all map cleanly to ME-JEPA-Code components per doc 09 — the *primitives survive*, the *two-loop / inner-actor / sibling-pack framing* does not.

Heaviest staleness concentration: doc 03 (whole reality-loop framing) and doc 04 §10 + §3a (gridworld / counter_world pack examples). Heaviest still-relevance: doc 02 capability map and doc 06 clean-room policy.
