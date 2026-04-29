# EXP-2 Output Deliverables

Detailed spec for every artefact. Each one has a defined consumer (paper, video, blog, GitHub) and a defined success criterion.

## 1. The Headline Number

`n_eff_summary.json` → a single scalar with bootstrap CI.

```json
{
  "headline": {
    "n_eff": 47.3,
    "ci_low": 44.1,
    "ci_high": 50.6,
    "method": "redundancy_adjusted",
    "panel_size": 14,
    "structural_upper_bound": 91,
    "interpretation": "The 14-embedder panel produces 47.3 effective independent supervisory signals per input on the 800-input audit corpus, against a structural upper bound of 91."
  },
  ...
}
```

**Consumer**: paper §3.1, paper §6.1 (replaces "up to 91"), YouTube video reveal, blog headline.

**Success criterion**: a number with bootstrap CI, three significant figures.

**Possible outcomes**:
- $N_{\mathrm{eff}} \in [60, 91]$: panel is highly independent; minor swap recommendations.
- $N_{\mathrm{eff}} \in [30, 60]$: typical case; significant redundancy in some pairs; specific swap recommendations.
- $N_{\mathrm{eff}} \in [14, 30]$: panel substantially redundant; rethink panel composition; document honestly.

All three are publishable. The story changes by outcome:
- "DDA effectively multiplies signal by ~7×" (mid case).
- "DDA effectively multiplies signal by 4-5×, with E1/E10 redundancy as the major cost" (low case).
- "DDA achieves near-91 with minor pairwise overlap" (high case; most surprising; needs careful validation against methodology).

## 2. The Redundancy Heatmap

`redundancy_heatmap.png` (web) + `redundancy_heatmap.pdf` (paper).

**Specifications**:
- 14 × 14 grid.
- Each cell colored by MI value (off-diagonal) or entropy (diagonal).
- Color scale: viridis (perceptually uniform); log-scaled for MI to span the full range.
- Embedder labels on both axes: "E1 (e5-large-v2 sem)", "E2 (sin-PE temp-recent)", etc. — readable, expanded, not just `E_n`.
- Color bar with units (nats).
- Title: "Pairwise mutual information across the 14-embedder Context Graph panel (n=800 inputs, 32-D PCA, InfoNCE primary)".
- Caption: full attribution + citation hint.
- Resolution: 300 DPI for PDF; 1920×1920 for PNG.
- Format: square aspect ratio.

**Consumer**: paper Figure 1 (or the new Figure adding to the §3 figure budget), YouTube T+0:30 reveal, blog hero image, tweet thread first tweet.

**Success criterion**: heatmap legible at thumbnail (web preview); embedder labels distinguishable at full size.

**Visual story it should tell**:
- Diagonal blocks (auto-MI / entropy) bright.
- Off-diagonal: structure visible — clusters of high-MI pairs (e.g., E1/E10/E14 grouping; E6/E13 grouping; E2/E3/E4 grouping).
- Some pairs near-zero (sanity for the framework's independence claim).

## 3. Per-Pair Table

`per_pair_table.csv` — 91 rows × 14 columns.

| Column | Type | Description |
|---|---|---|
| j | str | embedder j name |
| k | str | embedder k name |
| mi_infonce_nats | float | InfoNCE estimate |
| mi_infonce_ci_low | float | bootstrap 2.5% |
| mi_infonce_ci_high | float | bootstrap 97.5% |
| mi_ksg_nats | float | KSG estimate |
| mi_ksg_ci_low | float | bootstrap 2.5% |
| mi_ksg_ci_high | float | bootstrap 97.5% |
| agreement_nats | float | absolute difference |
| agreement_pass | bool | within 0.15 nats |
| redundancy | float | MI / min(H_j, H_k) |
| recommendation | str | "keep both", "consider swap", "high redundancy" |
| training_diagnostic | str | "converged", "overfit", "unstable" |

**Consumer**: paper appendix table, GitHub release, replication studies.

**Success criterion**: 91 rows, all columns populated for ≥ 90/91 pairs (one or two may have caveats).

## 4. Per-Pair JSON Bundle

`pair_results/E{j}_E{k}.json` — 91 files.

Format per [03_methodology.md](03_methodology.md) §13.

**Consumer**: replay tests, audit verification, future EXP-3 follow-up.

**Success criterion**: every file passes schema validation.

## 5. Spectrum Plot

`spectrum_plot.png/pdf`.

X-axis: rank index (1 to 14).
Y-axis: eigenvalue magnitude (log scale).
Plot: bar chart + dashed line for 0.05 threshold.
Annotation: vertical line at numerical rank cutoff.

**Consumer**: paper Figure 2, blog secondary visual.

**Success criterion**: clear visual reveal of effective rank.

## 6. Per-Embedder Entropy Bars

`entropy_bars.png/pdf`.

X-axis: 14 embedder slots.
Y-axis: entropy (nats).
Error bars: bootstrap CIs.
Colors: highlight high-entropy embedders (those carrying most independent information).

**Consumer**: paper appendix or main figure, video diagnostic, blog.

## 7. Embedder Swap Recommendation

`embedder_swap_recommendation.md`.

Identifies the 3-5 most redundant pairs and recommends swap candidates:

```markdown
# Embedder Swap Recommendations

Based on EXP-2 measurements:

## Highest-redundancy pairs (top 5)

1. E1 (e5-large-v2) ↔ E10 (e5-base-v2): MI = X nats, redundancy = 0.78.
   - Both dense semantic encoders trained on overlapping corpora.
   - Recommendation: replace E10 with a non-dense-semantic alternative
     (e.g., InstructorXL, a domain-tuned encoder, or a non-sentence-transformer
     architecture).

2. E6 (SPLADE-coCondenser) ↔ E13 (SPLADE-PP): MI = X nats, redundancy = 0.72.
   - Both SPLADE-family sparse encoders.
   - Recommendation: replace E6 with a non-SPLADE sparse encoder
     (e.g., DeepImpact, or a learned LSH variant).

3. E2 (sin-PE temporal recent) ↔ E4 (sin-PE positional): redundancy = 0.69.
   - Both sinusoidal positional encoders with different decay rates.
   - Recommendation: re-engineer E4 as a fundamentally different temporal
     encoder (e.g., learned positional with sequence-context or RNN-based).

...

## Recommended panel for EXP-2 v2 (future)

E1, [E1-replacement], E2, E3, E4, E5, [E6-replacement], E7, E8, E9, E11, E12, [E13-replacement], E14

Estimated post-swap N_eff: ~70 (from 47.3 currently).
```

**Consumer**: internal action item; future panel design; competitive intelligence (revealing what makes a *better* panel).

## 8. Audit Chain JSON

`audit_chain.json`.

Structure:
```json
{
  "run_id": "exp2_v1_20260415",
  "creator": "operator",
  "creation_timestamp": "2026-04-XX UTC",
  "artefacts": [
    {
      "artefact_id": "art_001",
      "label": "corpus_manifest_v1",
      "path": "artefacts/exp2/corpus_manifest_v1.json",
      "sha256": "...",
      "parent_artefact_ids": [],
      "created_by": "T-004",
      "created_at": "..."
    },
    ...
  ],
  "events": [
    {"event_type": "config_load", "config_path": "exp2.yml", "config_sha256": "..."},
    {"event_type": "embedder_load", "embedder_id": "E1", "weight_sha256": "..."},
    ...
  ],
  "code_commit_sha": "...",
  "embedders": [{"slot": "E1", ...}, ...],
  "hyperparameters_sha256": "..."
}
```

**Consumer**: regulators, replicators, paper auditors.

**Success criterion**: entire chain reproducibly verifiable from public hashes.

## 9. Paper Patch

`PAPER_PATCH.md` — drop-in replacement for paper §3.1 + §6.1.

See [09_distribution_plan.md](09_distribution_plan.md) §A for the exact content.

## 10. Publishable Summary

`publishable_summary.md` — internal, a single-page distilled finding:

```markdown
# EXP-2 Publishable Summary

## Headline
The 14-embedder Context Graph panel produces N_eff = 47.3 (CI: 44.1–50.6)
effective independent supervisory signals per input, against a structural
upper bound of 91.

## Method
- 800 inputs, 8 strata.
- All 91 embedder pairs, both InfoNCE and KSG estimators.
- 10,000 bootstrap resamples, seed 42.
- Three N_eff methods; redundancy-adjusted as primary.

## Key Pairs
- Highest MI: E1↔E10 (~5.2 nats), E6↔E13 (~4.1 nats).
- Lowest MI: E2↔E11 (~0.05 nats), E4↔E14 (~0.08 nats).

## Implication For DDA
The "up to 91 signals" claim becomes "measured 47.3 effective signals."
The 5x signal multiplication over the marginal 14 is real and significant.

## Implication For Panel Design
Swap E10 (overlaps E1) and E13 (overlaps E6) for non-overlapping alternatives;
estimated post-swap N_eff = ~70.

## Reproducibility
Full code: github.com/teleox/exp2-v1
Pre-registration: osf.io/...
Bundle DOI: zenodo.org/...
```

**Consumer**: paper revision, video/blog/tweet condensation.

## 11. YouTube Video Script

`docs/exp2/youtube_script.md`.

See [09_distribution_plan.md](09_distribution_plan.md) §C.

## 12. Blog Post

`docs/exp2/blog_post.md`.

See [09_distribution_plan.md](09_distribution_plan.md) §D.

## 13. Tweet Thread

`docs/exp2/tweet_thread.md`.

See [09_distribution_plan.md](09_distribution_plan.md) §E.

## 14. Open-Source Bundle

GitHub release `contextgraph-exp2-v1.0.0` per [05_technical_spec.md](05_technical_spec.md) §20.

**Consumer**: replicators, students, researchers.

**Success criterion**: a reviewer with no Context Graph context can `git clone && pytest` and see passing tests + replay artefacts within tolerance.

## 15. arXiv Revision

A new version of the paper at `arxiv.org/abs/...` with:
- §3.1 + §6.1 updated.
- Figure added (heatmap).
- Reference to GitHub release.
- Reference to Zenodo bundle DOI.
- Reference to OSF pre-registration DOI.

**Consumer**: scientific community.

**Success criterion**: arXiv accepts (no formatting violations).

## 16. Zenodo Archive

DOI'd archive of the open-source release. Bundle includes:
- Code (frozen at v1.0.0 tag).
- Corpus manifest.
- Per-pair table.
- Heatmap PNG/PDF.
- Audit chain.

**Consumer**: long-term replication.

**Success criterion**: DOI assigned; bundle persistent.

## 17. HuggingFace Hub

Model card + corpus card for the EXP-2 release:
- `huggingface.co/teleox/contextgraph-exp2-corpus-v1` — corpus.
- `huggingface.co/teleox/contextgraph-exp2-pca-models-v1` — fitted PCA models.
- `huggingface.co/teleox/contextgraph-exp2-pair-results-v1` — 91 pair JSONs + heatmap.

**Consumer**: AI/ML researcher tribe.

**Success criterion**: discoverable via HF search; cited from paper.

## 18. OSF Pre-Registration Record

Pre-registration completed before T-016 (production InfoNCE run).

**Consumer**: research community; defends against post-hoc tuning accusations.

**Success criterion**: timestamped DOI; hyperparams visible.

## 19. Diagnostic Logs

`run.log.jsonl` — full structured log of the production run. Useful for:
- Debugging.
- Future replication discrepancy investigation.
- Verifying the timing budget.

**Consumer**: operator, replicators, future EXP-3 design.

## 20. Synthetic-Validation Report

`synthetic_validation_report.json` (T-013 output).

Documents that the estimators recover known MI values within tolerance.

**Consumer**: paper appendix; defends methodology against "did your estimator work?" questions.

## 21. Cross-Validation Report

`cross_validation_report.json` (T-020 output).

Documents the InfoNCE-vs-KSG agreement across all 91 pairs.

**Consumer**: paper appendix.

## 22. Replay Log

`fsv_replay_log.json` (T-025 output).

Documents that the pipeline is deterministic.

**Consumer**: paper appendix; replicators.

---

## Deliverable Status Tracker

The operator maintains a status tracker (in `tasks/exp2/todo.md`) with:

| Deliverable | Task ID | Status | Owner | Due |
|---|---|---|---|---|
| Corpus manifest | T-004 | pending | subagent + operator | Day 1 |
| Embeddings | T-007 | pending | subagent | Day 2-3 |
| PCA models | T-009 | pending | subagent | Day 3 |
| Pair results | T-016, T-017, T-018 | pending | subagent | Day 6-7 |
| N_eff summary | T-021 | pending | subagent | Day 8 |
| Heatmap | T-022 | pending | subagent | Day 9 |
| Audit chain | T-024 | pending | subagent | Day 11 |
| FSV pass | T-025 | pending | subagent + operator | Day 11-12 |
| Paper patch | T-027 | pending | operator | Day 12 |
| YouTube video | T-031 | pending | operator | Day 13-14 |
| Blog post | T-032 | pending | operator | Day 13-14 |
| arXiv revision | T-030 | pending | operator | Day 14 |
| Zenodo / GitHub release | T-029 | pending | subagent + operator | Day 14 |
