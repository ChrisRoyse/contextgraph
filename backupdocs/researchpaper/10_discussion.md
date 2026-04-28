# 10. Discussion, Limitations, and Open Problems

## 10.1 Summary of what has been shown

Three claims, three kinds of evidence:

| Claim | Evidence type | Support |
|---|---|---|
| Derived Data Abundance multiplies effective training signal | Constructive | Section 4 (13 embedders for text, 91 signals per input). Section 5 (7 modalities for video, 12,000+ labeled samples from 16 min). |
| The multiplied signal is not synthetic and does not collapse | Structural | Section 3.3 (no feedback loop exists in a measurement pipeline). Section 9.12 (empirical micro-scale collapse when a TTS output *is* fed back as reference). |
| TCT produces verifiable per-output alignment for embedding-definable targets at small-sample complexity | Empirical | Section 7 (15/15 style tests on Shakespeare three-layer stack). Section 8 (identity-locked video from 16 min single-subject). Section 9 (0.961 mean WavLM SECS; DNSMOS / UTMOS parity with real speech). |

The third claim is the most directly testable and the most load-bearing. Case 3 is fully-measured; Case 1 is empirically verified on its 15-test battery with a forensically-identified training-data pathology that the thirteen-embedder guard caught and single-embedder evaluation would have missed; Case 2 is architecturally complete but the specific LoRA is still training at submission time — we flagged this candidly in §8.9 and would recommend that the camera-ready version re-verify the projected lip-sync accuracy against the trained outputs.

## 10.2 Limitations

The framework has real limits. We enumerate them in decreasing order of how load-bearing they are to the paper's claims.

### 10.2.1 Embedder independence is assumed, not proven

The DDA multiplicative-gain claim (Section 3.2) depends on the $N$ embedders being information-theoretically independent. If two embedders produce correlated representations, the effective signal gain from adding the second is reduced — in the limit of identical representations, the second embedder contributes zero new information.

For the systems we describe, the embedders are drawn from different base architectures and largely disjoint training data. We treat their outputs as approximately independent. We do not quantify this independence formally. An information-theoretic measurement (mutual information between embedders on a common corpus) is possible in principle; we have not performed it.

The empirical support we offer is indirect: the cross-embedder-anomaly tool (Section 4.7) finds, for any query, memories that score highly in one embedder but poorly in another. If the embedders were redundant, no such anomalies could exist; the fact that they routinely exist is evidence of non-redundancy. But this is a lower bound on independence, not a measurement of it.

Future work should formalise this with mutual-information estimation across embedder outputs on a held-out corpus. An open question is whether the DDA multiplicative factor scales linearly with added embedders in practice or with strongly-diminishing marginal returns past some small $N$.

### 10.2.2 The compute cost of decomposition is non-trivial at internet scale

Running a corpus through 13 text embedders costs approximately 13× the compute of running it through one. A SigLIP forward pass is ≈ 10 ms per image on modern hardware. Processing the entirety of YouTube (estimated 800 million hours) at 2 FPS through a single embedding model would require roughly 4.6 million GPU-hours — substantial, but well within the compute budgets of frontier AI labs that spend billions of dollars on training runs.

At the single-organisation scale (a corporate document archive, a research-lab publication archive, a healthcare system's clinical-record corpus) the cost is easily absorbed. At internet scale, a commercial operator of DDA-as-a-service would need to amortise the decomposition cost across many downstream uses. The argument is that decomposition is a one-time fixed cost; the derived data can be stored and reused across any number of training runs. Whether this amortises favourably depends on how much of frontier-training cost is actually bottlenecked by data (our Section 1 argument) vs. by compute (a different argument).

### 10.2.3 The constellation guard has inference-time cost

Re-embedding every generated frame through SigLIP at inference time costs ≈ 10 ms per frame on RTX 5090. For 25 fps video this is 250 ms per second of output — a 25% overhead on the rendering budget. Batched validation can halve this; faster encoders can reduce it further. But the cost is real and scales with output length. For real-time applications (meeting bots, live streams) the overhead is the practical ceiling on how many independent modality checks can run at inference.

### 10.2.4 Threshold calibration is empirical

The per-modality thresholds in Section 8.8 (visual 0.70, emotion 0.60, speaker 0.80, voice 0.85) were calibrated empirically from the centroid spread statistics and cross-validation against the real-human baseline measurements in Section 9.7. There is no first-principles method for selecting optimal thresholds. Too loose admits drift; too tight forces excessive regeneration.

A promising direction is to use the spread statistics themselves — cosine p50 and p95 of the training reference against the centroid — as guidance for threshold selection. A threshold set at, say, p05 of the reference distribution admits anything that looks at least as centroid-close as the least-centroid-close reference sample. This is implementable but not rigorously characterised.

### 10.2.5 The constellation defines the centre, not the radius of valid variation

The constellation is a frozen point. The model can drift *within* the manifold defined by the threshold without being rejected. The user must verify that intra-manifold variation is acceptable for the task; the guard only prevents extra-manifold drift. This is not a failure of the framework; it is a property. For identity-locked avatar generation it is appropriate (the subject should be recognisable; specific expression choices are acceptable variation). For tighter constraints (e.g., specific physical properties of a medical imaging output), a constellation alone may not be sufficient and additional constraints (classifiers, hard rules) may be needed.

### 10.2.6 Not all alignment problems admit an embedding target

TCT works when the target can be expressed as proximity to a reference centroid. Identity-locked generation admits this. Style control admits it. Safety profiles that can be expressed as classifier regions admit it. Open-ended "be helpful and harmless" objectives in general dialogue do not obviously admit this: helpfulness is a capability more than a measurable attribute, and the relevant reference embeddings may not exist or may not be stable.

The framework is therefore positioned as a complement to scalar-reward alignment, not a replacement. RLHF / DPO / Constitutional AI (see Related Work) remain appropriate for their original problem class. TCT addresses the subclass where the alignment target admits a reference embedding.

### 10.2.7 Embedder completeness

The argument "anything you can embed, you can constrain" assumes that embedding models provide approximately complete descriptions of their input domains. This holds reasonably well for mature models (SigLIP on images, WavLM on voice, e5-large-v2 on semantic text) but may not hold for less-studied modalities: touch, smell, social context, multi-agent interaction dynamics. The framework's power is limited by the quality and coverage of available frozen embedding models. An open call: the field benefits from more diverse, better-trained frozen embedders along more axes of meaning.

### 10.2.8 Reference data quality is load-bearing

DDA inherits the bias, noise, and quality properties of its input. A constellation built from low-quality reference data produces a low-quality identity target. The framework does not fix garbage-in-garbage-out; it amplifies whatever is in the reference.

In Case 2 (Section 8.3), the six-day forensic sprint turned up six blockers that would have invalidated any training run regardless of the algorithm — wrong sign, wrong frame rate, wrong mouth location, dead LoRA target, etc. In Case 1 (Section 7.3), 67% of the training data was corrupted with scene headers that the LoRA dutifully learned to reproduce. In both cases the framework performed as designed; the problem was the data. Any production TCT deployment needs a forensic-posture data-audit step comparable to the `docs/inspect/*.md` reports; this is not an optional nicety.

### 10.2.9 The Shakespeare-style echo is currently retraining; Case 2's LoRA is not fully verified

These are specific empirical gaps in Sections 7 and 8 that we flag candidly rather than paper over. The paper's structural claims do not depend on those specific pending results — the theoretical argument and the other two validated cases (especially Case 3, fully measured) carry the load — but the camera-ready version should re-verify the pending numbers from completed training runs.

## 10.3 Open problems

1. **Formalise embedder independence.** Quantify the mutual information between pairs of embedders on a common corpus; characterise the DDA multiplicative-gain curve as a function of embedder correlation.
2. **Automatic threshold calibration.** Derive per-modality thresholds from the reference spread statistics without manual tuning; provide a procedure that produces thresholds with a stated false-rejection rate on the reference distribution.
3. **Hierarchical constellations.** A single constellation per subject is a point; a hierarchy of constellations (per emotional state, per discourse context, per time-of-day) may carry more information. How does the hierarchy interact with the guard architecture?
4. **Cross-constellation operations.** Given two constellations (two subjects, two styles), what does blending, interpolation, or arithmetic on them mean, and when is the result itself a valid constellation?
5. **The "doorway" to AGI claim.** The conversational transcript motivating this paper includes a speculative claim that DDA "basically turns that data wall into a data door". We do not formally defend this claim. A careful characterisation of what capability boundaries DDA moves — and which it does not — is an open empirical programme. The case studies here establish that identity-locked generation from small datasets becomes tractable under DDA+TCT; whether this is a capability that "takes us toward AGI" is a larger question that this paper does not attempt to answer.
6. **Scaling-law modification.** If DDA multiplies the effective training signal by a factor $f$, does the Chinchilla-optimal ratio of parameters to training tokens shift by that factor? A modified scaling-law analysis under DDA-enriched training data would address this. We conjecture the ratio stays roughly the same (parameters must scale to absorb the richer signal) but the absolute performance gain at fixed compute is higher. This is not tested.
7. **Synthetic-data hybridisation.** DDA is distinct from synthetic data. But can the two be combined? Running the output of a synthetic-data pipeline through DDA decomposition before using it for training produces labels on synthetic content. Whether the resulting labels are trustworthy — given that the underlying content is model-generated — is an open question. Our inclination is that DDA does not rescue bad synthetic data, but we have not tested this formally.
8. **The orthogonality of "meaning compression" to LLM-as-compressor.** Delétang et al. (2024) and Huang et al. (2024) show LLMs *are* compressors. DDA decomposes data to multiply its meaning density. Are these two phenomena directly relatable? A speculative framing: LLMs are compressors because they learn the redundancy structure of human language; DDA works because different embedders capture orthogonal redundancy structures that any single LLM averages over. A rigorous information-theoretic account of this relationship is open.

## 10.4 Reproducibility and release

All three systems referenced in the paper exist as production code bases maintained in private repositories. Context Graph is roughly 370 K lines of Rust; ClipCannon is roughly 67 K lines of Python; the Santa LoRA training scripts and the Shakespeare three-layer stack are additional repositories. Open-sourcing these systems is in scope for the paper's associated release; the timing depends on commercial considerations discussed in Section 12 (Teleox.ai positioning).

At minimum, the paper's camera-ready version should release:

- The `ConstellationAccumulator` / `ConstellationGuard` reference implementations in both Rust (Context Graph) and Python (Phoenix).
- The voice-cloning pipeline scripts (`optimize.py`, `verify.py`, the WavLM-scored best-of-N harness).
- A small benchmark dataset and evaluation protocol for the same-session band claim, so that readers can reproduce the 0.95-threshold empirical characterisation on their own reference speakers.
- Appendix A (Section 14) schemas in machine-readable form so that a re-implementation can verify its data-structure conformance.

Sections 11 and 12 close the paper with the companion system (OCR Provenance) that applies the same architecture to document intelligence, and with the commercial positioning of the work under Teleox.ai.
