---
namespace: reports
agent_id: agent-02-sherlock-dpo
timestamp: 2026-04-17T20:10:00Z
status: final
tags: final-report, dpo, contrastive-pairs, style-vs-topic, verdict-no, forensic
---

# Final Report — DPO Pair Mining Forensic Audit

## 1. Verdict
**NO.** The 12,567 DPO pairs were NOT mined using style-correct embedders. The pairs are contrastive at the SEMANTIC/STRUCTURAL level, built from cross-embedder anomaly rules in which E1_Semantic (topic) is the pivotal criterion. Chosen and rejected passages in every observed pair have nearly identical style. The DPO training therefore reinforced "prefer this Shakespeare topic over that Shakespeare topic," not "prefer Shakespeare-style over non-Shakespeare-style."

Confidence: HIGH. Evidence is drawn from (a) direct inspection of Rust source for the miner, (b) direct inspection of all Python scripts in the DPO pipeline, and (c) direct statistical analysis of the shipped JSONL files on disk.

## 2. Evidence — Exact File:Line References

### A. Miner rules hinge on semantic, not style
`crates/context-graph-core/src/contrastive/mining.rs:104-134` — `classify_anomaly()`:
- Line 114-116: rule 1 uses E1 (low) as the anomaly pivot: `high E7 + low E1 → CodeShapeButDifferentIntent`
- Line 122-124: rule 3 uses E1 (low): `high E9 + low E1 → HdcRobustButSemanticDifferent`
- Line 130-132: rule 5 uses E1 (high): `high E1 + low E5 → SemanticButNotCausal`
- No rule partitions on "high E1 + low style embedders (E7/E12/E13)" — which is what you would need to surface topic-match, style-miss pairs.

### B. Candidate pool is the whole daemon (Shakespeare-only at the time)
`crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs:175-189` — miner loads `list_fingerprints_unbiased(candidate_pool_size)` from the live store. At ingest time the daemon held the Shakespeare corpus (see `scripts/shakespeare_ingest.py:2-4,110`). Every pair is Shakespeare-vs-Shakespeare. Style-contrast is structurally impossible from this pool.

### C. MCP tool has no knobs for a style-primary / style-contrast embedder
`crates/context-graph-mcp/src/tools/definitions/contrastive.rs:29-96` — schema for `mine_contrastive_pairs` exposes ONLY these parameters:
- `maxPairs`, `minDisagreement`, `highThreshold`, `lowThreshold`, `kinds`, `sessionFilter`, `topKCandidatesPerAnchor`, `candidatePoolSize`
- No `primary_embedder`. No `contrast_embedder`. The tool computes all 13 and applies the fixed classify_anomaly rules. Style-awareness is not even possible at the MCP boundary.

### D. Python DPO pipeline has ZERO style-discriminative filtering
- `scripts/shakespeare_build_dpo_dataset.py:42-48` and `scripts/shakespeare_rlhf.py:47-71` — fetch pairs blindly via `list_contrastive_pairs` and `get_contrastive_pair`, then write them verbatim.
- `scripts/shakespeare_rebuild_all_data.py:96-137` — `clean_dpo_data()` strips stage directions and drops identical-after-cleaning pairs. No embedder re-evaluation, no style check.
- `scripts/shakespeare_adversarial_boost.py` is SFT-only, not DPO. (Uses `sft()` helper; writes `shakespeare_adversarial_boost.jsonl` which is appended to SFT, never to DPO.)

### E. Shipped dataset — 100% anomaly_kind="other"
Direct statistical audit of `/home/cabdru/.contextgraph/training/shakespeare_dpo.jsonl`:
- 12,567 records, all with `anomaly_kind="other"` — meaning NONE of the five named cross-embedder anomaly rules fired. The data slid into the fallback bucket.
- Cleaned version `shakespeare_dpo_clean.jsonl`: 11,724 records, same 100% "other".

### F. Shipped dataset — style embedders are HIGH for both chosen and rejected
Per-embedder similarity statistics across all 11,724 clean pairs (computed from `_meta.similarity_profile`):
- E1_Semantic (topic): mean 0.928, stdev 0.015
- E6_Sparse (keyword): mean 0.162
- E7_Code (STYLE): mean 0.810
- E12_ColBERT (STYLE): mean 0.901
- E13_SPLADE (keyword): mean 0.158
- Chosen and rejected share ~81% E7 style similarity and ~90% E12 style similarity. You cannot teach style preference from data where both sides have the same style.

### G. Chosen passages are heavily reused
616 distinct chosen passages across 11,724 pairs (avg 19 rejections per anchor; max 57 for "1. ACCURACY ABOVE ALL…" — a non-Shakespeare chunk that leaked into the daemon). First ten pairs all share chosen="SCENE I. Coventry (Warwick)" paired against ten different Shakespeare scenes. The prompt for each pair is `"Write about: SCENE I. Coventry"` (from `shakespeare_build_dpo_dataset.py:155` — `f"Write about: {first_line}"`). DPO learns: "given a prompt mentioning Coventry, prefer Coventry scene over Westminster scene" — TOPIC matching, not style discrimination.

## 3. Fix Needed (If NO Was Right)

### Root-cause fix (correct but larger): extend the miner
Add a new AnomalyKind + rule to `crates/context-graph-core/src/contrastive/mining.rs`:
```
StyleMismatch: high E1 (topic agreement) AND
               low on ANY of E7/E12/E13 (style disagreement)
```
Add a corresponding MCP knob so the caller can request ONLY style-mismatch pairs. This requires co-ingesting non-Shakespeare text into the daemon (modern prose, articles, code, etc.) so the miner has a non-Shakespeare pool to contrast against. The current daemon holds only Shakespeare, so even a style-aware rule would find nothing to pair.

### Pragmatic fix (right-size for the current Shakespeare demo):
Rewrite DPO construction in Python, bypassing `mine_contrastive_pairs` entirely:
1. For each prompt that the SFT data already pairs with a Shakespeare response, use the shipped Shakespeare text as CHOSEN.
2. Generate REJECTED by running the base Gemma-4-E4B (not the LoRA) against the same prompt — this produces a modern-English answer to the Shakespeare prompt. Off-STYLE by construction.
3. Optionally score each (chosen, rejected) pair against the `Shakespeare — Sonnets` constellation via `score_against_constellation` (see `memory/discoveries/coordinator--constellation-anchors-validated.md`); keep pairs where `combined_score_chosen ≥ 0.35` and `combined_score_rejected < 0.30` to ensure style contrast is real.
4. Write to `shakespeare_dpo_style_corrected.jsonl`.
5. Retrain the DPO phase of `scripts/shakespeare_full_pipeline.py` with this new file.

Specific files to change:
- NEW: `scripts/shakespeare_build_dpo_style_pairs.py` — replaces both `shakespeare_build_dpo_dataset.py` and `shakespeare_rlhf.py` for the Shakespeare demo.
- EDIT: `scripts/shakespeare_full_pipeline.py:499` — change `dpo_path` to the new JSONL.
- EDIT: `scripts/shakespeare_rebuild_all_data.py:96-137` — add per-pair style-score filtering via `score_against_constellation`; drop pairs where chosen doesn't clear 0.35 or rejected doesn't fall below 0.30.

### Verification after fix
Run the same statistics script over the new DPO file. Expected:
- E1 mean still high (~0.9) — topics match
- E12 mean should split: chosen-vs-Shakespeare-reference high, rejected-vs-Shakespeare-reference low; within-pair E12 between chosen and rejected should be LOW (~0.3-0.5 range)
- `combined_score` against Sonnets constellation: chosen mean ≥ 0.40, rejected mean ≤ 0.28

## 4. Confidence and What I Could Not Verify

**HIGH confidence** on the main verdict. The evidence is a triangle: Rust source + Python pipeline + actual shipped data all point the same direction.

**Could NOT verify**:
- The exact command-line that originally ran `mine_contrastive_pairs` to populate CF_CONTRASTIVE_PAIRS. The resulting rows landed in RocksDB; no audit log survives. I infer the tool was called with no `sessionFilter` (or a Shakespeare-only filter — makes no difference to the verdict) because all chosen/rejected texts are Shakespeare.
- Whether there was a second, unshipped dataset mined with style-aware intent. I found only the two JSONLs (`shakespeare_dpo.jsonl`, `shakespeare_dpo_clean.jsonl`), plus an empty `shakespeare_dpo_creative.jsonl`. Neither DPO-consuming caller in the training pipeline references any other path.
- Whether the training actually plateaued before reaching the "prefer topic-match" pathology — DPO loss 0.47 with rewards/accuracies 98.0% per SITREP suggests strong convergence, but the *content* of the preference learned is what this report contests, not the convergence itself.

## 5. Memory Files Written
- `memory/discoveries/agent-02-sherlock-dpo--pair-mining-uses-semantic-not-style.md`
- `memory/journal/agent-02-sherlock-dpo--success-forensic-verdict.md`
- `memory/reports/agent-02-sherlock-dpo--final-report.md` (this file)

## 6. Chain of Custody
All statements in this report are supported by:
- Rust sources in git (commit `8ff6df4d` and earlier; none of the relevant Rust files are in `git status` as modified)
- Python sources in git (same)
- Training JSONL files in `~/.contextgraph/training/` with mtimes 2026-04-15 through 2026-04-16 (produced before the cleanup commits)
- Statistics reproducible with `python3` reading the JSONLs directly (see commands in `memory/journal/agent-02-sherlock-dpo--success-forensic-verdict.md`)

Case closed.
