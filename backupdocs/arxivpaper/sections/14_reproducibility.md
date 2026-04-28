# Appendix A. Reproducibility

This appendix specifies the hyperparameters, data manifests, and version anchors required to reproduce the three case studies at the release posture disclosed in the Ethics section. Placeholders marked `[author-to-fill at final submission]` will be populated with SHA-256 hashes at submission time.

## A.1 Release posture per case

| Case | Release posture | Reproducibility scope |
|---|---|---|
| 1 Shakespeare | byte-identical | Trained LoRA (rank 256, $\alpha = 512$ across 258 modules on Gemma-4-E4B-it) published at `https://huggingface.co/cabdru/shakespeare-lora-gemma4`; DDA-projected corpus (7,925 cleaned SFT + 11,724 cleaned DPO); three-layer enforcement stack (LoRA + constrained decoder + 13-embedder audit); 5/5 FSV battery; pinned commit `[author-to-fill]` |
| 2 Santa | protocol-level | TCT protocol, six forensic fixes, constellation construction specification; subject artefacts withheld |
| 3 Voice | protocol-level | Pipeline version `qwen3-tts-foricl-v1`, best-of-12 specification, 50-clip centroid procedure, WavLM scorer configuration, threshold calibration; subject artefacts withheld |

## A.2 Per-case hyperparameters

**Case 1 — Shakespeare stylistic LoRA (MEASURED 2026-04-17).** Base: Gemma-4-E4B-it (Google open-weight release), NF4-quantised at inference. LoRA rank 256, $\alpha = 512$, dropout 0.0; 258 target modules (`q_proj`, `k_proj`, `v_proj`, `o_proj`, `gate_proj`, `up_proj`, `down_proj` across all transformer layers in `language_model.layers.*`). SFT phase: 3 epochs, per-device batch 1, gradient accumulation 32 (effective batch 32), learning rate $2 \times 10^{-4}$, cosine schedule, warmup 5%, bf16, optimiser `adamw_8bit`, max sequence length 512, packing disabled, 4 dataloader workers pinned. DPO phase: 1 epoch, same batch configuration, learning rate $5 \times 10^{-5}$, $\beta = 0.1$, `ref_model=None` (implicit reference to fit in VRAM), max length 512. Hardware: single NVIDIA RTX 5090 (32 GB), CUDA 13.0, PyTorch 2.11; total wall-clock approximately 7.5 h (SFT 164.7 min, DPO 282.4 min). Final metrics: SFT loss 1.12 with 98.9% token accuracy over 765 steps (3 × 255); DPO loss 0.47 with 98.0% `rewards/accuracies` and `rewards/margins` 43.62. Trained LoRA published under Apache-2.0 at `https://huggingface.co/cabdru/shakespeare-lora-gemma4` with model card, `shakespeare_constrained_decoder.py` (standalone logit processor boosting ≈500 archaic token IDs by $+3.0$ and suppressing ≈200 modern token IDs by $-8.0$), tokenizer files, and chat template; DDA-projected supervision was drawn from Context Graph's thirteen-embedder decomposition of the Shakespeare canon (2,698 memories, ≈249,000 multi-dimensional supervisory signals) per §6.1.2.

**Case 2 — Santa identity-locked video LoRA.** Base: EchoMimicV3 (1.86 B parameters, Wan2.1-Fun backbone). LoRA rank 256, $\alpha = 512$ on eight target modules (`q, k, v, o, k_audio, v_audio, k_img, v_img`); `q_audio` excluded per the forensic audit. Source: 16-minute single-subject interview, 2,362 curated clips at 25 fps produced by the ClipCannon 23-stage DAG. Loss: mouth-weighted flow-matching ($10\times$ mouth, $2\times$ face, $1\times$ background) with corrected sign `target = noise - latent` (EchoMimicV3 velocity convention). Starting per-modality loss weights: $\lambda_{\text{speaker}} = 1.0$, $\lambda_{\text{global-visual}} = 0.5$, $\lambda_{\text{local-visual}} = 0.3$, $\lambda_{\text{motion}} = 0.2$, $\lambda_{\text{caption}} = 0.3$. Operating thresholds (projected): visual 0.70, emotion 0.60, speaker 0.80, voice 0.85.

**Case 3 — Voice cloning.** Base: Qwen3-TTS, inference-only (no parameter update). Conditioning: Full In-Context Learning over all 50 reference clips. Reference-set construction: top-50 of 489 candidate clips ranked by WavLM cosine against the real-microphone reference (disjoint at the clip level from Case 2 training partition). Centroid: L2-normalised mean of L2-normalised WavLM-base-plus-sv projections. Selection: $K = 12$ candidates per sentence at $T = 0.3$, argmax by $\cos(\phi_{\text{WavLM}}(\hat y^{(k)}), c_{\text{speaker}})$. Threshold: $\tau_{\text{speaker}} = 0.95$, calibrated to the 10th-percentile same-session-band floor on a disjoint calibration set of authentic subject samples *before* the 10-sentence held-out evaluation; no retrospective adjustment of $\tau$. Seed chain: per-sentence chain seed logged in the pipeline audit record; per-candidate seeds derived as chain seed + per-candidate index.

## A.3 Embedder and scorer version manifest

| Embedder / Scorer | Role | Version | Dimension | Weight SHA-256 |
|---|---|---|---|---|
| E1 e5-large-v2 | Context Graph semantic | v2 (HuggingFace `intfloat/e5-large-v2`) | 1,024 | `[author-to-fill]` |
| E5 nomic-embed-causal | Context Graph causal | v1.5 | 768 | `[author-to-fill]` |
| E7 Qodo-Embed | Context Graph code | v1 | 1,536 | `[author-to-fill]` |
| E11 KEPLER | Context Graph entity | production | 1,024 | `[author-to-fill]` |
| E12 ColBERTv2 | Context Graph late-interaction | v2 | 128/token | `[author-to-fill]` |
| E13 SPLADE | Context Graph sparse | production | 30,522 | `[author-to-fill]` |
| SigLIP-SO400M | ClipCannon visual | Google release | 1,152 | `[author-to-fill]` |
| WavLM-base-plus-sv | Case 3 speaker guard + evaluator | HuggingFace `microsoft/wavlm-base-plus-sv` | 512 | `[author-to-fill]` |
| WavLM-large | ClipCannon speaker | HuggingFace `microsoft/wavlm-large` | 512 | `[author-to-fill]` |
| ECAPA-TDNN | EXP-1 cross-encoder (pending) | SpeechBrain | 192 | `[author-to-fill]` |
| DNSMOS P.808 | Naturalness predictor | Microsoft release (Reddy et al., 2022) | scalar | `[author-to-fill]` |
| UTMOS | Naturalness predictor | Saeki et al. (2022) | scalar | `[author-to-fill]` |

All embedder parameters are frozen; no gradient update flows into any of the above at any phase of the pipeline.

## A.4 Hardware and wall-clock

Case 3 measurement run was executed on a single workstation with an NVIDIA RTX 4090 (24 GB VRAM) plus 128 GB system RAM. Wall-clock for the 10-sentence best-of-12 evaluation was approximately 9 minutes; full pipeline including centroid construction and threshold calibration was approximately 40 minutes end-to-end. Cases 1 and 2 training runs are specified in the respective subject repositories (pinned commits to be released at the byte-identical level for Case 1 and at the protocol level for Case 2).

## A.5 Bootstrap procedure (Case 3 CI)

The retrospective 95 % bootstrap confidence interval on the 10-sentence mean WavLM SECS (percentile method) uses: $N_{\text{resample}} = 10{,}000$, seed $= 20260416$, resampling with replacement from the 10 observed per-sentence SECS values. Result: $[0.951, 0.971]$. This is a descriptive summary of the ten observed values; it is not a power-based inferential interval and assumes iid across sentences, which under-represents possible text-level shared structure.

## A.6 Case 1 FSV battery

Case 1 verification is organised in two batteries. The pre-training 15-test integrity smoke-verification covers: (1) base-model identity hash; (2) LoRA adapter attachment check; (3) SFT loader integrity; (4) DPO loader integrity; (5) echo-scan over training data; (6) constellation construction determinism; (7) frozen-encoder audit (`requires_grad == False`); (8) centroid invariance between Phase 1 and Phase 3; (9) $\mathcal{L}_{\mathrm{TCT}}$ integration check; (10) constrained-decoding vocabulary audit; (11) guard predicate evaluation on canned inputs; (12) rejection-policy dry run; (13) provenance trace completeness; (14) end-to-end smoke generation; (15) threshold-proximity stability under seed perturbation. These are integrity checks, not identity-quality measurements. The post-training 5-test Full State Verification executed via Playwright against the live dashboard and inference server covers: (a) love-poem generation (passed with 9 archaic markers; poetic, no prompt echo); (b) quantum-entanglement explanation with Bell's theorem in verse (passed with 5 markers; correct physics); (c) admin-override prompt-injection refusal in character (passed; refused override while speaking Shakespeare); (d) Go binary-search function generation with Shakespearean code comments (passed; valid Go code); (e) Spanish-language prompt demanding Spanish reply (passed; produced Golden Age Spanish per §6.1.4). Result: 5/5 PASS on 2026-04-17.

## A.7 Data availability

Case 1 Shakespeare base corpus is the public-domain canon available from Project Gutenberg. The DDA-projected derivative corpus will be released at byte-identical scope. Case 2 and Case 3 subject-specific data are withheld under the asymmetric release posture disclosed in the Ethics section.

## A.8 Source crosswalk

All measured Case 3 numbers (0.961 mean, 0.975 max, 0.914 min, 9/10 $\geq 0.95$, 3.93/3.93 DNSMOS, 3.012/2.997 UTMOS, bootstrap CI $[0.951, 0.971]$, temperature-sweep table, centroid-size ablation) are drawn from the measurement SITREP at `tasks/newdata/alldata.md` (sections 5.1, 5.4, 5.5, 5.6). The crosswalk is verifiable by line-number against that file.
