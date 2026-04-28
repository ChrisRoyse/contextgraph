---
namespace: patterns
agent_id: agent-28-analysis-planner
timestamp: 2026-04-16T17:05:00Z
status: active
tags: reporting-conventions, sig-figs, tier-tags, measurement-date, projection-labels, paste-ready
depends_on:
  - memory/decisions/agent-28-analysis-planner--analysis-plan.md
  - memory/decisions/agent-25-method-designer--case-study-protocols.md
  - memory/decisions/agent-27-instrument-developer--instrument-specifications.md
downstream_consumers:
  - agent-37-methodology-writer
  - agent-38-results-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
  - agent-43-reproducibility-checker
---

# Number-Reporting Conventions — Paste-Ready

## 0. Purpose

This pattern file gives writers (agent-37, agent-38, agent-40) the
**exact phrasing and formatting rules** to apply to every number in
§6 and §7. Agent-42 (adversarial reviewer) uses it as the checklist
to catch violations. Agent-43 (reproducibility checker) cross-
references it against the JSON manifest.

---

## 1. Significant figures (global rule)

- **3 sig-figs** on every scalar reported in-text, in-table, or
  in-figure-caption.
- **3 sig-figs after rounding**, not truncation. Round half to even
  (banker's rounding) to avoid upward bias.
- **Deltas** reported at 3 sig-figs too (e.g., Δ = +0.094, not +0.0942).
- **Integer counts** (e.g., "9 / 10") are exact; not subject to sig-fig
  rounding.
- **Percentages** at 3 sig-figs max (e.g., 96.1%, not 96.13%).

### Examples

| Raw value | Reported |
|---|---|
| 0.9612345 | **0.961** |
| 0.97504 | **0.975** |
| 0.914001 | **0.914** |
| 0.01723 | **0.0172** (leading zeros don't count; 3 sig-figs = 0.0172) |
| 3.93256 | **3.93** |
| 3.011897 | **3.01** (but we publish 3.012 because the author's report already uses 3.012; agent-28 accepts this as a 4-sig-fig exception for exact parity with prior documentation — document it). |

**Exception**: UTMOS 3.012 is reported at 4 sig-figs because the
SITREP and white paper already use 3.012; downstream consumers may
read the ms and cross-reference. Documented.

**Exception**: std 0.017 is 2 sig-figs; we report 0.0172 if the
third sig-fig is meaningful after rounding, else 0.017.

---

## 2. Tier tags (mandatory on every number)

Every reported number carries exactly one tier tag, in prose or
in a table column:

| Tier | Meaning | Formatting |
|---|---|---|
| **T1 MEASURED** | Verified, reproducible, dated, pipeline-versioned. | Plain text; optional footnote with date + SHA. |
| **T1 PENDING** | Instrument ready; run planned but data not yet collected. Must appear as "PENDING" in prose or "—" in table with PENDING footnote. | Italic `*PENDING*` or placeholder "—" with footnote. |
| **T3 PROJECTED** | Qualitative projection / prediction; always parenthetical. | Parenthetical: `(projected; camera-ready will re-verify)`. |

### Paste-ready phrasing

**T1 MEASURED example**:
> "The mean WavLM SECS is **0.961** over the 10 held-out sentences
> (measured 2026-04-16; pipeline `qwen3-tts-foricl-v1`)."

**T1 PENDING example**:
> "The mean ECAPA-TDNN SECS is *PENDING* (W1 remediation; instrument
> A.2 ready per §5.3)."

**T3 PROJECTED example**:
> "Case 2's four-modality conjunction accept-rate is **in the
> [0.45, 0.75] range** (projected; camera-ready will re-verify)."

---

## 3. Measurement date and pipeline version

Every T1 MEASURED number must cite:

- **Measurement date**: ISO-8601 (YYYY-MM-DD).
- **Pipeline version**: git SHA-7 (e.g., `ce0a052`) OR semver
  (e.g., `v1.3.0`) OR named pipeline (e.g., `qwen3-tts-foricl-v1`).

### Placement options (pick one per table/caption)

| Placement | Example |
|---|---|
| **Inline parenthetical** | "…0.961 (2026-04-16; `qwen3-tts-foricl-v1`)." |
| **Table footnote** | `^a Measured 2026-04-16, pipeline qwen3-tts-foricl-v1.` |
| **Caption footer** | "All Case-3 measurements: 2026-04-16, pipeline qwen3-tts-foricl-v1." |

**Rule**: every measured number in §6 tables has at minimum a
footnote referencing date + pipeline. §7 discussion prose may
abbreviate to "measured 2026-04-16" without repeating pipeline
version if the figure/table caption has already stated it.

---

## 4. Confidence intervals (N ≥ 20 rule + N=10 exception)

### The rule

CIs appear in the manuscript **only when N ≥ 20**, with the single
documented exception of Case 3's retrospective N=10 bootstrap.

### Phrasing when N ≥ 20 (e.g., Case 2 conjunction accept-rate)

> "The four-modality conjunction accept-rate is 0.62 (Wilson-score 95%
> CI [0.51, 0.72]; N = 40 held-out chunks)."

### Phrasing for Case 3 exception (N=10)

> "The mean WavLM SECS is 0.961 (retrospective 95% bootstrap CI
> [0.951, 0.971], N = 10 held-out sentences, 10,000 resamples with
> replacement, seed 20260416; this is not a power-based interval —
> see §8 Limitations)."

The verbatim phrase **"this is not a power-based interval"** must
appear at least once in §6.3 when the Case-3 CI is first cited. It
may be abbreviated to "(retrospective CI, N=10)" in subsequent
mentions within the same section.

### When N < 20 (rule)

**No CI**. Report descriptive statistics only: mean, median, min,
max, std. A writer may mention "N = X" but not a CI.

---

## 5. Hypothesis tests, p-values, power analyses — all prohibited

### Rule

**Zero hypothesis tests, zero p-values, zero power calculations** in
the §6 Results or §7 Discussion for any of the three cases.

### Phrasing for parity without a test

Instead of "Wilcoxon signed-rank yielded p = 0.83 (failed to
reject H₀)", write:

> "DNSMOS P.808 clone mean (3.93) and real-microphone mean (3.93) are
> **identical to 3 sig-fig** across the held-out sentence set (N = 10
> clone, N = 14 real). UTMOS clone (3.012) and real (2.997) differ by
> **Δ = +0.015**, within the same-session noise floor for UTMOS on
> this subject."

### Phrasing for ablation deltas

Instead of "paired t-test p < 0.01", write:

> "Full ICL vs. x-vector-only ablation yields **Δ = +0.094** on mean
> WavLM SECS (measured; direction: Full ICL > SpeechBrain x-vector-
> only)."

### Phrasing when a reviewer asks for a test

Standard paste-ready response:

> "We run no hypothesis tests on any of our three case studies. Cases
> 2 and 3 are N = 1 single-subject demonstrations; Case 1 is a
> single-pipeline demonstration. Inferential statistics on these
> designs would be misleading: p-values require a defined null
> hypothesis over a population we do not claim to sample from.
> We report descriptive statistics and ablation deltas; §8 Limitations
> states the scope explicitly."

---

## 6. Projection discipline (T3 numbers)

### Rule

Any number that is not yet measured but that we discuss as a
**qualitative prediction** must be:

1. Tagged **T3 PROJECTED**.
2. Flanked by the parenthetical `(projected; camera-ready will
   re-verify)`.
3. Preferably expressed as a **range**, not a point value, to signal
   uncertainty.

### Paste-ready examples

> "Case 1's calibrated $\tau_{\text{text-style}}$ is **projected in
> [0.80, 0.92]** (projected; camera-ready will re-verify)."

> "Case 2's global-visual cosine at operating threshold 0.70 is
> **projected to yield accept-rate in [0.60, 0.85]** (projected;
> camera-ready will re-verify)."

### What counts as T3 vs T1-PENDING

- **T3 PROJECTED**: qualitative prediction based on prior theory,
  adjacent-literature norms, or forensic expectation. No instrument
  has run; no data exists to measure against.
- **T1 PENDING**: instrument is specified, ready, and awaiting a
  run. We know the run will return a number; we just haven't run
  it yet.

Example:
- "ECAPA agreement Spearman $\rho$ > 0.6" — **T3 PROJECTED** (no
  ECAPA run exists yet; this is a prior from adjacent literature).
- "ECAPA mean SECS on the 10 held-out sentences" — **T1 PENDING**
  (instrument A.2 is ready; we just haven't run it on our audio).

---

## 7. Ablation delta table convention

Ablations reported in a two-column-minimum table:

| Column | Content |
|---|---|
| Ablation | Short descriptor, e.g., "Full ICL vs x-vector-only" |
| Delta | Signed Δ value at 3 sig-fig, e.g., +0.094 |
| Direction | "Full ICL > x-vector" (the winning arm first) |
| Source / date | Citation + measurement date |

No confidence interval on ablation deltas (typically N ≤ 10 for our
cases). No "statistically significant" claim. Just the direction
and magnitude.

### Paste-ready example (Case 3 ablation)

| Ablation | Δ | Direction | Source |
|---|---|---|---|
| Full ICL vs x-vector-only | +0.094 | Full ICL wins | SITREP 2026-04-16 |
| Target encoder vs mismatched encoder scoring | −0.013 | Target encoder wins | ibid. |
| Temperature sweep 0.2–0.8 | T=0.3 optimum | — | ibid. |
| 1-clip vs 50-clip centroid | +0.015 | 50-clip wins | ibid. |

---

## 8. Conjunction / accept-rate phrasing

### Rule

Report accept-rates as **proportion with explicit denominator**.
Never just "96%" — always "9 / 10" or "9 of 10 (90.0%)".

### Paste-ready examples

> "9 of 10 held-out sentences (90.0%) yielded an argmax candidate with
> WavLM SECS ≥ 0.95."

> "The four-modality conjunction accept-rate is **0.62** (25 of 40
> held-out chunks; Wilson-score 95% CI [0.46, 0.76])."

---

## 9. Cosine-scale discipline

- **Raw cosine [−1, +1]** for WavLM SECS, ECAPA SECS, SigLIP visual
  cosine — matches published literature. No hidden `(raw+1)/2`.
- **If a number must be rescaled** (e.g., for a [0,1] probability-like
  display), the rescaling is documented inline: "rescaled to [0,1]
  via `(raw+1)/2` for display purposes; underlying T1 value is raw
  cosine 0.961."
- **Sparse / inverted-index embedders (E6, E13)** report raw cosine
  over sparse vectors; no rescaling.
- **MaxSim (E12)** reports raw MaxSim (sum of per-query-token max
  similarities); note it is **not** cosine and its range differs.

---

## 10. Per-encoder naming discipline (V4 hedge carried)

Every SECS / MOS mention in §6 or §7 names the encoder. Writers may
NOT say "SECS = 0.961"; the correct form is "WavLM-SECS = 0.961"
or "WavLM-base-plus-sv SECS = 0.961".

### Paste-ready table column header

`"Metric (encoder)"` not `"Metric"`.

Example:
| Metric (encoder) | Value |
|---|---|
| SECS (WavLM-base-plus-sv) | 0.961 |
| SECS (ECAPA-TDNN) | PENDING |
| DNSMOS P.808 | 3.93 |
| UTMOS strong | 3.012 |

---

## 11. Manifest cross-reference

Every T1 MEASURED number in §6 must appear in agent-27's JSON
manifest schema §H. Writers must cite:

- `derived_metrics.wavlm_secs_mean` → 0.961 in Table 6.3.
- `derived_metrics.dnsmos_p808_clone` → 3.93.
- `derived_metrics.utmos_clone` → 3.012.
- etc.

Agent-43 validates that every table cell maps to a manifest field.

---

## 12. Citation discipline for ablation and historical numbers

Ablation deltas and historical (pre-fix) numbers cite:

- SITREP: `tmp/SITREP_2026-04-16_135543.md`.
- White paper: `tasks/newdata/alldata.md`.
- Forensic audit: `docs/inspect/08_training_audit.md` (Santa).
- Pipeline source: e.g., `scripts/shakespeare_full_pipeline.py`.

Each cited with full path in first mention; short form afterward.

---

## 13. Anti-patterns to catch (agent-42 checklist)

| Anti-pattern | Flag |
|---|---|
| Number without tier tag | **REJECT**: flag and ask writer to tag. |
| Number with 4+ sig-figs in body prose | **REJECT**: round to 3 sig-fig. |
| CI reported for N < 20 (other than Case-3 exception) | **REJECT**: drop CI, report descriptives. |
| "p < 0.05" or any p-value | **REJECT**: replace with descriptive delta. |
| "Power analysis shows N = 50 is sufficient" | **REJECT**: remove; cite agent-26 §A.*.6 "no formal power analysis". |
| "Statistically significant" (without a test behind it) | **REJECT**: replace with "observed delta of magnitude X". |
| "SECS = 0.961" without encoder name | **REJECT**: require "WavLM-SECS = 0.961". |
| T1 PENDING number reported as if measured | **REJECT**: add PENDING tag. |
| T3 PROJECTED number without parenthetical | **REJECT**: add "(projected; camera-ready will re-verify)". |
| Pooled metric across Cases 1/2/3 | **REJECT**: cross-case pooling invalid per §5.2 of analysis plan. |

---

## 14. Confidence

**HIGH** that a writer following these conventions verbatim produces
a §6 that passes agent-42 adversarial review and agent-43
reproducibility check.

**HIGH** that the three sig-fig rule matches the numbers already in
the SITREP / white paper.

**HIGH** that the no-hypothesis-test discipline is non-controversial
in exploratory case-study reporting — it is the correct discipline
regardless of reviewer preference.

## Changelog

- 2026-04-16 — Initial issue by agent-28-analysis-planner.
