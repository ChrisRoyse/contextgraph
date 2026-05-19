# ClipCannon vs ContextGraph / ME-JEPA-Code — Multi-Embedder Architecture Comparison

**Author:** Chris (via Claude analysis)
**Date:** 2026-05-19
**Sources:**
- ClipCannon: `/home/cabdru/clipcannon/` (Python, BSL-1.1), `docs/clipcannon_whitepaper.md`, `docs/clipcannon_teleological_constellations_whitepaper.md`, `docs/teleological_constellation_training.md`, `docs/codestate/*`, `src/clipcannon/db/schema.py`, `src/clipcannon/pipeline/visual_embed.py`, `src/clipcannon/pipeline/highlights.py`.
- ContextGraph: `/home/cabdru/contextgraph/` (Rust workspace), `docs/PAPER.md`, `docs/alldata.md`, `crates/context-graph-mejepa-instruments/src/lib.rs`, `crates/context-graph-mejepa-tct/src/{gtau.rs,constellation.rs,lib.rs}`, `crates/context-graph-mejepa/src/predictor/mod.rs`, `crates/context-graph-mejepa-embedders/src/forward.rs`.

Both systems are yours. Both implement multi-embedder architectures, both invoke "teleological constellation" language, and they share a published paper (`PAPER.md` §4.1 and §4.2 name them as two production instances of the same DDA framework). But the systems are aimed at *very different shapes of problem* — and that has produced two genuinely different multi-embedder strategies underneath the shared vocabulary. This document maps what each does, where they line up, where they diverge, and what each could learn from the other.

---

## 0. TL;DR

| Dimension | ClipCannon | ContextGraph / ME-JEPA-Code |
|---|---|---|
| Job to be done | Understand + edit + render a video; clone a person's identity end-to-end | Predict whether an AI-generated code change is correct/safe before a human reviews it |
| Output | Edits, renders, voice clones, lip-synced avatars | `RealityPrediction` per candidate patch (Pass / Fail / OutOfDistribution / Abstain / GuardRejected) |
| Truth signal | Cosine distance to a *frozen identity centroid* in re-embedded output space | Cosine distance from a frozen *per-slot* code-state centroid + oracle test outcomes from a real Docker harness |
| Number of embedders | **5 storage spaces** (visual, semantic, emotion, speaker, voice-id) + ~12 supporting ML models. Constellation extends to **7 modalities** | **13 retrieval embedders** (E1–E13) on text/code memory + **15 frozen instrument slots** (5,120-d panel) on code state |
| What is trained | LoRA on a generative model (EchoMimicV3 video diffusion, Qwen3-TTS via best-of-K, Gemma-4 SFT+DPO for text) | Predictor + heads on top of frozen instrument panel (no LLM inner loop; the agent itself is the actor) |
| Fusion strategy | **Weighted late fusion** (`highlights` stage = sum of 7 weighted signals). Constellation guard runs single-modality threshold checks (visual frame-by-frame). | **Strict per-slot conjunction** (`gtau_check`) — every embedder must clear its own threshold; no compensation across slots. RRF fusion only at memory-retrieval query time, not for the predictor decision. |
| Storage | Per-project SQLite with 4× `sqlite-vec` vec0 tables + ~34 core tables | Single RocksDB store with 59 column families; HNSW indexes per embedder; AgentDB-style typed edges |
| Provenance | SHA-256 hash chain linking every pipeline op | RocksDB audit-log CF + provenance MCP tools + FSV evidence files on durable aiwonder ZFS |
| Verification posture | Per-frame Constellation Guard (rejects frames where re-embedded SigLIP < 0.70 cosine vs identity centroid) | Per-output Gτ guard *plus* oracle-test ship gate requiring `prediction_oracle_correlation ≥ 0.95` across 4 rolling windows |
| Maturity | Shipping product, 994 tests, 54 MCP tools, paid product (BSL-1.1) | Phase G in progress, 5,184+ tests, 75 MCP tools, currently `0.866667 < 0.95` on the master ship gate |
| Sample size of identity targets | 1 subject (Santa) + voice cloning anchor at `0.961 mean SECS` | Cross-task generic — must work on every SWE-bench Lite instance, not on a single named subject |

**One-line summary.** ClipCannon optimizes a *single named identity* and uses 7 frozen embedders as a *defense-in-depth filter* over the model's generative output. ContextGraph / ME-JEPA-Code optimizes a *general-purpose calibrated verdict* on arbitrary code changes and uses 13 retrieval embedders + 15 frozen instrument slots as the *substrate for a learned predictor* whose verdict is the product. The architectural verb on the ClipCannon side is **reject**; on the ME-JEPA side it is **predict**.

---

## 1. What each system actually is

### 1.1 ClipCannon

A 247-file Python package (~67,585 LOC) running as an MCP server on stdio plus a FastAPI license server (port 3100) plus a FastAPI dashboard (port 3200). It ingests a video, runs a 23-stage analysis DAG over it (probe → VFR normalize → audio_extract → frame_extract → source_separation → visual_embed → ocr → quality → shot_type → storyboard → transcribe → acoustic → scene_analysis → semantic_embed → narrative_llm → speaker_embed → emotion_embed → reactions → prosody_analysis → profanity → chronemic → highlights → finalize), stores everything in a per-project SQLite database with four `sqlite-vec` virtual tables, and exposes 54 MCP tools an AI assistant can call to:

- search / understand content (`search_content`, `get_transcript`, `get_frame`),
- discover edit material (`find_best_moments`, `find_cut_points`, `get_narrative_flow`, `find_safe_cuts`),
- compose iterative edits (11 editing tools with full version control over an Edit Decision List model),
- render to 7 platform profiles (NVENC h264_nvenc with libx264 fallback),
- generate music (ACE-Step diffusion, MIDI presets, DSP SFX),
- clone voices (Qwen3-TTS 1.7B with multi-gate verification),
- lip-sync avatars (LatentSync 1.6),
- run a real-time meeting-bot voice agent (Whisper → Qwen3-8B → faster-qwen3-tts).

On top of all of that sits the **Teleological Constellation System** (Phoenix avatar engine + 4 constellation MCP tools): a method for cloning a specific person from ~16 minutes of source video, controlling them through 8 named behavioural states (e.g. `warm_conversational`, `empathetic_concern`), and validating every generated frame against a 7-modality frozen identity manifold.

### 1.2 ContextGraph / ME-JEPA-Code

A Rust workspace (~370K LOC across ~14 crates) implementing two interlocking products:

1. **ContextGraph** — a 13-embedder memory + retrieval substrate (E1–E13). New text, code, or conversation chunks are projected through every embedder simultaneously, stored in RocksDB (59 column families), and exposed via 75 MCP tools (search, traversal, causal analysis, audit, provenance). This is the "labeller" role from PAPER §3.4.

2. **ME-JEPA-Code** — a predictor that sits on top of a 15-slot, 5,120-d frozen instrument panel (E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Test, E_Trace, E_Diff, E_Witness, E_Oracle, E_Problem, E_CommitMsg, E_StaticAnalysis, E_Runtime, E_Reasoning, Scalars). Its job is to take a candidate code change from an external LLM coding agent and return a calibrated `RealityPrediction` that answers five operator questions:

| Q | Question | Mechanism |
|---|---|---|
| Q1 | Does the AI's claim *exist* in bytes/AST/test state? | Reconcile claim graph against SHA chain on disk |
| Q2 | Will it *work*? | Predictor + conformal interval + TCT Gτ guard → `Verdict::{Pass, Fail, OOD, Abstain, GuardRejected}` |
| Q3 | *Why* would it fail? | Ranked predicted failure modes + violating embedders + closest exemplars |
| Q4 | *What else* could go wrong? | Always-on heads: edge cases, latent bugs, tech debt, dead code, redundancy, perf, security, accuracy, cost |
| Q5 | How is it *impacting reality*? | Replays the durable shift log against predicted side effects |

ME-JEPA is deliberately not an LLM. The retired inner-actor (Qwen3.6-27B / Hermes-2-Pro / Ollama / llama.cpp generative inference) was deleted on 2026-05-09 and is forbidden to reanimate. The outer agent (Claude / Codex / Cursor) is the actor; ME-JEPA is the *verifier*. The master ship gate is `prediction_oracle_correlation ≥ 0.95` stably across four rolling windows on the 300×8 SWE-bench Lite corpus — currently sitting at 0.866667.

---

## 2. Embedder strategy

### 2.1 ClipCannon — 5 dense storage spaces, 7-modality constellation

Storage:

| Space | Model | Dim | Storage table | What it captures |
|---|---|---|---|---|
| Visual | SigLIP-SO400M-patch14-384 | 1152 | `vec_frames` | Frame-level appearance, scene structure |
| Semantic | Nomic-embed-text-v1.5 | 768 | `vec_semantic` | Transcript meaning, topics |
| Emotion | Wav2Vec2-large-960h | 1024 | `vec_emotion` | Prosodic emotion, arousal, valence |
| Speaker | WavLM-base-plus-sv | 512 | `vec_speakers` | Voice identity for diarization |
| Voice ID | ECAPA-TDNN (Qwen3-Voice-Embedding) | 2048 | `voice_profiles` BLOB | Voice-clone identity verification |

Storage subtotal: **3,456-d across 4 vector tables in `sqlite-vec`**, plus a 2,048-d ECAPA blob for the voice identity gate.

The TCT (Teleological Constellation Training) whitepaper extends this to a 7-modality constellation totalling **4,044 dimensions**:

| # | Modality | Model | Dim | Captures |
|---|---|---|---|---|
| 1 | Visual | SigLIP-SO400M | 1152 | Face / pose / expression |
| 2 | Semantic | Nomic-embed-text-v1.5 | 768 | Spoken content / vocabulary |
| 3 | Emotion | Wav2Vec2-large-emotion | 1024 | Vocal emotion / arousal / valence |
| 4 | Speaker | WavLM-large | 512 | Voice identity / timbre |
| 5 | Prosody | Custom features | 12 | F0 / energy / rate / contour |
| 6 | Sentiment | all-MiniLM-L6-v2 | 384 | Sentence-level sentiment / intent |
| 7 | Voice | ECAPA-TDNN | 192 | Speaker-verification embedding |

Every embedder is **frozen by design**. They are off-the-shelf perception models (5 are explicitly named after their HuggingFace IDs). The selection is principled per modality but pragmatic: each modality picks the dominant well-trained encoder in its sub-field.

### 2.2 ContextGraph — 13 retrieval embedders (E1–E13)

The "labeller" panel that powers memory retrieval and DDA:

| Slot | Encoder | Dim | Notes |
|---|---|---|---|
| E1 | e5-large-v2 | 1024 | General semantic; the workhorse |
| E2–E4 | Sinusoidal / Fourier temporal | 512 each | Recency / position / chronology (weight-gated; algorithmic) |
| E5 | Nomic-embed asymmetric causal | 768 × 2 | Dual vectors: cause / effect (directional retrieval) |
| E6 | Sparse keyword | 30,522 sparse | Lexical recall |
| E7 | Qodo-Embed-1 | 1536 | Code-specific dense |
| E8 | e5-large-v2 structural | 1024 × 2 | Source / target dual for graph edges |
| E9 | HDC (hyperdimensional, 10,000-bit → 1024) | 1024 | Typo-robust binary then projected |
| E10 | e5-base-v2 paraphrase-asymmetric | 768 × 2 | Document / query dual |
| E11 | KEPLER (entity graph) | 768 | TransE-style entity geometry (currently disabled) |
| E12 | ColBERTv2 late-interaction | 128 per token | Token-level MaxSim |
| E13 | SPLADE v3 | ~30K sparse | Learned keyword expansion |

Total dense: 11,008 + variable late-interaction + two sparse vocabularies. **All 13 are frozen** (per PAPER §3.4 Axiom 1). Three are asymmetric (E5, E8, E10) — they store *dual* vectors for directional retrieval.

### 2.3 ME-JEPA-Code — 15 frozen instrument slots (5,120-d panel)

A separate panel built specifically for the SWE-bench code-correctness problem. This is structurally code-state-aware where ClipCannon's panel is perception-modality-aware:

| Slot | What it encodes | Dim |
|---|---|---|
| E_AST | tree-sitter AST encoder | 384 |
| E_CFG | control-flow graph attention | 256 |
| E_DataFlow | def-use graph attention | 256 |
| E_TypeGraph | type-signature + inferred-type graph | 256 |
| E_Test | e5-large-v2 on test descriptions | 384 |
| E_Trace | execution-trace sequence transformer | 512 |
| E_Diff | AST-diff edge encoder | 256 |
| E_Witness | witness-chain entry sequence | 256 |
| E_Oracle | one-hot test outcomes + exception class | 128 |
| E_Problem | e5-large-v2 on the SWE-bench problem text | 1024 |
| E_CommitMsg | e5-large-v2 on the candidate fix's commit message | 384 |
| E_StaticAnalysis | linter / typecheck / complexity aggregates | 256 |
| E_Runtime | wall-clock / RSS / coverage / network signal aggregates | 256 |
| E_Reasoning | reasoning/reflexion-trace encoder | 384 |
| Scalars | BFS depth, blame age, churn, coverage Δ, repo-health | 128 |

Total: **5,120 dimensions across 15 frozen instrument slots**.

The crucial difference vs ClipCannon's 7-modality constellation: most of these slots are **not pre-existing perception models**. Several are project-specific encoders (E_CFG, E_DataFlow, E_TypeGraph, E_Trace, E_Diff, E_Witness, E_StaticAnalysis, E_Runtime, E_Reasoning, Scalars). ClipCannon reuses the SOTA from each sub-field; ContextGraph builds purpose-specific instruments for the code-state domain because no SOTA exists for "encode the control-flow graph delta of a SWE-bench patch".

### 2.4 Embedder-count comparison summary

| Metric | ClipCannon | ContextGraph (labeller) | ME-JEPA-Code (predictor input) |
|---|---|---|---|
| Number of frozen embedders | 5 stored + 7 in constellation | 13 (E1–E13) | 15 (E_AST..Scalars) |
| Total dimensions | 4,044 (constellation) | ~11,008 dense + sparse + late-interaction | 5,120 |
| Where embedders come from | Off-the-shelf SOTA perception | Mix of SOTA + project-specific (E2–E4, E9 algorithmic) | Mostly project-specific (only E_Test/E_Problem/E_CommitMsg reuse e5) |
| Asymmetric embedders | None | E5, E8, E10 (dual cause/effect, src/tgt, doc/query) | None at the slot level |
| Storage backend | sqlite-vec (4× vec0 virtual tables) | RocksDB (59 CFs) + HNSW per embedder | RocksDB CFs + frozen Panel struct + cache shards |
| Pairwise interactions | Not first-class; implicit in highlights weights | First-class: cross-correlation across all 13 → C(13,2) = 78 pairwise signals (PAPER §3.1) | First-class: per-slot pairwise tensors (#388 / `task-py-g-088`) |

---

## 3. Fusion strategy — the deepest divergence

This is where the two architectures diverge most sharply.

### 3.1 ClipCannon — weighted late fusion (compensable)

The headline cross-modal mechanism is the `highlights` pipeline stage. From `src/clipcannon/pipeline/highlights.py`:

```python
W_EMOTION = 0.25
W_REACTION = 0.20
W_SEMANTIC = 0.20
W_NARRATIVE = 0.15
W_VISUAL_VARIETY = 0.10
W_QUALITY = 0.05
W_SPEAKER = 0.05
```

Each 30-second candidate window gets a **scalar composite score** that is a weighted sum across the 7 signals. The whitepaper says the same explicitly:

> "moments that score highly across multiple modalities … are ranked above moments that are strong in only one dimension."

That phrasing sounds like conjunction but the *implementation* is additive: a high emotion score can compensate for a weak quality score, and vice versa. The fusion is at the *score level*, not the embedding level. The 5 vec-tables themselves are queried independently in `clipcannon_search_content` (KNN over one space at a time) — there is no cross-space ranking fusion in the live retrieval path.

The Constellation Guard (`src/phoenix/video/constellation_guard.py`) does enforce a per-modality threshold check, but at runtime it runs **single-modality** on every generated frame: re-embed through SigLIP → cosine against visual centroid → accept if ≥ 0.70. The multi-modal conjunction described in PAPER §5.4 is *aspirational* for ClipCannon at the per-output level — the audio modalities (speaker, voice) are validated against centroids in a separate path, and there is no atomic `forall m : cos(phi_m(yhat), c_m) ≥ tau_m` predicate over a generated *frame*.

### 3.2 ContextGraph — RRF at retrieval; strict conjunction at verification

Two separate fusion regimes:

**At retrieval** (across the 13 E1–E13 panel): **Reciprocal Rank Fusion** with a weighted profile. From PAPER §4.1:

> "Retrieval is a weighted Reciprocal Rank Fusion (RRF; Cormack et al., 2009) over per-embedder rankings. The default MultiSpace strategy runs all HNSW-indexed embedders in parallel with a profile-selected weight vector (K = 60). A pipeline variant (E13 SPLADE → E1 Matryoshka 128-D → MultiSpace rerank → K-NN graph expand → E12 MaxSim rerank) targets sub-60 ms latency at a million memories."

RRF is rank-based, not score-based, and it's late fusion — but it's used for *retrieval ranking*, which is a soft preference. Compensation is allowed and expected (E9 typo-robust can rescue a query that E1 semantic misses).

**At verification** (across the 15 ME-JEPA instrument slots): **strict per-slot cosine conjunction**. From `crates/context-graph-mejepa-tct/src/gtau.rs:14`:

```rust
pub fn gtau_check(
    predicted_panel: &Panel,
    predicted_class: MutationCategory,
    language: Language,
    entity_type: EntityType,
    constellation: &TctConstellation,
) -> Result<GtauOutput, TctError> {
    let mut violations = Vec::new();
    for embedder in EmbedderId::all() {
        let slot = panel_slice_for_embedder(predicted_panel, embedder)?;
        let (centroid, origin, _n) = constellation.lookup_centroid(...);
        let observed = cosine_similarity(slot, centroid)?;
        let threshold = constellation.threshold(embedder, Some(entity_type))?;
        if observed < threshold {
            violations.push(GtauViolation::try_new(
                embedder, observed, threshold, None, origin,
            )?);
        }
    }
    GtauOutput::try_new(violations, ...)
}
```

Every slot must clear its own per-class, per-language, per-entity-type threshold. If any one slot fails, the panel violates Gτ. This is the `forall m : cos(phi_m(yhat), c_m) >= tau_m` predicate from PAPER §5.4 — exactly the predicate the ClipCannon whitepaper *describes* but does *not* enforce as an atomic per-output runtime check across all 7 modalities.

This conjunction-vs-compensation difference is also why the active P0 issue #385 ("Refactor predictor to slot-preserving per-embedder constellations") matters so much in the current Phase G work. The original predictor (`crates/context-graph-mejepa/src/predictor/mod.rs`) concatenates `panel_t0` and `panel_t1` into a single 10,240-d input and passes them through `linear(CONCAT_INPUT_DIM, hidden_dim)`. That fusion *allows* slot compensation by construction, which is the architectural error the slot-preserving refactor (#386 / #387 / #388 / #389 / #390) is fixing. Once #385's children land, ME-JEPA-Code will be a true strict-conjunction system end-to-end. ClipCannon never made this architectural commitment in the first place — its identity-locking is single-modality at runtime by design, plus a soft audio side-check.

### 3.3 What this means in practice

| Scenario | ClipCannon outcome | ContextGraph outcome |
|---|---|---|
| Generated frame looks like Santa (high SigLIP) but voice is wrong (low WavLM) | Frame passes the visual guard; audio is checked separately at clip level — *can desync* | Patch panel rejected — speaker slot violation surfaces with `embedder=Speaker` annotation |
| Code patch passes all tests (high E_Oracle) but type-graph drifts (low E_TypeGraph) | (not applicable) | Patch rejected; `predicted_failure_modes` ranks `E_TypeGraph` as the violating embedder |
| Long-form interview scores high on emotion but low on semantic density | Highlights composite still surfaces it as best moment if 0.25 × emotion > 0.20 × semantic_loss | (not applicable) |

The conjunction discipline is more expensive to satisfy (every modality must clear), more brittle to threshold mis-calibration, and gives you stronger per-output safety. The weighted sum is easier to ship, easier to tune, and gives you a smooth ranking signal. They are right tools for different jobs.

---

## 4. Storage and retrieval

| | ClipCannon | ContextGraph |
|---|---|---|
| Backend | SQLite + `sqlite-vec` extension (one DB per project) | RocksDB (single store, 59 column families) |
| Vector indexes | 4× `vec0` virtual tables; KNN via sqlite-vec's MATCH operator | HNSW per embedder; up to 15 indexes when learner / matryoshka variants are active |
| Query primitive | Single-space KNN; cross-modal join via timestamp metadata | RRF over multi-space rankings; typed-edge graph traversal; causal-direction-aware E5 |
| Mutation model | Per-project DBs; project deletion = directory deletion; no global graph | Global graph spanning every memory; merges + soft-delete + tombstones; importance boosts; consolidation |
| Provenance | SHA-256 hash chain in `provenance` table; `verify_chain()` walks chronologically | Append-only audit log CF (`CF_AUDIT_LOG`); `get_audit_trail` / `get_merge_history` / `get_provenance_chain` MCP tools; per-entity provenance with TransE scores |
| Per-modality direction | Symmetric cosine on every space | Asymmetric on E5/E8/E10 (cause/effect, src/tgt, doc/query stored separately) |
| Memory durability | Per-project file in `~/.clipcannon/projects/<id>/analysis.db` | Production durable root on aiwonder `/zfs/archive/contextgraph/...` enforced fail-closed (#363/#364/#365) |

The SQLite-per-project model is structurally fine for ClipCannon because each project is self-contained: one source video, one analysis, one set of edits. There is no need to query across projects (and `analysis.db` is even classified as "Sacred" tier in the disk-cleanup policy).

ContextGraph is structurally cross-project: memories accumulate, merge, and link over time. The single RocksDB + 59-CF design supports merges, importance boosts, soft deletes, and audit trail in a way that 1000 sqlite-vec databases would not.

---

## 5. Verification / alignment primitive

This is the second-deepest divergence.

### 5.1 ClipCannon — "stay near the centroid; otherwise reject the frame"

The Teleological Constellation System has three phases (TCT whitepaper §3 / §6 / §7):

1. **Phase 1 — Construction.** Pass 16 minutes of Santa video through the 7 frozen embedders. Compute L2-normalized mean per modality. Store 7 centroids in `~/.clipcannon/models/santa/embeddings/all_embeddings.npz`.
2. **Phase 2 — LoRA conditioning.** Fine-tune EchoMimicV3 (1.86B DiT) with mouth-weighted flow-matching, conditioned on the 8-state behavioral constellation and 34-skill expression hierarchy.
3. **Phase 3 — Runtime guard.** Re-embed every generated frame through frozen SigLIP. Cosine against visual centroid. Reject if < 0.70. Visual severity tiers: WARN at 0.80, STRONG at 0.90.

The guard is a **hard binary boundary**: pass or reject. The alignment property the whitepaper claims is *per-frame verifiability without ongoing human feedback*. This is genuinely valuable; it is the architectural answer to the question "how do I know my LoRA hasn't drifted off Santa over the course of generation?". And it works: ClipCannon's audio centroid + best-of-12 best-of-K + threshold 0.95 anchored the headline **mean WavLM SECS 0.961** on 10 held-out English sentences in PAPER §6.3.

### 5.2 ContextGraph / ME-JEPA-Code — "predict the consequence, calibrate the verdict, ship at 0.95 oracle correlation"

ME-JEPA's primitive is `RealityPrediction` — not a binary accept/reject but a structured verdict with calibrated confidence intervals:

- **Verdict**: one of `Pass`, `Fail`, `OutOfDistribution`, `Abstain`, `GuardRejected`.
- **Predicted oracle outcome** with conformal interval.
- **Predicted failure modes** ranked by likelihood (Q3).
- **Predicted side effects** replayed against the durable shift log (Q5).
- **Always-on heads** for edge cases / latent bugs / tech debt / perf / security / accuracy / cost (Q4).
- **Slot-level evidence** — which embedders agreed, which disagreed, closest exemplars in the corpus.
- **Reality impact** scoring how the patch would shift durable repo state.

The ship gate is *not* a per-prediction threshold; it is `prediction_oracle_correlation >= 0.95` stably across four rolling windows on the 300×8 SWE-bench Lite Python corpus. The system has to be *calibrated against a real Docker test harness* across 300 problems × 8 mutation categories, not just identity-anchored to one named subject.

The Gτ guard (the strict-conjunction predicate from §3.2 of this doc) is *one* component of the verdict, not the verdict itself. Gτ rejection becomes `Verdict::GuardRejected`; oracle disagreement becomes `Verdict::Fail`; conformal interval blowing up becomes `Verdict::OutOfDistribution`; conformal interval not clearing the calibration floor becomes `Verdict::Abstain`. The whole verdict lattice lives behind a calibrated confidence gate.

This is structurally a **different alignment shape**:

| Dimension | ClipCannon TCT guard | ME-JEPA RealityPrediction |
|---|---|---|
| Granularity of decision | Per generated frame | Per candidate patch |
| Output shape | Boolean accept/reject + cosine trace | Structured 5-question verdict with intervals |
| Target | Single named identity (Santa, Boris, …) | Generic predicate across every SWE-bench task |
| Calibration | Per-modality threshold; same-session-band floor | Conformal intervals; per-cell calibration; OOD heads; consequence replay |
| Failure mode behavior | Regenerate / fall back / escalate | Surface to operator with ranked failure modes; never auto-act |
| Truth signal | Frozen centroid distance | Real Docker test harness (oracle), AST/CFG/data-flow state, witness chain |
| Ship gate | Cosine ≥ τ per modality | Oracle correlation ≥ 0.95 across 4 rolling windows × (mutation × language) |

In a slogan: **ClipCannon orbits an identity; ME-JEPA predicts a consequence.** Both use frozen embedders as the substrate, but the verb the substrate is used for is different.

---

## 6. Generation vs verification posture

| | ClipCannon | ME-JEPA-Code |
|---|---|---|
| Is there a trained generator? | Yes — EchoMimicV3 LoRA, Qwen3-TTS, Gemma-4 SFT/DPO for the Shakespeare style case | No — explicitly retired; the AI coding agent is the actor |
| Who generates? | ClipCannon, via the AI assistant's MCP calls | The external LLM (Claude / Codex / Cursor) generates patches; ME-JEPA only predicts whether they work |
| Inner loop? | Yes — LoRA training; best-of-K sampling; multi-gate verification loop | No inner LLM; only the predictor + heal scheduler + consolidation |
| Outer loop? | Operator + AI assistant deciding what to render | Single outer loop around the invoking coding agent |
| Trainable params today | LoRA adapters (rank 256, α 512, ~1.2 GB safetensors) + audio LoRA + dashboards | Predictor (multi-head) + heads (always-on Q4) + per-slot OOD calibration |
| Frozen vs trainable | All embedders frozen; LoRA trainable on generator | All instruments frozen; predictor + heads trainable; embedders never trained in the live loop |

This is the structural answer to "why are clipcannon's results so much more polished?" — clipcannon has *something to generate*, so the multi-embedder substrate is wrapped in a creative product. ME-JEPA has *something to predict*, so the multi-embedder substrate is wrapped in a verifier. They are not the same machine.

---

## 7. Where the architectures line up

Despite the different verbs, the systems share substantial DNA. Both:

1. **Treat embedders as frozen measurement instruments**, not learnable adapters. Neither updates the embedder weights in the live training loop.
2. **Decompose meaning into multiple independent spaces** rather than concatenating into a single learned representation. ClipCannon's 7 modalities ≈ ContextGraph's 13 retrieval slots ≈ ME-JEPA's 15 instrument slots all instantiate the same DDA bet: that N approximately-independent perception models capture more of "what matters" than any single backbone.
3. **Use L2-normalized centroids as the alignment target.** Both systems compute centroids by L2-normalizing per-sample, averaging, then L2-normalizing the mean. Both then test cosine similarity against the resulting unit-vector centroid. The math is *identical* — only the conjunction discipline differs.
4. **Apply runtime per-output threshold checks.** ClipCannon's Constellation Guard (single-modality visual) and ME-JEPA's Gτ guard (15-slot strict conjunction) are both deterministic O(M·d_max) tests against frozen centroids with no auxiliary learned model. PAPER §3.4 calls this "the three-role composition: retriever, labeller, runtime guard."
5. **Provide tamper-evident provenance.** Clipcannon's SHA-256 hash chain in the `provenance` table and ContextGraph's append-only audit log column family (`CF_AUDIT_LOG`) play the same role: forensic verifiability of every operation.
6. **Expose everything via MCP.** 54 tools (ClipCannon) vs 75 tools (ContextGraph). Both treat the AI assistant as the operator and use stdio MCP as the only sanctioned action surface.
7. **Run locally on consumer hardware.** Both are designed for RTX 5090 / 32GB VRAM class, both have precision auto-detection, both have hard offline rules (`HF_HUB_OFFLINE=1` / model-pre-cache).
8. **Forensic-fix culture.** ClipCannon documents 17 failed approaches across 6 days (TCT whitepaper §10); ContextGraph documents the flow-matching sign bug, five companion fixes, and ongoing Sherlock-style FSV audits. The "assume code is guilty until proven innocent" forensic doctrine is the same.
9. **Live constellation language.** The word "constellation" comes from the same author and means the same thing in both repos: a set of frozen per-modality centroids that together define an identity / class / state.

---

## 8. Where the architectures diverge

| Axis | ClipCannon | ContextGraph |
|---|---|---|
| Output type | Generative artifacts (video, audio, edits) | Predictive verdicts (Pass/Fail/OOD/Abstain) |
| Target shape | Single named identity per project | Generic predicate across every SWE-bench task |
| Fusion at the decision point | Weighted late-fusion sum (`highlights`); single-modality guard at runtime | Strict per-slot cosine conjunction (`gtau_check`); RRF only at memory-retrieval ranking time |
| Embedder selection | Off-the-shelf SOTA per modality | Mix: a few SOTA reuses + many project-specific encoders (E_CFG, E_DataFlow, E_TypeGraph, E_Trace, E_Diff, E_Witness, E_Reasoning…) |
| Number of embedders | 5 stored / 7 in constellation | 13 (retrieval) + 15 (instrument panel) |
| Storage | sqlite-vec per project | RocksDB single store with 59 CFs |
| Asymmetric embeddings | None | E5 / E8 / E10 store dual vectors |
| Pairwise interactions | Not first-class | First-class: 78 pairwise correlations at N=13 + cross-embedder constellation tensors (#388) |
| Calibration | Per-modality cosine threshold | Per-cell conformal intervals; per-slot OOD reasons; Q4 confidence calibration gate |
| Ship gate | Operator judgement + per-frame guard | `prediction_oracle_correlation ≥ 0.95` across 4 rolling windows × cell |
| Inner LLM | Yes (Qwen3-8B for narrative, Qwen3-14B for VoiceAgent reasoning) | None — explicitly retired |
| Verification ground truth | Re-embedding through frozen perception model | Real Docker SWE-bench oracle harness |
| Test scale | 994 tests | 5,184+ tests |
| Programming language | Python 3.12 | Rust workspace (370K LOC) |
| Workload character | Long-running batch (analysis takes minutes; render takes more) | Online predictor (sub-second verdicts after panel materialization) |
| Cross-modal binding | Timestamp join across SQL tables | Typed edges in RocksDB graph; PageRank-derived foundationality scalar |
| Memetic state | Not modeled (per-project, fresh each time) | First-class: importance boosts, soft delete, merges, consolidation, EWC++ Fisher protection on foundational chunks |

---

## 9. Pros / cons of each strategy

### 9.1 ClipCannon's weighted-late-fusion + single-modality guard

**Pros**

1. **Ships.** The composite score is easy to tune, smooth to optimize, and produces obviously-better-than-nothing rankings for highlights. No threshold calibration crisis.
2. **Smooth ranking signal.** Editors can iterate; the weighted score gives a continuous ranking rather than a hard accept/reject. Good UX for human-in-the-loop creative work.
3. **Per-frame guard is fast and tight.** A single SigLIP forward pass + cosine check is ~10ms on the RTX 5090; runs at 25fps with 25% overhead.
4. **The verbal claim ("multi-modal alignment") is intuitive.** Easy to explain to users that "we check the face *and* the voice *and* the emotion".
5. **Off-the-shelf encoders mean fast iteration.** Each modality picked the dominant SOTA encoder; no project-specific training is required to start.

**Cons**

1. **Compensation is silent.** A weighted sum can be satisfied by a single strong signal masking 6 weak ones. The whitepaper *describes* multi-modal conjunction but the implementation is additive.
2. **Single-modality runtime guard is a defense in depth illusion when only one modality is actually checked per frame.** The other 6 modalities are validated against centroids at different layers (audio chunk, voice clip) but never *atomically* with the visual frame they decorate.
3. **Per-modality threshold calibration is per-subject.** Add a new person; recalibrate every threshold against their natural variation.
4. **No first-class pairwise interaction.** ClipCannon doesn't compute the C(7,2)=21 cross-modal correlations explicitly; they're implicit in the highlights weights. This means "tests agree but runtime disagrees" or "AST/diff agree but oracle disagrees" patterns — first-class signals in ME-JEPA — are not surfaced.
5. **Identity targets are per-subject.** Cloning a new person requires a new constellation, a new LoRA, new training data, new calibration. There's no generic predicate that works across arbitrary subjects.

### 9.2 ContextGraph / ME-JEPA's RRF-at-retrieval + strict-conjunction-at-verification

**Pros**

1. **No silent compensation.** `gtau_check` rejects the panel if any one slot fails its threshold. A slot-preserving predictor (once #385 lands) means the trained model cannot learn to trade one slot off against another.
2. **Pairwise / disagreement signals are first-class.** #388 materializes cross-embedder correlation tensors; #389 produces per-slot OOD reasons; #390 (in progress) adds consensus / contradiction heads that detect "tests agree but runtime disagrees" patterns directly.
3. **Calibration is per-cell, not per-subject.** ME-JEPA must generalize across every SWE-bench task. The calibration story is conformal intervals + per-(mutation × language × entity_type) thresholds + OOD-rate guards.
4. **Ground truth is harder than centroid distance.** A real Docker test harness either passes or fails; you can't fool it by being visually similar to a passing patch. The 0.95 ship gate is an external falsification standard.
5. **Asymmetric embedders for directional retrieval.** E5 cause/effect, E8 src/tgt, E10 doc/query are structurally tuned for the kinds of code-causality queries ME-JEPA needs.
6. **Forensic discoverability.** A verdict comes with `predicted_failure_modes`, violating embedders, closest exemplars, and the cosine trace — the operator can ask *why* and get a structured answer.

**Cons**

1. **Strict conjunction is brittle.** Every slot has to clear its threshold; one bad calibration ruins the whole panel verdict. The current 0.866667 gate is partly a calibration problem (the model is sometimes right but the conformal intervals are still too wide).
2. **Far more engineering to satisfy.** 15 project-specific instrument slots, 51+ RocksDB CFs, per-cell conformal calibration. Most of this machinery has no direct analogue in ClipCannon because ClipCannon's problem doesn't *need* it.
3. **No generative product.** ME-JEPA can tell you whether a patch is good; it cannot generate a better patch. The AI agent (Claude, Codex) does that. So the substrate is invisible to most users.
4. **Slot-preserving discipline is recent.** #385's flat-fusion path was the architecture as recently as 2026-05-18; the entire current Phase G push is the slot-preserving correction. ME-JEPA was, until very recently, susceptible to the same silent-compensation criticism levied at ClipCannon above.
5. **Ship gate is far harder to hit.** Cosine similarity ≥ 0.70 visual against one identity is achievable on day 3. `prediction_oracle_correlation ≥ 0.95` stably across 4 windows on 2,400 task instances is not.
6. **The substrate is harder to reuse outside the SWE-bench code domain.** ClipCannon's 7-modality stack works for any subject. ME-JEPA's 15-slot panel is specifically tuned to code-change-state.

### 9.3 When to use which strategy

| If your problem is… | Pick this strategy |
|---|---|
| Identity-locked generation against one or a small number of named targets | ClipCannon-style weighted fusion + per-modality guard. The named-target framing makes threshold calibration tractable and the generative product easy to evaluate (does it look/sound like them?). |
| Generic-predicate verification across arbitrary inputs with a falsifiable external oracle | ME-JEPA-style strict conjunction + conformal intervals + per-cell calibration + oracle-driven ship gate. The genericity demands the strict discipline; the external oracle gives you a real ground truth that doesn't require human judgement. |
| Information retrieval / memory recall where soft-preference ranking is acceptable | RRF-style late fusion with per-embedder weight profiles. Compensation across embedders is a *feature* here — E9 typo-robust rescuing E1 semantic is good behaviour. |
| Real-time per-frame defense in depth on a single modality | Single-modality runtime guard, as ClipCannon does. 10ms/frame is the right budget for a 25fps stream. |
| Per-output structured-verdict explanation with calibrated confidence | ME-JEPA-style multi-head predictor with conformal intervals + violating-embedder annotations. Explanations require slot-preserving structure. |

---

## 10. What each system could learn from the other

### 10.1 What ME-JEPA-Code could take from ClipCannon

1. **Behavioural-state hierarchy as a structured prior.** ClipCannon's 4-level expression hierarchy (Action Units → Groups → Skills → Constellations) is genuinely novel and architecturally clean. The analogous ME-JEPA construct would be a hierarchy of *failure modes*: low-level signals (AST violation, type-graph drift, dataflow break) → groups (compositional-generalization failure, environment-mismatch failure) → skills (`patch_breaks_test_isolation`, `patch_introduces_dead_code`) → constellations (`risky_security_change`, `safe_refactor`). #390's consensus/contradiction work is moving in this direction; making the hierarchy explicit would crystallize the Q3 / Q4 surfaces.

2. **Per-output structured-verdict explanation as the front door, not the back door.** ClipCannon's `clipcannon_find_best_moments` surfaces the *score breakdown* and *why* in the response. ME-JEPA's `RealityPrediction` is already structured but most surface tools return JSON blobs. A `mejepa_explain_prediction` tool that returns a hierarchical narrative ("This patch fails because: (1) test isolation broke — `pytest test_django` no longer passes solo; (2) the AST diff touches `Model.save()` which is called by 17 other tests; (3) historically, similar diffs to Model.save have a 73% repro-on-CI rate.") would be a major UX improvement.

3. **Best-of-K + cosine-scored selection as a generic verification primitive.** Case 3 in PAPER §6.3 (50-clip centroid + best-of-12 + WavLM scoring) is essentially "pick the best of N candidates by their distance to a frozen centroid". The analogous ME-JEPA primitive — `mejepa_rank_candidates` ranking K alternative patches by their predicted oracle outcome — already exists but is under-exposed. Promoting it as a first-class MCP tool that AI agents call before committing to a patch would change how agents work with ME-JEPA.

4. **Constellation-blend transitions for behavioural change.** ClipCannon's `transition_to(new_constellation, duration_s=0.5)` blends prompts during transitions. The ME-JEPA analogue would be confidence-interpolation during cell-boundary cases: when a patch sits between two mutation-category cells, blend the conformal intervals rather than picking the closest cell.

5. **The forensic-discipline doctrine is mature on the clipcannon side and *should be ported into the ME-JEPA agent prompts* explicitly.** The 6 Sherlock blockers from the flow-matching sign-bug investigation are documented across `docs/inspect/01–10`. The ME-JEPA Phase G work has analogous bugs (e.g. Q5 replay consequence semantics, fixed in `27c8e8f0` last night). A `docs/inspect/` directory in contextgraph with the same forensic-report culture would help future agents inherit the investigation pattern.

### 10.2 What ClipCannon could take from ME-JEPA-Code

1. **Strict per-modality conjunction for the constellation guard.** The whitepaper claims multi-modal verification but the implementation runs single-modality. Promoting Gτ from aspirational to enforced — re-embedding the full audio+video tuple through all 7 modalities and requiring every cosine to clear its threshold per generated chunk — would make the alignment property the paper claims actually hold. The cost is 4-7× more embedder forward passes per chunk; the benefit is a real "every output is in the manifold" guarantee.

2. **First-class pairwise cross-modal interaction features.** ContextGraph stores 78 pairwise correlations per memory (N=13 → C(13,2)). ClipCannon could store 21 pairwise correlations per video chunk (N=7 → C(7,2)). The signals that matter for highlight scoring — "emotion is high, audience reaction is high, *and the two co-occur*" — are exactly the kind of pairwise-conjunction signal that a weighted sum smears.

3. **Calibration discipline.** ClipCannon's per-modality thresholds (0.70 visual, 0.60 emotion, 0.80 speaker, 0.85 voice) are stated as "calibrated against the Santa dataset's natural variation" but the calibration procedure is not formal. Importing ME-JEPA's same-session-band-floor calibration (PAPER §5.4: collect a disjoint calibration set, compute cosines against centroid, set τ at 10th percentile) and *logging the calibration provenance* per identity would make the threshold story auditable.

4. **Asymmetric embeddings for directional retrieval.** ClipCannon's `clipcannon_search_content` is symmetric — query and document both get encoded the same way. For RAG-style retrieval over meeting transcripts (OCR Provenance RAG, per README), an asymmetric document/query encoder like ContextGraph's E10 would improve recall by ~5-10% on standard MTEB benchmarks.

5. **Out-of-distribution detection per modality.** ME-JEPA #389 produces `per_slot_ood_reasons` — a structured list of which slots are out of their calibration distribution. ClipCannon's guard currently knows that a frame is *below threshold*; it doesn't know whether the *reason* is "the speaker's emotion is unusual for this constellation" vs "the speaker is in a weird visual pose". Adding per-modality OOD reasons would help the Phoenix avatar engine decide whether to regenerate, fall back, or escalate.

6. **A single RocksDB-or-KV store for cross-project memory.** The per-project SQLite model is fine for the analysis pipeline but breaks down for cross-project queries ("show me all moments across all my projects where Santa laughs genuinely"). The OCR Provenance RAG already wants this; a unified ContextGraph-style typed-edge graph store would extend ClipCannon from "video understanding pipeline" to "knowledge graph of all my video content."

7. **Foundationality-graded importance.** ContextGraph's PageRank-derived `psi_found(x_i)` (PAPER §3.2bis) gives load-bearing chunks more sampling weight and more EWC++ protection. The clipcannon analogue would be giving scene-key-frames (heavily referenced as crop anchors) more centroid weight than incidental B-roll frames. The current centroid is an unweighted mean, which means a 5-second silent reaction shot contributes the same to the visual centroid as a 5-second key conversation.

8. **An external oracle.** ClipCannon's only ground truth is "the AI assistant or user agrees the output is Santa." That is the same RLHF-style preference-based ground truth that PAPER §11 explicitly contrasts TCT against. ME-JEPA's SWE-bench Docker oracle is genuinely external. The closest ClipCannon analogue would be a hold-out human evaluation panel + a published cross-encoder SECS triangulation (the PAPER §6.3 named-future-work item: ECAPA-TDNN cross-encoder triangulation). Closing the cross-encoder gap would convert the "encoder-matched 0.961 SECS" headline from a within-WavLM-family result to a genuinely cross-encoder identity guarantee.

---

## 11. Architectural verdict

The two systems are not in competition. They are two instantiations of the **same DDA bet** described in PAPER §3.4: that the same frozen embedder panel can serve as retriever, labeller, and runtime guard simultaneously, without parameter updates.

- **ClipCannon** is the **labeller + retriever + identity-guard** instantiation. It uses 5–7 frozen embedders to label a single source video into a queryable database, retrieves moments from that database to compose edits, and at generation time guards every output frame against a frozen 7-modality identity centroid. It is a **generative product** built on top of a multi-embedder substrate. The thing it sells is the rendered video.

- **ContextGraph / ME-JEPA-Code** is the **labeller + retriever + consequence-predictor** instantiation. It uses 13 frozen embedders to label every memory across all projects into a single RocksDB graph, retrieves over that graph via RRF + typed edges + causal-direction-aware E5, and at decision time uses a separate 15-slot frozen instrument panel to predict the consequence of an external AI agent's proposed code change. It is a **verifier product** built on top of a multi-embedder substrate. The thing it sells is the calibrated verdict.

What both systems prove together is that the multi-embedder strategy works for both shapes of problem — generative identity-locked output and predictive consequence-anticipation. What they differ on is *the verb the substrate is used for*, and that verb dictates the fusion discipline (compensable vs strict-conjunction), the calibration discipline (per-identity vs per-cell-conformal), and the ship gate (operator judgement vs external oracle correlation).

The current Phase G work on ContextGraph (#385 + its children #386 / #387 / #388 / #389 / #390) is exactly the move that makes the contextgraph system honour the strict-conjunction discipline that PAPER §5.4's Gτ predicate has been claiming all along. Once that lands and `prediction_oracle_correlation` clears 0.95, the two systems will share a *symmetric* relationship: both with frozen embedders, both with cosine guards, both with provenance — one orbiting an identity, the other orbiting an oracle.

---

## Appendix A. Concrete code anchors

| Claim | Location |
|---|---|
| ClipCannon `vec_frames` schema (1152-d SigLIP) | `/home/cabdru/clipcannon/src/clipcannon/db/schema.py:378` |
| ClipCannon `vec_semantic` schema (768-d Nomic) | `/home/cabdru/clipcannon/src/clipcannon/db/schema.py:385` |
| ClipCannon `vec_emotion` schema (1024-d Wav2Vec2) | `/home/cabdru/clipcannon/src/clipcannon/db/schema.py:392` |
| ClipCannon `vec_speakers` schema (512-d WavLM) | `/home/cabdru/clipcannon/src/clipcannon/db/schema.py:399` |
| ClipCannon highlight weight constants (weighted sum) | `/home/cabdru/clipcannon/src/clipcannon/pipeline/highlights.py:40-46` |
| ClipCannon visual scene-boundary threshold (0.85 cosine) | `/home/cabdru/clipcannon/src/clipcannon/pipeline/visual_embed.py:50` |
| ClipCannon Constellation Guard description | `/home/cabdru/clipcannon/docs/clipcannon_teleological_constellations_whitepaper.md` §6.1–6.4 |
| ContextGraph 15-slot `InstrumentSlot` enum (5,120-d panel) | `/home/cabdru/contextgraph/crates/context-graph-mejepa-instruments/src/lib.rs:173-205` |
| ContextGraph 15-slot layout / dims | `/home/cabdru/contextgraph/crates/context-graph-mejepa-instruments/src/lib.rs:210-230` |
| ContextGraph Gτ strict-conjunction predicate | `/home/cabdru/contextgraph/crates/context-graph-mejepa-tct/src/gtau.rs:14-55` |
| ContextGraph cosine_similarity (fail-closed on zero-norm / NaN) | `/home/cabdru/contextgraph/crates/context-graph-mejepa-tct/src/gtau.rs:90-139` |
| ContextGraph TCT constellation centroid lookup | `/home/cabdru/contextgraph/crates/context-graph-mejepa-tct/src/constellation.rs` |
| ContextGraph 13-embedder panel description | `/home/cabdru/contextgraph/docs/PAPER.md` §4.1 |
| ContextGraph predictor flat-concat fusion (the slot-preserving refactor target) | `/home/cabdru/contextgraph/crates/context-graph-mejepa/src/predictor/mod.rs:130-141` |
| Current P0 slot-preserving refactor issue | GitHub `#385` (in-progress 2026-05-19) |
| Current ship-gate value | `prediction_oracle_correlation = 0.866667 < 0.95` (PAPER §6.4; GitHub `#310` Phase G context) |

---

## Appendix B. Quick numeric comparison

| Metric | ClipCannon | ContextGraph |
|---|---|---|
| Source LOC | ~67,585 (Python) | ~370,000 (Rust) |
| MCP tools | 54 | 75 |
| Tests | 994 | 5,184+ |
| Embedders (storage) | 5 dense | 13 (E1–E13) + 15 (instrument panel) |
| Total dims (panel) | 4,044 (7-modality TCT) | 5,120 (15-slot instrument panel) |
| Asymmetric embedders | 0 | 3 (E5, E8, E10) |
| Vector storage | sqlite-vec, 4× vec0 virtual tables | RocksDB + HNSW + 59 CFs |
| Pipeline stages | 23-stage DAG | predictor + always-on Q4 heads + heal scheduler + eval |
| Inner LLM | Qwen3-8B (narrative) + Qwen3-14B (VoiceAgent) | None (retired 2026-05-09) |
| Subject scope | 1 named target per project | Generic across SWE-bench Lite (300 tasks × 8 mutation categories) |
| Headline measured number | mean WavLM SECS = 0.961 on 10 sentences (PAPER §6.3) | prediction_oracle_correlation = 0.866667 (target ≥ 0.95) |
| Calibration set | 50 audio clips (one subject) | 300×8 SWE-bench Lite corpus + per-cell calibration |
| Provenance | SHA-256 hash chain | Append-only RocksDB CF_AUDIT_LOG + Merkle witness |
| Durable SoT | `~/.clipcannon/projects/<id>/analysis.db` | aiwonder `/zfs/archive/contextgraph/...` |

---

*End of comparison report.*
