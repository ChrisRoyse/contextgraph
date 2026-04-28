---
namespace: patterns
agent_id: agent-09-methodology-scanner
timestamp: 2026-04-16T18:25:00Z
status: active
tags: metrics, inventory, evaluation, writer-handoff, cluster-metrics
depends_on:
  - memory/discoveries/agent-09-methodology-scanner--methodology-scan.md
  - memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md
  - memory/decisions/agent-08-citation-extractor--master-reference-pool.md
downstream_consumers:
  - agent-37-methodology-writer
  - agent-38-results-writer
  - agent-39-discussion-writer
  - agent-42-adversarial-reviewer
---

# Standard Metrics Inventory — Per Cluster

This is the **writer-ready metrics reference**. Every metric listed is used by at
least one citation in the master pool; use this inventory to pick comparator
numbers, to report our own case-study metrics, and to pass the adversarial-reviewer
check on §6 (metrics named in methods must be reported in results).

Every metric is labelled with:
- **Unit** / range (or "unitless" where dimensionless).
- **Direction of merit** (↑ better / ↓ better).
- **Typical reporting context** (where it lives in the prior literature).
- **Tier constraint** (T1-anchor metrics vs. auxiliary).
- **Used by our paper in case N** (1 / 2 / 3) — or n/a if framing-only.

## A. Scaling laws + data wall — cluster metrics

| Metric | Unit / range | Direction | Typical context | Used by our paper |
|---|---|---|---|---|
| Validation loss / perplexity | nats (loss) / exp(nats) (ppl); > 0 | ↓ better | Kaplan, Chinchilla | framing only |
| Tokens per parameter | tokens / parameter; [1, 100] | ↑ toward 20 (Chinchilla) | hoffmann2022chinchilla | framing only |
| Public-text stock estimate | tokens (scale 10^14) | — (descriptive) | villalobos2024data | framing only |
| Robots.txt-restricted fraction | % of high-quality corpus | ↑ indicates wall tightening | longpre2024consent | framing only |
| Exhaustion-year range | year | — | villalobos2024data (2026–2032 band) | framing only |

**Body-pool rule.** These metrics are framing quotes in §1 and §3.3. We do NOT
report our own numbers on any Cluster-A metric.

## B. Model collapse + synthetic data — cluster metrics

| Metric | Unit / range | Direction | Typical context | Used by our paper |
|---|---|---|---|---|
| KL(pₙ ‖ p₀) | nats; ≥ 0 | ↓ (against collapse) | shumailov2024nature, alemohammad2023mad | n/a |
| Generator variance / tail mass | depends on family | ↓ variance = late collapse | shumailov2024nature | n/a |
| Cross-entropy on held-out real | nats | ↓ better | dohmatob papers, seddik2024collapse | n/a |
| FID / IS (image generators) | scalar; FID ↓ better | ↓ better | alemohammad2023mad | n/a |
| Iteration at which collapse onset detected | integer; ≥ 1 | — | gerstgrasser2024colm, dohmatob papers | n/a |

**Body-pool rule.** Cited for §3.3 categorical argument ("DDA has no feedback
loop") and §8 threat R4. We do NOT run model-collapse experiments on DDA in this
manuscript.

## C. Compression-as-intelligence — cluster metrics

| Metric | Unit / range | Direction | Typical context | Used by our paper |
|---|---|---|---|---|
| Compression ratio (bits/bits or bits/char) | dimensionless ratio or bpc; > 0 | ↓ (better compression) | deletang2024compression, valmeekam2023llmzip, li2025lossless | framing §3.1, §3.5 |
| Correlation r (compression vs. capability) | [-1, 1] | ↑ (huang ≈ 0.95) | huang2024compression | framing §3.1 |
| Classification accuracy (compression-substitute classifier) | % | ↑ better | jiang2023gzip | n/a in results |
| **Meaning-compression ratio MC = N + C(N,2)** | signals / input | ↑ (higher yield) | **our paper §3.4** | Case 1 / 2 / 3 all report N-specific MC |

**Novel metric.** The meaning-compression ratio MC is introduced in §3.4. Report MC
= 91 signals/input (N=13, Context Graph) and MC = 28 signals/input (N=7,
ClipCannon). These are **T4 theoretical counts**, not measurements.

## D. Multi-modal embedders + retrieval — cluster metrics

| Metric | Unit / range | Direction | Typical context | Used by our paper |
|---|---|---|---|---|
| Top-k retrieval accuracy | % | ↑ better | khattab2020colbert, santhanam2022colbertv2, formal2021splade | n/a (C.G. §4.1 descriptive only) |
| NDCG@10 / MAP / MRR | [0, 1] | ↑ better | khattab / santhanam / cormack | n/a |
| EER on VoxCeleb1-O | % (lower = better discrimination) | ↓ better | chen2022wavlm, desplanques2020ecapa | referenced in §6.3 Case 3 methods only |
| Recall@k (ANN) | [0, 1] | ↑ better | malkov2018hnsw | n/a (infrastructure) |
| Fusion-lift delta (RRF vs. best single ranker) | NDCG delta | ↑ (positive lift) | cormack2009rrf | referenced §4.1 descriptive |
| **Cosine similarity (L2-normalised)** | [-1, 1]; in our work normalised to [0, 1] | ↑ (higher = closer) | SigLIP, WavLM, all embedders | **Case 1, 2, 3 constellation guard uses this** |

**Our metric.** Per-modality cosine similarity $\cos(\phi_m(\hat{y}), c_m)$ is our
primary inference-time quantity (glossary #11). In the manuscript, report these
on the same scale as the encoder (raw or [0,1] normalised), and always name the
encoder (glossary #16 / V4).

## E. Zero-shot TTS + voice cloning — cluster metrics (Case 3 T1 anchor)

| Metric | Unit / range | Direction | Typical context | Used by our paper |
|---|---|---|---|---|
| **WavLM SECS** (microsoft/wavlm-base-plus-sv cosine) | [-1, 1]; our measured range on Case 3 [0.94, 0.975] | ↑ better | chen2022wavlm (scorer); chen2024valle2 (reports 0.881 WavLM-large SV cosine as secondary) | **Case 3 primary: mean 0.961, max 0.975 on 10 unseen sentences** |
| SIM-o (neural-codec speaker similarity) | [-1, 1]; VALL-E 2 0.643 | ↑ better | chen2024valle2 (primary); tan2024naturalspeech3 0.67; wang2024maskgct 0.687; chen2024f5tts 0.66 | not used (we don't use their speaker-encoder) |
| WER | % | ↓ better | all Cluster-E TTS papers | Case 3 should report on the 10-sentence set |
| DNSMOS P.808 (naturalness MOS) | [1, 5] | ↑ better | reddy2022dnsmos (method paper) | **Case 3 primary: 3.93 clone vs. 3.93 real (parity)** |
| DNSMOS P.835 SIG/BAK/OVRL | [1, 5] per component | ↑ better | reddy2022dnsmos | optional reporting |
| UTMOS (learned naturalness MOS) | [1, 5] | ↑ better | saeki2022utmos (method paper) | **Case 3 primary: 3.012 clone vs. 2.997 real (parity)** |
| CMOS (comparative MOS) | [-3, +3] listener delta | ≈ 0 (parity) | all Cluster-E | optional; requires listener study |
| **WavLM SECS same-session band** (authors' empirical observation on Case 3 speaker) | [0.95, 0.99]; **NOT universal, NOT a published benchmark** (V12) | — | **our Case 3 own measurement** | Case 3 reports the band as authors' observation; not a citation |
| τ_m per-modality threshold | scalar cosine floor; encoder-specific | — | **our paper §5.3** | Case 3 sets τ_speaker calibrated to same-session band lower edge |

**Mandatory reporting for Case 3 (T1):**
- WavLM SECS mean + max on 10 unseen sentences.
- DNSMOS P.808 clone vs. real-recording parity (same utterance baseline).
- UTMOS clone vs. real-recording parity (same utterance baseline).
- The V12 hedge on the same-session band ("authors' own empirical observation;
  not a universal value").
- Cross-encoder disclosure (Case 3 is WavLM; the generator Qwen3-TTS conditions
  on a different speaker-encoder family).

**Forbidden (per V4).** Bare "SECS" without the encoder name. Always "WavLM SECS",
"SpeechBrain SECS", "Qwen3 SECS", etc.

## F. Identity-locked video generation — cluster metrics (Case 2 T2+T3)

| Metric | Unit / range | Direction | Typical context | Used by our paper |
|---|---|---|---|---|
| LSE-D / LSE-C (SyncNet lip-sync) | LSE-D: distance ↓; LSE-C: confidence ↑ | ↓ D / ↑ C | li2024latentsync (anchor); cui2024hallo3 | Case 2 projected (T3) |
| FVD (Fréchet Video Distance) | scalar; > 0 | ↓ better | cogvideox, emo, hallo3 | Case 2 projected (T3) |
| FID (per-frame) | scalar; > 0 | ↓ better | most Cluster-F | optional |
| CSIM (face-identity cosine) | [-1, 1] | ↑ better | cui2024hallo3, tian2024emo, tian2025emo2 | Case 2 projected (T3); our visual-modality cosine in the constellation guard |
| Visual MOS (subjective) | [1, 5] | ↑ better | most Cluster-F | optional |
| **Per-modality cosine to centroid** (visual, audio-speaker, audio-voice, emotion, FLAME) | [-1, 1] per modality | ↑ better | **our paper §5.3** | **Case 2 five per-modality cosines (T3 projected)** |
| Accept-rate under constellation guard | % of generations passing all τ_m | ↑ → stricter generator | **our paper §5.3** | Case 2 projected |
| Flow-matching sign convention (qualitative) | label: "noise − latent" (EchoMimicV3 / Wan family); flipped → anti-denoising | correct-sign required | **our forensic** `docs/inspect/08_training_audit.md` | **Case 2 forensic disclosure** |

**Mandatory reporting for Case 2 (T2 + T3):**
- Pipeline counts (T2): number of clips (2,362), source duration (16 min),
  modalities (7), ClipCannon stages (23), LoRA rank.
- Per-modality centroid provenance (count of clips in each reference set, encoder,
  dimension).
- Flow-matching sign convention (T2 forensic): state the convention + cite the
  forensic file.
- Projected T3 numbers flagged: LSE-D/LSE-C, FVD, CSIM, per-modality cosines,
  accept-rate. Each must carry V22 "(projected; camera-ready will re-verify)".

## G. Auxiliary pipeline components — cluster metrics

| Metric | Unit / range | Direction | Typical context | Used by our paper |
|---|---|---|---|---|
| WER (ASR) | % | ↓ better | bain2023whisperx | not reported (tool quality assumed) |
| SI-SDR (source separation) | dB; > 0 = separated | ↑ better | rouard2023htdemucs | not reported |
| SUPERB benchmark metrics (various) | varies | varies | baevski2020wav2vec | not reported |
| Cluster purity / silhouette | [0, 1] | ↑ better | campello2013hdbscan, zhang1996birch | not reported |
| mAP (face detection) | [0, 1] | ↑ better | tinachen2021yolov5face | not reported |

**Body-pool rule.** Cluster G is pipeline-engineering; we cite but do not report
auxiliary-component metrics. The per-component quality is *assumed* and the DAG
quality is reported via the downstream case-study numbers.

## H. Enterprise / market signals — cluster metrics (body quota ≤ 6)

| Metric | Unit / range | Direction | Typical context | Used by our paper |
|---|---|---|---|---|
| MedQA / MedMCQA accuracy | % | ↑ better | singhal2023nature, tu2025nature | framing only (§8.6 Ethics) |
| Pile-of-Law dataset coverage | count (documents) | — | henderson2022pileoflaw | framing only |
| FPB / financial-NER accuracy | % | ↑ better | wu2023bloomberggpt | framing only |
| Licensing-deal dollar value | USD | — (descriptive) | H02, H08, H09, H11, H18 | framing only (if any) |

**Body-pool rule.** Cluster H contributes no methodological metric to our §3–§7
quantitative reporting.

## I. Alignment (RLHF / DPO / CAI) — cluster metrics

| Metric | Unit / range | Direction | Typical context | Used by our paper |
|---|---|---|---|---|
| Preference-model win-rate | [0, 1] | ↑ better | ouyang2022instructgpt, bai2022constitutional | comparator framing |
| Reward-model score | scalar (reward-model-specific) | ↑ better | ouyang2022instructgpt, rafailov2023dpo | comparator framing |
| KL-to-reference-policy | nats; ≥ 0 | ↓ (staying on-distribution) | rafailov2023dpo, ouyang2022instructgpt | comparator framing |
| MMLU / TruthfulQA / BBH accuracy | % | ↑ better | all Cluster I | comparator framing (Case 1 may report post-SFT+DPO) |
| Harmlessness / helpfulness MOS (HH-RLHF) | scale; typically [1, 5] | ↑ better | bai2022constitutional | framing only |

**Body-pool rule.** I-cluster metrics are comparator framing for TCT. Our paper
does NOT report win-rate or reward-model scores on our own case studies because
our method is geometric, not scalar-reward.

---

## Cross-cluster metric legend (canonical phrasing for PAPER.md)

When a metric is reported, always use the canonical phrasing below (V4 / V12 / V22):

- **"WavLM SECS"** — never bare "SECS". If a different encoder is used, say
  "SpeechBrain SECS" or "Qwen3 SECS". Always name the encoder.
- **"DNSMOS P.808"** or **"DNSMOS P.835"** — never bare "DNSMOS".
- **"UTMOS"** with citation on first use.
- **"Per-modality cosine"** — used in TCT body; qualify with the modality name
  (visual, speaker, voice, emotion, FLAME, text).
- **"Meaning-compression ratio MC = N + C(N,2) signals per input"** — use in §3.4.
- **"Same-session band (authors' observation)"** — when citing the 0.95–0.99
  range; always with the V12 hedge.
- **"Accept-rate under constellation guard"** — for the fraction of generations
  passing all τ_m.

## Mandated "metrics to report" checklist per case study

This checklist is the contract with agent-37 / agent-38 / agent-42: every metric
listed here MUST appear in the corresponding case study section, in the canonical
phrasing.

### Case 3 Voice Cloning (§6.3, **T1**)

- [ ] WavLM SECS mean + max on 10 unseen sentences (our measurement).
- [ ] DNSMOS P.808 parity (clone vs. real-recording baseline).
- [ ] UTMOS parity (clone vs. real-recording baseline).
- [ ] Reference-set size (50 clips) + centroid dimension.
- [ ] Temperature T + best-of-K values (T=0.3, K=12).
- [ ] Cross-encoder statement (WavLM SV ≠ Qwen3 speaker encoder).
- [ ] V12 hedge on same-session band.
- [ ] V22 tags inapplicable (all measured T1).
- [ ] **Ablations**: (a) N=1 vs. N=50 reference; (b) no-guard vs. guarded;
      (c) T=0 greedy vs. T=0.3 best-of-K.

### Case 2 Santa (§6.2, **T2 + T3**)

- [ ] Pipeline counts: 2,362 clips / 16 min source / 7 modalities / 23 stages
      (T2).
- [ ] Per-modality reference-set size and encoder.
- [ ] LoRA rank + training-set size (T2).
- [ ] Mouth-weighted flow-matching loss coefficient (T2).
- [ ] Flow-matching sign convention (T2): noise − latent; cite forensic file.
- [ ] Per-modality cosines (five modalities) at inference (**T3 projected**,
      V22 tag mandatory).
- [ ] Accept-rate under guard (**T3 projected**, V22).
- [ ] LSE-D / LSE-C / FVD / CSIM (**T3 projected**, V22).
- [ ] **Ablations**: (a) centroid present vs. absent; (b) mouth-weighted vs.
      uniform flow-matching; (c) per-modality threshold sweep.

### Case 1 Shakespeare (§6.1, **T2 + T3**)

- [ ] Pipeline description: cleaned-corpus SFT → DPO on preference pairs.
- [ ] Echo-bug forensic paragraph (T2): diagnostic method, fix, 15/15 post-fix
      FSV passes.
- [ ] LoRA hyperparameters.
- [ ] **T3 projected**: perplexity on held-out Shakespeare, style-transfer
      cosine to text centroids, constellation-guard accept-rate (V22 tags).
- [ ] **Ablations**: (a) SFT-only vs. SFT+DPO; (b) cleaned vs. original data
      (echo-bug sensitivity).

---

## Changelog

- 2026-04-16 — Initial issue by agent-09-methodology-scanner.
