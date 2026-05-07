---
namespace: handoff
agent_id: coordinator
timestamp: 2026-04-21T11:30:00Z
status: active
tags: handoff, v4.1, shipped, v4.2, candidates, huggingface, production
---

# Handoff: v4.1 Shipped — v4.2 Candidates for Next Session

## Status as of 2026-04-21 11:30 CDT

**v4.1 is production-ready and validated:**
- 10/10 on enhanced 10-prompt FSV (code-gap + retention)
- 3/3 on production-stack smoke test (HTTP + daemon + constrained decoder + v4.1)
- Inference server default updated to v4.1 (was v2.1)
- Model card at `~/.contextgraph/models/shakespeare_lora_v4_1/final/README.md`
- Upload script at `scripts/upload_v4_1_to_huggingface.py`

**BLOCKED on external action**: HuggingFace token expired. User must provide
a fresh token and run:

```bash
HF_TOKEN="<fresh-token>" python3 scripts/upload_v4_1_to_huggingface.py
```

This uploads the 1.1 GB adapter + README + tokenizer to `cabdru/shakespeare-lora-gemma4`
(same repo as v2.1, new commit with updated README).

## v4.1 artifacts on disk

| Path | Size | Purpose |
|------|------|---------|
| `~/.contextgraph/models/shakespeare_lora_v4_1/final/` | 1.1 GB | Production adapter (published to HF when token works) |
| `~/.contextgraph/models/shakespeare_lora_v4/final/` | 1.1 GB | v4 baseline — keep for comparison |
| `~/.contextgraph/models/shakespeare_lora/final/` | 3.2 GB | v2.1 production baseline (HF live) |
| `~/.contextgraph/training/shakespeare_v4_augment_code.jsonl` | 37 KB | 31 Shakespeare-framed code records |
| `~/.contextgraph/training/shakespeare_v4_1_train.jsonl` | ~500 KB | 800-record v4.1 training corpus |
| `~/.contextgraph/training/shakespeare_v4_sft.jsonl` | 26.2 MB | Full Phase 2 output (9,000 records) |
| `~/.contextgraph/training/shakespeare_v4_sft_final.jsonl` | 13.5 MB | v4 SFT corpus (8,699 accepted) |

Intermediate sft_checkpoint/ directories cleaned (7.6 GB freed).

## v4.2 candidates (from Sherlock-1 corpus investigation)

All 4 discovered during v4.1 work; not fixed in v4.1 scope.

### Gap #1: Foreign-language filter bug (HIGH value, reclaim 116 records)

**What**: The v4 style-score filter (English-anchored Sonnets constellation)
rejected 116 REJECTED_SCORE records where the model correctly answered in
**archaic Portuguese / Italian / German / French / etc.** — the CORRECT behavior
was being filtered out. Breakdown:
- pt→pt (correct archaic Portuguese): 13 records rejected
- it→it: 10 records rejected
- de→de: 7 records rejected
- fr→fr: 6 records rejected
- es→es: 4 records rejected
- cjk→cjk: 14 records rejected
- (and more)

**Meanwhile** the filter ACCEPTED 263 responses where the model wrongly
answered in English after being asked to respond in another language.

**Root cause**: `STYLE_WEIGHTS` in `shakespeare_v4_generate_responses.py:53`
assumes English archaic content. Apply Sonnets centroid (E12 ColBERT MaxSim)
against non-English text → artificially low score.

**Fix**: Language-aware scoring. When prompt language != English, use a
different archaic reference (e.g., Golden Age Spanish corpus centroid for es)
OR use a multilingual embedder (E10 paraphrase) as the anchor.

### Gap #2: Structured output (JSON / YAML / SQL tables)

**What**: 0 positive examples in v4 corpus for structured output requests.
Only the 4 adversarial prompts asking for JSON/SQL/structured data — and
these were ACCEPTED as verse-refusals of the request. Model learned "refuse
structure in verse" rather than "emit structure with Shakespeare narration."

**Fix**: ~50 handcrafted records showing valid structured output (real JSON,
real SQL, real tables) wrapped with Shakespeare narration. Continue-train
same as v4.1 but with 50 structured records + 500 replay.

### Gap #3: Math / equation rendering

**What**: 0.33% of training corpus contains math notation. Model hasn't learned
to render LaTeX, multi-step proofs, derivatives.

**Fix**: ~40 handcrafted records: quadratic formula, pythag theorem, derivatives,
integrals, algebraic manipulations — all with Shakespeare narration.

### Gap #4: Length bimodal

**What**: Accepted responses cluster at 400-700 or 1200-1500 chars (the 1500-char
ceiling clips longer compositions). Model has learned these two natural stopping
points only.

**Fix**: Regenerate the 81 REJECTED_FILTER=`too_long` records with MAX_LEN=2500
and max_new_tokens=600. Append to training corpus.

## Recommended v4.2 approach

1. **Scope**: address all 4 gaps in one continue-training pass
2. **New records**: ~50 structured + 40 math + 81 regenerated-long + reclaim 116 foreign = **~287 new records**
3. **Replay**: 500 stratified from v4.1 corpus
4. **Training**: continue-train from v4.1 adapter (2 epochs, LR 3e-5 — lower
   than v4.1's 5e-5 to preserve more of v4.1's tight fit)
5. **FSV**: extend to 15-prompt harness with new tests per gap
6. **Expected runtime**: ~20 min wall time

## Known v4.1 shortcomings we accepted

- `STYLE_THRESHOLD=0.50` is strict — loses ~3% of acceptable responses at
  the margin (part of why augmentation generator had 17.7% acceptance)
- Upsample factor 4× means each new code pattern sees 8 gradient updates in
  2 epochs — probably adequate but some nuance patterns may be weaker
- No DPO pass (per earlier decision on synthetic-data collapse risk)
  - v2.1 had SFT + DPO
  - v3/v4/v4.1 are SFT only
  - Could revisit DPO if real-text negatives become available

## Infrastructure guardrails in place (Sherlock-2 audit findings applied)

All present in v4.1 committed code, verified working:

- H1 `resume_from_checkpoint`: training auto-resumes from latest `checkpoint-N/` if present
- H2 `fcntl.flock`: prevents concurrent training runs corrupting adapter
- M2 base-model version check in inference server: guards against silent version mismatch
- M3 disk preflight: refuses train if <20 GB free
- M6 `torch.nan_to_num` in ShakespeareLogitProcessor: bnb+bf16+Blackwell NaN guard

## For next agent

Start by reading:
- `memory/discoveries/coordinator--*.md` (daemon orphan, MCP timeout, category
  arithmetic, corpus gaps — full incident history)
- `memory/reports/coordinator--v4-prompt-bank-final-report.md` (Phase 1 context)
- `memory/journal/coordinator--v4-phase2-launched.md` (Phase 2 context)
- `tmp/SITREP_2026-04-18_171128.md` (mid-session handoff for deep context)
- This file for what's next

If user wants v4.2, the concrete steps are:
1. Write `scripts/shakespeare_v4_augment_structured.py` (50 records, similar
   to `shakespeare_v4_augment_code.py` pattern — handcrafted exemplars + v2.1 gen)
2. Write `scripts/shakespeare_v4_augment_math.py` (40 records)
3. Write `scripts/shakespeare_v4_2_language_aware_filter.py` (reclaim 116 foreign)
4. Merge augmentations + 500 replay → `shakespeare_v4_2_train.jsonl`
5. Continue-train from v4.1 adapter (`shakespeare_train_v4_2_continue.py`)
6. Extend FSV to 15 prompts (add structured/math/foreign tests)
7. Run FSV, compare to v4.1 baseline, ship if ≥10/15
