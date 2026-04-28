# 09 Risk Register

| Risk | Probability | Impact | Mitigation |
|---|---:|---:|---|
| Dirty scene/stage text re-enters training | High | Critical | Hard contamination gates before SFT/DPO. Manual sample report. |
| CAMD pairs teach topic preference instead of style | Medium | High | Content-equivalence gates via E1/E10/E14, style-gap gates via E12/E13/E9/E7. |
| Model memorizes Shakespeare passages | Medium | High | n-gram, MinHash, longest-common-substring anti-memorization gate. |
| Modern-topic refusal returns | Medium | High | Bridge examples across modern domains; system prompt says modern assistant, not historical Shakespeare. |
| Code/structured syntax breaks | Medium | High | Format-specific constrained decoding and compile/parse FSV. |
| Archaic-token loops | Medium | Critical | Loop-negative DPO, optional unlikelihood, MVAG coherence checks, runtime loop detector. |
| v4.2 artifacts contaminate v5 | Medium | High | Treat current v4.2 as untrusted until audited. |
| E14 is misused as style signal | Medium | Medium | Use E14 for content/cross-lingual unless calibration proves style discrimination. |
| Synthetic bridge data causes drift | Medium | Medium | Cap generated data, require TCT verification, never train on candidate model outputs recursively. |
| Guard passes style but misses coherence | High | High | Separate coherence panel; style score alone cannot pass. |
| Direct harness overclaims production quality | High | High | Browser/dashboard FSV is the publish gate. |
| DPO overpowers SFT | Medium | High | Low LR, beta 0.05-0.10, SFT regularization, content validation. |
| Thresholds are stale after E14 | High | Medium | Recalibrate thresholds after every embedder-panel change. |
| Runtime guard latency too high | Medium | Medium | Cache centroids, batch scoring, use cheap hard gates before daemon calls. |
| HF card overstates results | Medium | Medium | Publish exact category pass rates and known failure modes. |

## Stop Conditions

Stop the run immediately if any of these appears:

- scene headers in generated responses,
- Context Graph implementation notes in any training record,
- non-quotation exact Shakespeare passage output,
- loop output passes guard,
- code block cannot parse on more than 10% of code prompts,
- guard reports success when daemon is down,
- DPO reward accuracy rises while browser FSV drops.

## Open Decisions

### Start From v2.1 Or Base?

Recommendation: start from v2.1.

Run base-start as an ablation only if there is time. v2.1 remains the best known production baseline.

### Use Current v4.2?

Recommendation: no, not until audit.

The existence of `shakespeare_lora_v4_2/dpo/final` is useful, but sampled pair data shows contamination patterns. It may still be valuable as a research checkpoint, not a publish baseline.

### Use Synthetic Bridge Data?

Recommendation: yes, but bounded and verified.

The base model already knows modern topics. The bridge data exists to teach style application, not facts. Keep bridge examples guard-scored, provenance-labeled, and anti-memorization-filtered.

### Claim Cross-Lingual Period Transfer?

Recommendation: not yet as a stable capability.

Treat it as an observed phenomenon until a multi-seed, multi-prompt, language-reviewed matrix passes.

### Optimize For Perfect Style Or Perfect Helpfulness?

Recommendation: optimize for invariant style plus preserved usefulness.

The product promise is not a Shakespeare quote machine. It is useful assistance through Shakespeare's style channel.

