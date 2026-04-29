# EXP-2 Data Corpus

The 800-input audit corpus, stratified across 8 strata × 100 inputs each. The corpus is the most consequential single decision in EXP-2; a biased corpus produces a biased $N_{\mathrm{eff}}$.

## 1. Stratification Goals

The corpus must exercise every embedder's discriminative axes. A corpus that is purely English literary prose under-stresses E14 (BGE-M3 multilingual). A corpus with no code under-stresses E7 (Qodo). A corpus with no causal claims gives E5 nothing to discriminate. A corpus with all single-sentence inputs under-stresses E12 (token MaxSim).

Cross-corpus diversity is the audit's evidence base. Each stratum 100 inputs.

## 2. The 8 Strata

### S1 — Shakespeare canonical corpus (100)
Source: `/home/cabdru/contextgraph/shakespear/` directory + the 2698 memories already in CF_FINGERPRINTS.

Purpose: anchors against the Case 1 paper instance; ensures backward comparability.

Selection: 100 stratified across plays + sonnets, balanced by length (mean 200 tokens, std 80, range 80–500).

### S2 — Code (100)
Source: stratified across:
- 25 Python (mix of standard library, ML/AI, web frameworks).
- 25 Rust (workspaces; from your own crates plus rustc/tokio/serde).
- 25 TypeScript / JavaScript.
- 25 SQL / shell / config (TOML, YAML, Dockerfile).

Selection: 100 unique snippets, 80–600 tokens each. Cleaned (no copyrighted vendor code).

Purpose: stresses E7 (Qodo code embedder); contrasts with S1.

### S3 — Causal-claims corpus (100)
Source: distill from Wikipedia "X causes Y" patterns + scientific abstracts. Use your existing `causal_realdata_bench` / `causal_e2e_bench` test corpora.

Purpose: stresses E5 (nomic-causal, asymmetric direction).

Selection: 100 sentences with explicit cause-effect claims, balanced cause→effect vs effect→cause, fact-checked for legitimacy of claim.

### S4 — Learning events (100)
Source: real `CF_LEARNING_EVENTS` rows from your existing system, selected to span the 14D before/after topic profiles.

Purpose: real session content; tests the meta-property — does an embedder panel sensitive to learning events also produce diverse MI patterns?

Selection: 100 distinct event records with `outcome.label` populated.

### S5 — BEIR-style retrieval queries + documents (100)
Source: 50 short queries + 50 long documents from BEIR (https://github.com/beir-cellar/beir). MS MARCO + NQ + HotpotQA.

Purpose: stresses retrieval-tuned embedders (E1, E10, E13).

Selection: balanced query/doc ratio, no overlap with training data of any embedder we use.

### S6 — Random Wikipedia paragraphs (100)
Source: Wikipedia 2024 dump, sampled uniformly.

Purpose: baseline; ensures we're not overfitting to your stack-curated corpora.

Selection: 100 paragraphs, 100–400 tokens, sampled with seed 42, deduplicated against Shakespeare and BEIR sets.

### S7 — Temporal/sequence content (100)
Source: chronologically-ordered conversation turns, news article timeline series, scientific paper version histories.

Purpose: stresses E2 (recency), E3 (periodic), E4 (positional). Ensures temporal embedders see content with explicit temporal signals.

Selection: 100 multi-turn snippets each with timestamps. Mixed sources (10 conversation snippets, 5 news timelines, 5 paper versions = 100 paragraphs).

### S8 — Multilingual (100)
Source: parallel sentences from FLORES-200 (https://github.com/facebookresearch/flores).

Purpose: stresses E14 (BGE-M3 multilingual). Without S8, E14's axis goes underused, biasing its MI estimates.

Selection: 50 English originals + their translations to 5 languages (Spanish, French, German, Chinese, Arabic) — 100 sentences total. Audit-chain logs per-sentence language.

## 3. Total: 800 inputs, 8 strata, 100 per stratum.

## 4. Validation Rules (Per-Stratum)

Each stratum, before being added to the production corpus, passes:

| Rule | Check |
|---|---|
| No duplicates within stratum | text-hash uniqueness |
| No duplicates across strata | text-hash uniqueness, full corpus |
| Length bounds | 50 ≤ tokens ≤ 800 (per Whisper / E1 limits) |
| Encoding | utf-8 valid, no control chars (except newlines/tabs) |
| Per-embedder validity | every input produces non-zero embedding for every embedder |
| License | each stratum's license documented in manifest |

## 5. Corpus Manifest

The manifest is a JSON file with this structure:

```json
{
  "corpus_id": "exp2_corpus_v1",
  "version": "1.0.0",
  "manifest_sha256": "<computed at finalization>",
  "creation_timestamp": "<ISO 8601 UTC>",
  "creator_attribution": "operator",
  "total_inputs": 800,
  "strata": {
    "S1_shakespeare": {
      "count": 100,
      "source": "/home/cabdru/contextgraph/shakespear/",
      "license": "public_domain",
      "selection_seed": 42
    },
    "S2_code": {
      "count": 100,
      "source": "stratified mix per spec",
      "license": "MIT/Apache-2.0",
      "selection_seed": 43
    },
    ...
  },
  "inputs": [
    {
      "input_id": "exp2_input_00000",
      "text": "...",
      "stratum": "S1_shakespeare",
      "metadata": {"source_file": "...", "tokens": 247},
      "text_sha256": "..."
    },
    ...
  ],
  "embedder_panel": [
    {"slot": "E1", "name": "E1_Semantic", "weight_sha256": "..."},
    ...
  ]
}
```

The manifest sha256 is the **single hash that anchors all downstream artefacts**. Every per-pair MI result row carries this hash.

## 6. Stratum Construction Notes

### S1 (Shakespeare): Direct Selection
Pull from existing CF_FINGERPRINTS where `source_path` matches Shakespeare files. Filter to non-cleaned (pre-stage-direction-stripping) variants for diversity.

### S2 (Code): Seed-Random + Manual Curation
Seed-random sample from public repos. Operator manually reviews to exclude:
- Boilerplate (license headers, `__init__.py` empty files).
- Copyrighted vendor code (proprietary patterns).
- Duplicates of S1/S5/S6 patterns.

### S3 (Causal): Pattern Mining
Use existing causal_realdata_bench corpus. Filter by surface-marker presence ("because", "therefore", "leads to", "results in", and counter-causal patterns). Operator validates a 20-sentence sample to confirm legitimate causal claims (vs. spurious correlation).

### S4 (Learning Events): Time-Window Sample
Pull 100 events from CF_LEARNING_EVENTS where `event.metadata.timestamp` is within last 90 days, prioritizing diverse `outcome.label` distribution.

### S5 (BEIR): Public Test Set
Use BEIR's test split. License: MIT. No overlap with training corpora of any of your embedders (per BEIR documentation).

### S6 (Wikipedia): Random Sample
`wikipedia` Python package; seed 42; 100 random pages; one paragraph each (random index).

### S7 (Temporal): Synthesize From Real Logs
Compose from real session logs + paper version histories + news timelines. 100 separate snippets. Operator manually structures the temporal context.

### S8 (Multilingual): FLORES Selection
Use FLORES-200 dev split, 50 English-original sentences with translations to 5 target languages. Pre-cleaned by FLORES authors.

## 7. Construction Procedure (T-002 Through T-004)

```
T-002: Build per-stratum source files (parallelizable)
  → 8 JSON files: S1.json ... S8.json, each with 100+ candidate inputs

T-003: Sample exactly 100 from each stratum with seed 42
  → 8 JSON files: S1_sampled.json ... S8_sampled.json, each with exactly 100 inputs

T-004: Merge + validate + sign manifest
  → corpus_manifest.json (signed)
  → corpus_manifest.json.sha256 (separate file with hash for cross-machine verification)
```

## 8. Operator Sign-Off Gate

Per Constitution C16 + C2: the operator personally reviews the corpus manifest before T-005 (embedding extraction) begins. Specifically:

- Open `corpus_manifest.json` in viewer.
- Spot-check 5 inputs per stratum (40 total) for: legitimate text, no encoding issues, fits the stratum description.
- Review per-stratum length distribution histogram.
- Confirm the manifest's sha256.
- Sign-off via committing a `MANIFEST_APPROVED.txt` file with the sha256 and a one-line approval.

If sign-off fails, the corpus is rebuilt — possibly with adjusted sampling or replacement of failing items.

## 9. Frozen Reference: PCA Training Corpus

PCA models in [03_methodology.md](03_methodology.md) §3 are fit on a *separate* 1000-input corpus, distinct from the 800 audit inputs:

- 200 random inputs from each of the 8 strata's *unsampled* pool (1600 candidates) → 1000 random selection.
- Frozen PCA fit; persisted as `pca_per_embedder.npz`.
- Audit chain records the PCA training corpus manifest hash separately.
- **No PCA training data overlaps with the 800-input audit corpus.**

This separation prevents the PCA from over-fitting to the audit set, which would inflate MI estimates.

## 10. License & Re-Distribution

The corpus manifest (text included) is released alongside the open-source harness when:
- All 8 strata have permissive licenses (S1: public domain; S2: MIT/Apache-2.0; S5: BEIR MIT; S6: Wikipedia CC-BY-SA; S8: FLORES CC-BY-SA).
- S3, S4, S7 require operator review:
  - S3: source attribution per sentence; permissively-licensed inputs only.
  - S4: anonymized; consent-validated; aggregated metadata only.
  - S7: same anonymization; sanitized; consent-validated.

If any subset cannot be redistributed, the manifest publishes the *hashes only* with the corpus available privately for replicators-on-request. The numeric results remain public.

## 11. Reusability For Future Experiments

This corpus is built for EXP-2 but reusable for:
- $n_m$ sample-complexity sweep (EXP-3) on the same corpus.
- Future panel evaluations (e.g., when E15 is added; replay against same 800).
- Cross-encoder triangulation (EXP-1) on subset.
- Replication by independent researchers with different embedders.

The corpus is therefore versioned (`exp2_corpus_v1`); future versions can extend with new strata while preserving the original 800.

## 12. What This Corpus Is *Not*

It is **not** a benchmark. It is not optimized for any downstream task. It is purely an instrument for measuring the panel's information geometry. Reviewers may say "your corpus is too small" — the response is: 800 inputs is sufficient for kSG with 32-D and InfoNCE with the critic-size we use; corpus size is justified in [03_methodology.md](03_methodology.md). Reviewers may say "your corpus is too biased toward semantic" — the response is: 8 strata, with explicit attention to E7 (code), E14 (multilingual), E5 (causal), E2-E4 (temporal). Bias is named, not denied.

A second-pass extension experiment with 2000 inputs is named future work. EXP-2 v1 stays at 800 to ship.
