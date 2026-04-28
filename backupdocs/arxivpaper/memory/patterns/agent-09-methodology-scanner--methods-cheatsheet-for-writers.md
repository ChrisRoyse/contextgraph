---
namespace: patterns
agent_id: agent-09-methodology-scanner
timestamp: 2026-04-16T18:45:00Z
status: active
tags: cheatsheet, writer-handoff, agent-37, methodology-writer, per-case-study, comparator-methods
depends_on:
  - memory/discoveries/agent-09-methodology-scanner--methodology-scan.md
  - memory/discoveries/agent-09-methodology-scanner--methodological-gaps.md
  - memory/patterns/agent-09-methodology-scanner--standard-metrics-per-cluster.md
  - memory/patterns/agent-08-citation-extractor--claim-to-citation-index.md
  - memory/decisions/agent-08-citation-extractor--master-reference-pool.md
downstream_consumers:
  - agent-37-methodology-writer
  - agent-38-results-writer
  - agent-39-discussion-writer
  - agent-42-adversarial-reviewer
---

# Methods Cheatsheet for Writers — Single-Page Handoff to Agent-37

**Audience.** agent-37 (methodology-writer), agent-38 (results-writer),
agent-39 (discussion-writer).

**Purpose.** For each of the three case studies, name the **comparator methods**
from the cited literature that §5 / §6 must benchmark against, the **metrics**
that must be reported in canonical phrasing, the **ablations** that must be run,
and the **methodological novelties** that must be claimed (and evidenced).

**Rule.** Cite by key only. Every key below is in `master-reference-pool.md`. If
you believe a comparator is missing, file a blocker; do not invent.

---

## Case 1 — Shakespeare stylistic LoRA (§6.1, **T2 + T3 projected**)

### Comparators to describe (in §2.6 / §5 / §6.1)

| Role | Keys | Why we compare |
|---|---|---|
| **Primary method substrate** | `hu2022lora` (LoRA low-rank adaptation) | Case 1 uses LoRA on a base LM; the substrate is identical. |
| **Primary alignment comparator (scalar-reward)** | `ouyang2022instructgpt`, `rafailov2023dpo` | Case 1 uses SFT + DPO; these are the Cluster-I canonical methods. Our delta is that DPO is applied on top of a cleaned corpus and (projected T3) under a text-modality constellation guard. |
| **Adjacent alignment framing** | `bai2022constitutional` | Mention in §2.6 as part of the Cluster-I triad; CAI is AI-feedback-based and orthogonal to our geometric alignment. |
| **Forensic transparency lineage** | (no cited comparator — this is METHOD-GAP-8) | Our echo-bug forensic is methodologically novel in this slot; disclose file path per P10. |

### Metrics to report

Required (from standard-metrics-per-cluster.md, Cluster I + C):

- **T2** — Pipeline description: cleaned-corpus SFT → DPO pipeline. LoRA rank
  and training-set size. The 15/15 FSV echo-bug post-fix verification (forensic
  claim).
- **T3 projected** (V22 flag mandatory — "(projected; camera-ready will
  re-verify)"):
  - Held-out perplexity (cross-entropy; Cluster I reporting convention).
  - Style-transfer cosine to Shakespeare-text centroid (our projected metric).
  - Constellation-guard accept-rate at text modality.
  - Optional: MMLU / TruthfulQA sanity-check post-alignment (Cluster I sanity
    baseline; do not over-report — Case 1 is style LoRA, not general-capability).

### Ablations (required for §6.1)

1. **SFT-only vs. SFT + DPO.** Baseline Cluster-I comparison.
2. **Original vs. cleaned data.** Echo-bug sensitivity (METHOD-GAP-8 forensic
   evidence).
3. **With vs. without text-modality centroid guard.** T3 projected; if measured
   at camera-ready, report accept-rate delta.

### Methodological novelties to claim in §6.1

- **Echo-bug forensic disclosure** (METHOD-GAP-8; P10). Cite the file path in
  text.
- **Projected text-modality constellation guard** (METHOD-GAP-4 at text
  modality). Flag T3.
- **Sample-complexity collapse in text style** (METHOD-GAP-9; T3 projected).

### Forbidden (per vocabulary rules)

- No bare "alignment" without V10 disambiguation (first use in §6.1 must say
  geometric vs. scalar-reward).
- No "style LoRA" as a standalone method name — always pair with the Cluster-I
  comparator in §2.6 or §5.

---

## Case 2 — Identity-locked talking-head clone "Santa" (§6.2, **T2 + T3 projected**)

### Comparators to describe (in §2.5 / §5 / §6.2)

| Role | Keys | Why we compare |
|---|---|---|
| **Primary base model** | `meng2025echomimicv3` | Case 2 base model. EchoMimicV3 is the DIT + flow-matching video family; our method adds centroid aggregation + projected runtime guard on top. |
| **Flow-matching theory substrate** | `lipman2023flowmatching` | The underlying flow-matching probability-path objective. Cite on first mention of flow matching in §5 / §6.2. |
| **LoRA substrate** | `hu2022lora` | Parameter-efficient fine-tune on the EchoMimicV3 backbone. |
| **Flow-matching sign-convention lineage** | `meng2025echomimicv3`, `wan2025video` | Both EchoMimicV3 and Wan2.1-Fun family use target = noise − latent (glossary #30). Case 2 forensic disclosure (METHOD-GAP-7) pins this convention. |
| **Lip-sync evaluation anchor** | `li2024latentsync` | LSE-D / LSE-C projected metric. |
| **Identity-locked video comparators (§2.5 / §6.2 comparator paragraph)** | `cui2024hallo3`, `zheng2025hunyuanvideo`, `tian2024emo`, `tian2025emo2`, `yang2024cogvideox` | Alternative identity / audio-visual-conditioning approaches; **none uses multi-modal frozen centroids or runtime cosine gates** (METHOD-GAP-3, METHOD-GAP-4). Frame as architectural alternatives. |
| **Facial-shape modality substrate** | `li2017flame` (FLAME 4D) | One of ClipCannon's seven modalities (FLAME / expression). |
| **Emotion modality substrate** | `ekman1978facs` | FACS emotion modality; print reference. |
| **Pipeline components (§4.2 ClipCannon)** | `baevski2020wav2vec`, `bain2023whisperx`, `rouard2023htdemucs`, `campello2013hdbscan`, `zhang1996birch`, `google2020mediapipe`, `tinachen2021yolov5face`, `resembleai2024enhance`, `mcp2024spec` | Cluster-G components composed in the 23-stage DAG. Cite per-stage at §4.2. |

### Metrics to report

Required (from standard-metrics-per-cluster.md, Cluster F):

- **T2** —
  - Pipeline counts: 2,362 clips / 16 min source / 7 modalities / 23 stages /
    58 MCP tools.
  - Per-modality reference-set size + encoder + dimension.
  - LoRA rank + training hyperparameters.
  - Mouth-weighted flow-matching loss coefficient.
  - Flow-matching sign convention (cite forensic file in text).
  - FLAME + FACS modality integration (files / module paths).

- **T3 projected** (V22 flag mandatory):
  - Per-modality cosines at inference (visual CSIM-like; speaker WavLM SECS;
    voice pitch-consistency embedder cosine; emotion FACS embedding cosine;
    FLAME embedding cosine).
  - Accept-rate under five-modality constellation guard at chosen τ_m values.
  - LSE-D / LSE-C (lip-sync) vs. a chosen comparator baseline.
  - FVD (Fréchet Video Distance) vs. EchoMimicV3 no-LoRA baseline.
  - CSIM (face-identity cosine) vs. a fixed face-identity embedder.

### Ablations (required for §6.2)

1. **Centroid present vs. absent** (frozen centroid as alignment target vs.
   no centroid). Baseline for METHOD-GAP-3.
2. **Mouth-weighted vs. uniform flow-matching loss.** Engineering contribution;
   pairs with glossary #30 sign-convention context.
3. **Per-modality threshold sweep.** At least three τ_m values per modality;
   report accept-rate curves.
4. **Flow-matching sign flipped** (as a negative-control forensic demonstration;
   optional — may be framed as "we observed anti-denoising when the sign was
   flipped"; the fix restored convergence).

### Methodological novelties to claim in §6.2

- **Multi-clip centroid aggregation across five modalities** (METHOD-GAP-3).
- **Runtime five-modality constellation guard** (METHOD-GAP-4; projected T3).
- **Flow-matching sign-convention forensic** (METHOD-GAP-7; P10 disclosure).
- **DDA instantiation at N=7** (METHOD-GAP-1; 28 signals/input).
- **Sample-complexity collapse for identity-locked video** (METHOD-GAP-9; T3
  projected — 2,362 clips of one subject → multi-modal alignment).

### Forbidden

- No "prototype" / "anchor" / "reference vector" — V1 / V2 enforce "centroid".
- No "identity-preserving" — V3 enforces "identity-locked".
- No bare "flow matching" without the probability-path term + glossary #30 sign
  convention on first mention in §5 or §6.2.

---

## Case 3 — Voice cloning beyond 0.95 WavLM SECS (§6.3, **T1 measured** — the only fully-measured case)

### Comparators to describe (in §2.6 / §5 / §6.3)

| Role | Keys | Why we compare |
|---|---|---|
| **Primary base model** | **Qwen3-TTS (currently key-unresolved; proposed `qwen2024qwen3tts`, see `unresolved-citations.md` U01)** | Case 3 generator. Full ICL is Qwen3's in-context learning for speaker adaptation. |
| **Primary speaker-encoder (scorer)** | `chen2022wavlm` | microsoft/wavlm-base-plus-sv; the T1 anchor for WavLM SECS. Cross-encoder-independent of Qwen3's internal speaker embedder. |
| **Primary Cluster-E comparator (parity claim)** | `chen2024valle2` | VALL-E 2 "human parity" on LibriSpeech continuation via SIM / WER / DNSMOS. Our comparison is narrow-scope (Q14): different held-out, different encoder, different protocol. §8 Limitations must preempt the "unfair comparison" objection. |
| **Zero-shot TTS alternatives** (describe but do not measure against) | `wang2023valle`, `tan2024naturalspeech3`, `wang2024maskgct`, `chen2024f5tts`, `le2023voicebox`, `shen2023naturalspeech2`, `jiang2023megatts2`, `casanova2024xtts` | Frontier zero-shot TTS systems; the 0.86–0.89 WavLM cosine band is descriptive. **None uses a 50-clip centroid or a runtime cosine-gate**. |
| **Cross-encoder generalisation (future work)** | `desplanques2020ecapa` | ECAPA-TDNN as an alternative cross-encoder; future-work anchor (Q20). |
| **Non-intrusive naturalness scorers** | `reddy2022dnsmos`, `saeki2022utmos` | DNSMOS P.808 and UTMOS method papers; we use these as our naturalness scorers and report parity with real recordings. |
| **Open benchmark framing (future comparability)** | `christop2025cloneval` | ClonEval open benchmark; cite in §8 / §10 as the direction for reproducible cross-cohort comparison. |

### Metrics to report (T1 — all measured)

From standard-metrics-per-cluster.md, Cluster E primary:

- **WavLM SECS** (microsoft/wavlm-base-plus-sv cosine):
  - Mean 0.961, max 0.975 on 10 unseen sentences.
  - Distribution plot (box / violin) over the 10 sentences recommended.
- **DNSMOS P.808** (P.808 naturalness protocol):
  - Clone: 3.93. Real recording of the same speaker: 3.93. **Parity.**
- **UTMOS** (VoiceMOS Challenge 2022 system):
  - Clone: 3.012. Real recording of the same speaker: 2.997. **Parity.**
- **Same-session band** (authors' empirical observation, V12 hedge):
  - 0.95 – 0.99 on the same speaker's own recordings (pairs within a single
    session).
- **WER** on the 10-sentence set — required for intelligibility baseline
  (Cluster-E convention).
- **Reference-set size and centroid details**: 50 clips → L2-normalised mean of
  L2-normalised WavLM embeddings.
- **Inference-time guard**: best-of-12 at T=0.3 (K=12 candidates resampled;
  top-scoring accepted).
- **Cross-encoder disclosure**: WavLM SV ≠ Qwen3 speaker encoder (METHOD-GAP-5).

### Ablations (required for §6.3)

1. **N=1 vs. N=50 reference set.** Sample-complexity-at-centroid measurement;
   fills part of METHOD-GAP-9.
2. **No-guard vs. guarded output distribution.** Baseline for METHOD-GAP-4.
3. **T=0 greedy vs. T=0.3 best-of-K (K ∈ {1, 4, 12}).** Temperature +
   resampling sweep. Expected: T=0 greedy is deterministic but brittle; T=0.3
   + K=12 produces the 0.961 mean.
4. **Cross-encoder generalisation** (optional, T3 projected): score the same
   outputs with ECAPA-TDNN and report the SECS correlation vs. WavLM SECS.
   This fills METHOD-GAP-5 and connects to Q20.

### Methodological novelties to claim in §6.3

- **50-clip centroid as the alignment target** (METHOD-GAP-3).
- **Runtime WavLM best-of-12 cosine guard at T=0.3** (METHOD-GAP-4; the only
  fully-measured instantiation in the paper).
- **Cross-encoder SECS enforcement** (METHOD-GAP-5).
- **Same-session band disclosure + calibration** (METHOD-GAP-6; authors'
  observation; V12 hedge).
- **Comparator scope explicitly narrow** (Q14): we do NOT claim parity on
  LibriSpeech continuation. §8 Limitations states this.

### Forbidden

- **Bare "SECS"** — V4 enforces qualified encoder name. Always "WavLM SECS".
- **"Same-session ceiling" / "0.95+ is universal"** — V12 enforces
  authors'-observation framing.
- **"Reaches same-session band"** — V12 mandates "falls within our measured
  same-session band".

---

## Cross-case standard methods table (for §5 Method composite)

| Method element | Our instantiation | Citation keys to support |
|---|---|---|
| Embedder definition (frozen, pretrained, independent) | Case 1: text (Nomic/Qwen text backbone); Case 2: 7 modalities (wav2vec + WavLM + voice + emotion + FLAME + visual + text); Case 3: WavLM + Qwen3-speaker | `zhai2023siglip`, `nussbaum2024nomic`, `chen2022wavlm`, `li2017flame`, `baevski2020wav2vec`, `khattab2020colbert` |
| Centroid construction | L2-normalised mean of L2-normalised per-clip embeddings per modality | (no direct precedent — METHOD-GAP-3) |
| Constellation | Per-subject set of centroids across M modalities | (no direct precedent — METHOD-GAP-3) |
| Training objective — flow matching | Case 2: flow-matching probability path + mouth-weighted loss | `lipman2023flowmatching`, `meng2025echomimicv3` |
| Training objective — SFT+DPO | Case 1: SFT on cleaned corpus + DPO on preference pairs | `ouyang2022instructgpt`, `rafailov2023dpo`, `hu2022lora` |
| Training objective — ICL + centroid conditioning | Case 3: Full ICL over 50-clip centroid (no weight update) | proposed `qwen2024qwen3tts` (T3 disclosure; see U01) |
| Runtime constellation guard | Per-modality cosine ≥ τ_m; best-of-K resampling if below | (no direct precedent — METHOD-GAP-4) |
| Cross-encoder SECS evaluation | Case 3: WavLM SV scoring a Qwen3-TTS output | `chen2022wavlm` (scorer); `desplanques2020ecapa` (future-work alternative scorer) |
| Naturalness evaluation | DNSMOS P.808, UTMOS | `reddy2022dnsmos`, `saeki2022utmos` |
| Pipeline infrastructure (Context Graph) | 13 embedders, HNSW, RRF | `malkov2018hnsw`, `cormack2009rrf`, `khattab2020colbert`, `santhanam2022colbertv2`, `formal2021splade`, `wang2021kepler`, `bordes2013transe` |
| Pipeline infrastructure (ClipCannon) | 23-stage DAG, 7 modalities, 58 MCP tools | `baevski2020wav2vec`, `bain2023whisperx`, `rouard2023htdemucs`, `campello2013hdbscan`, `zhang1996birch`, `google2020mediapipe`, `tinachen2021yolov5face`, `resembleai2024enhance`, `mcp2024spec` |

## Writer-checklist (before handoff to agent-38 results-writer)

- [ ] **Case 3 §6.3** names all metrics in standard-metrics inventory § E, with
      canonical phrasing.
- [ ] **Case 2 §6.2** declares all T2 pipeline counts as T2 and all projected
      numbers with V22 flags.
- [ ] **Case 1 §6.1** declares forensic disclosure per P10 with file path.
- [ ] **Every SECS value** is qualified by encoder (V4).
- [ ] **Every "alignment"** first-use has V10 disambiguation (geometric vs.
      scalar-reward).
- [ ] **Every "alignment" in §5 or §6** traces to a method in this cheatsheet.
- [ ] **Every Cluster-H citation** is confined to §2.1 or §8.6 and the body
      total ≤ 6.
- [ ] **Qwen3-TTS citation** (U01) resolved before §6.3 is finalised.
- [ ] **Every methodological novelty claim** traces to a METHOD-GAP in
      methodological-gaps.md.

## Changelog

- 2026-04-16 — Initial issue by agent-09-methodology-scanner.
