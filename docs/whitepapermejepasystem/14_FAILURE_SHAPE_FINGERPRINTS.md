# 14 — Failure-Shape Fingerprints: The First-Class ME-JEPA Object

**Status:** Canonical methodology document. Supersedes the looser "cell" language in `07_TELEOLOGICAL_CONSTELLATIONS.md`.

**Relation to PAPER.md:** This doc is the direct application of Chris Royse's *Teleological Constellation Training* (TCT) protocol (PAPER.md §§3–5) to the code-verification problem. The constellation $C_P$ that PAPER.md uses for identity-locked generation is the same mathematical object ME-JEPA uses for failure-mode classification, with a different referent: instead of "the geometric definition of person P," it's **"the geometric definition of failure-shape fingerprint F."** The three-role composition (retriever / labeller / runtime guard) over the same 12 frozen embedders transfers without modification.

---

## 0. The thesis in one paragraph

ME-JEPA-Code stores and trains against a **catalog of failure-shape fingerprints**. Each fingerprint is a frozen multi-embedder centroid set $C_F$ that encodes "this is what a patch of failure mode $F$ looks like across all 12 embedders simultaneously." The catalog has three typed entries: **KnownGood** (Pass-shaped — the gold-patch geometry), **KnownBad** (Fail-shaped — one per documented failure mode), and **Unknown** (out-of-distribution — patches that match no known fingerprint, queued for operator labeling). The verifier's job for a new patch is to (1) compute its 12-embedder joint embedding, (2) score it against every fingerprint in the catalog via the $G_\tau$ guard, (3) emit the typed verdict (Pass / Fail-with-fingerprint-name / Unknown), and (4) when Unknown, route to active learning so the catalog grows. The system gets better at edge cases the more it encounters them.

---

## 1. The fingerprint object, formally

**What's actually running on aiwonder right now** (verified against shard 0 packed_records 2026-05-15T23:30Z):

| Tier | Count | Slots | Role |
|---|---|---|---|
| Neural GPU embedders (the heavy lifters) | **8** | E1, E6, E7, E8, E10, E12, E13, E14 | Carry the discriminative signal — what fingerprints actually separate on |
| Algorithmic CPU embedders (free accompaniments) | 4 | E2, E3, E4, E9 | Cheap positional / cyclic / hyperdimensional encodings; cost no GPU |
| **Forward-cache total** | **12** | — | All 12 produce packed_records per chunk |

**The fingerprint catalog is built from the 8 neural embedders.** The 4 algorithmic ones supplement at near-zero cost and may participate in specific fingerprint definitions (especially for temporal / cyclic failure patterns), but the heavy discriminative signal lives in the 8 neural slots. Per-fingerprint weight profiles ([`06_EMBEDDER_STACK.md`](06_EMBEDDER_STACK.md) §"Layer 3") can further downselect: some fingerprints may calibrate sharply against E7 (Qodo code) + E1 (semantic) + E_DataFlow only, weighting the others at zero.

Let $D$ be the chunk corpus. Let $\Phi_{\text{neural}} = \{\phi_e\}_{e \in \{1,6,7,8,10,12,13,14\}}$ be the 8 frozen **neural** content embedders that carry the discriminative signal. Let $\Phi_{\text{algorithmic}} = \{\phi_e\}_{e \in \{2,3,4,9\}}$ be the 4 algorithmic embedders that supplement. A **failure-shape fingerprint** is:

```rust
pub struct FailureShapeFingerprint {
    /// Stable identifier. SHA-256 of the canonical key tuple.
    pub fingerprint_id: FingerprintId,

    /// Typed kind — answers "what does this fingerprint mean?"
    pub kind: FingerprintKind,

    /// Human-readable name. Required.
    pub name: String,                            // e.g. "subtle_flip_comparison_operator"

    /// Frozen per-embedder centroid set (PAPER.md §5.1 C_P, applied to failure modes).
    pub centroid_per_embedder: HashMap<EmbedderId, Vec<f32>>,   // 12 vectors

    /// Per-embedder variance — used for confidence calibration.
    pub variance_per_embedder: HashMap<EmbedderId, f32>,

    /// Per-embedder Gτ acceptance threshold (PAPER.md §5.4).
    pub tau_per_embedder: HashMap<EmbedderId, f32>,

    /// Aggregate-pair information: cosine + MI for the 66 pairs.
    /// Captures cross-embedder fingerprint geometry.
    pub pairwise_cosine: HashMap<(EmbedderId, EmbedderId), f32>,
    pub pairwise_mi: HashMap<(EmbedderId, EmbedderId), f32>,

    /// Reference chunks that constitute the fingerprint's geometric definition.
    pub reference_chunks: Vec<ChunkId>,
    pub n_references: usize,

    /// Oracle outcome (deterministic for KnownGood/KnownBad; absent for Unknown).
    pub oracle_outcome: Option<OracleOutcome>,
    pub exception_class: Option<ExceptionClass>,

    /// Provenance.
    pub created_at: Timestamp,
    pub last_refreshed: Timestamp,
    pub source_corpus: CorpusVersion,

    /// Active-learning state.
    pub is_canonical: bool,                      // operator-confirmed?
    pub confidence: FingerprintConfidence,       // calibrated by holdout performance
}

pub enum FingerprintKind {
    /// Patches that pass the Docker oracle. The "gold-patch geometry."
    KnownGood {
        gold_patch_variants: usize,              // count from corpus
    },

    /// Patches that fail the Docker oracle in a specific named way.
    KnownBad {
        mutation_category: MutationCategory,     // subtle_flip | off_by_one | ...
        failure_mode_description: String,        // human-readable
        nearest_oracle_exception: ExceptionClass, // TypeError | AssertionError | ...
    },

    /// Out-of-distribution. Doesn't match any known fingerprint.
    /// Lives in this state until operator labels it.
    Unknown {
        observed_at: Timestamp,
        observed_by_session: SessionId,
        ood_score: f32,
        embedder_disagreement_score: f32,
        active_learning_priority: u8,            // 1=high, 5=low
    },
}
```

Persisted to RocksDB CF `CF_MEJEPA_FAILURE_FINGERPRINTS`, keyed by `fingerprint_id`.

## 2. The catalog at seed

After the seed corpus run (300 SWE-bench Lite Python × 8 mutations × 12 embedders) and the constellation-builder pass, the initial catalog has:

| Kind | Count at seed | Source |
|---|---|---|
| `KnownGood` | 1 base + 12 per-repo = **13** | Gold patches across the 12 repos (django, sympy, matplotlib, sklearn, pytest, sphinx, astropy, requests, flask, xarray, seaborn, pylint) |
| `KnownBad` | 7 base × 12 repos = **84** | One per (mutation_category × repo): subtle_flip_django, off_by_one_sympy, ... |
| `Unknown` | 0 at seed | Grows live |
| **Total** | **97 fingerprints at seed** | Phase G build |

These 97 are the **catalog floor**. The 30-case adversarial corpus (issue #279) adds ~30 more named fingerprints for the gameable failure modes (`SubtleBugPassesTests`, `KnownGoodLookalike`, `EmbedderGaming`). Live operator usage grows the catalog from there.

## 3. The verification flow (replacing per-cell correlation)

When a new patch arrives, the predictor:

```
1. Compute 12-embedder joint embedding of the patch's chunks.

2. Score against the entire fingerprint catalog:
   for each F in catalog:
       g_τ(F) = ∀ e ∈ active_embedders: cos(φ_e(patch), F.centroid_per_embedder[e]) ≥ F.tau_per_embedder[e]
       if g_τ(F) holds:
           matched.append(F, similarity_score=mean_cosine_to_F)

3. Apply typed decision logic:
   if any matched F has kind=KnownGood and no KnownBad matched:
       emit Verdict::Pass
       attach: best_matching_good_fingerprint
   else if any KnownBad matched:
       emit Verdict::Fail
       attach: nearest_failure_fingerprint, predicted_exception_class, top-3 ranked
   else if all g_τ failed (no fingerprint matches):
       emit Verdict::OutOfDistribution
       enqueue chunk to CF_MEJEPA_ACTIVE_LEARNING_QUEUE with Unknown fingerprint placeholder
       priority = f(ood_score, embedder_disagreement_score)
   else:
       emit Verdict::Abstain (mixed signals, no clear winner)
```

This is **fingerprint classification**, not real-valued regression against oracle probability. The output is structurally explainable: *"this patch matches fingerprint `subtle_flip_django_comparison_operator` with mean cosine 0.94 across 12 embedders; the nearest historical exemplar is `django__django-12286#subtle_flip`."*

## 4. Edge-case capability (the new-fingerprint loop)

ME-JEPA must handle patches it has never seen before. When the typed decision logic emits `Verdict::OutOfDistribution`, the system does **NOT** fail — it activates the lifelong-learning loop:

```
1. Patch goes to CF_MEJEPA_ACTIVE_LEARNING_QUEUE.
2. The chunk's 12-embedder joint embedding is preserved.
3. The chunk's actual oracle outcome (once observed, via test run or operator override) becomes the label.
4. If multiple unlabeled chunks cluster in the same OOD region (≥ K with cosine similarity ≥ τ_cluster):
     a. The cluster becomes a candidate new fingerprint.
     b. Operator is notified via `mejepa active-learning suggest-new-fingerprint`.
     c. Operator labels: confirms kind (KnownGood/KnownBad), assigns name + exception class.
     d. New fingerprint is added to catalog, persisted to CF_MEJEPA_FAILURE_FINGERPRINTS.
     e. Next training cycle pulls the new fingerprint into the replay buffer at 6× weight.
5. The catalog grows monotonically. The system never forgets a fingerprint (EWC protection).
```

This is the **"optimized for edge case learning"** property. The catalog at seed is 97 fingerprints. After six months of live use, it might be 500. After a year, 1,000. Each new fingerprint encodes a failure mode the operator's codebase actually produces, not a hypothetical one the corpus designers thought of in advance.

**Active-learning priority levels:**

| Priority | When assigned | Behavior |
|---|---|---|
| **1 — critical** | Cluster of ≥ 10 OOD patches with similar geometry | Operator alerted immediately; blocks corpus retrain until resolved |
| **2 — high** | OOD patch with embedder disagreement > 2σ | Surfaces in `mejepa active-learning list` first |
| **3 — medium** | Single OOD patch | Joins queue tail |
| **4 — low** | OOD patch with near-match to existing fingerprint | Suggested re-classification of existing fingerprint |
| **5 — defer** | OOD patch on a deprecated cell | Archive without alerting |

## 5. The good-vs-bad asymmetry

Verification has a structural asymmetry the original TCT didn't need:

| | KnownGood | KnownBad |
|---|---|---|
| **Per fingerprint, what does $G_\tau$ match mean?** | "This patch looks like a known-passing patch" | "This patch looks like a known-failing patch" |
| **What's the verdict on match?** | `Verdict::Pass` (with caveats from Q4) | `Verdict::Fail` (with predicted exception class) |
| **What's the verdict on non-match?** | Possibly Pass via Q4 silence; possibly OOD | Not Fail; need positive match elsewhere |
| **How is the centroid built?** | L2-normalized mean of `known_good` cell members | L2-normalized mean of `mutation_category=X` cell members |
| **How is $\tau$ calibrated?** | Same-session-band floor (10th percentile of `known_good` calibration set) | 10th percentile of mutation-category calibration set |
| **What happens at saturation?** | Cell auto-subdivides by repo, AST node type, framework | Cell auto-subdivides by specific exception class, failure location |
| **Operator override behavior** | Promote a chunk to canonical KnownGood reference | Mark a chunk as canonical KnownBad reference, set its exception class |

The verifier must check **both** sides of the catalog before emitting Pass. A patch can simultaneously match `KnownGood:django_simple_function` AND `KnownBad:subtle_flip_django_comparison_operator` — that's the **conflict** case, which emits `Verdict::Abstain` and routes to operator review.

## 6. Calibration: per-fingerprint, not per-cell-correlation

The original ship gate metric (`prediction_oracle_correlation ≥ 0.95 × 4 windows × per cell`) becomes:

```
For every fingerprint F in catalog:
    classification_accuracy(F) ≥ 0.95          // patches that should match F do match F
    classification_precision(F) ≥ 0.95         // patches that match F should match F (no false ID)
    unknown_recall ≥ 0.90                      // patches that are truly OOD are emitted as Unknown
    must be stable across 4 rolling windows
```

This is per-fingerprint, not per-aggregate. It enforces:
1. Every KnownGood fingerprint is correctly identified ≥ 95% of the time it appears.
2. Every KnownBad fingerprint is correctly identified ≥ 95% of the time it appears.
3. False positives across fingerprints stay below 5%.
4. Truly novel patches get emitted as Unknown at ≥ 90% rate (not silently misclassified as a known fingerprint).

The ship gate becomes:

```
For all F in catalog: accuracy(F) ≥ 0.95 AND precision(F) ≥ 0.95
AND unknown_recall ≥ 0.90
AND stable across 4 consecutive rolling windows
```

When a new fingerprint joins the catalog (operator-labeled from active-learning queue), it enters in a "calibrating" state. After K observations, its threshold $\tau$ is calibrated and it joins the gated set.

## 7. The three roles, applied (PAPER.md §3.4 → ME-JEPA)

PAPER.md observes that the same frozen embedder panel $\Phi$ serves three roles simultaneously, over **identical parameter instances**, without any parameter update:

| Role | PAPER.md | ME-JEPA-Code application |
|---|---|---|
| **Retriever** | Context Graph ranks stored content by cosine | `mejepa_routing_lookup` ranks fingerprints in the catalog by similarity to a query patch |
| **Labeller** | DDA produces $n \cdot (N + \binom{N}{2})$ supervisory signals on a fixed corpus | Constellation builder produces 155 reward signals per chunk × 399,777 chunks = ~62 M signals to materialize the fingerprint catalog |
| **Runtime guard** | $G_\tau(\hat y; C_P) = \mathbb{1}[\forall m : \cos(\phi_m(\hat y), c_m) \geq \tau_m]$ for the generated output | $G_\tau(\text{patch}; C_F) = \mathbb{1}[\forall e : \cos(\phi_e(\text{patch}), c_e^F) \geq \tau_e^F]$ for the live patch, executed at MCP `mejepa_predict_what_if` time |

No new infrastructure. Three uses of the same 12 frozen embedders.

## 8. The DDA counting identity, applied

PAPER.md §3.1: $|D'| = n \cdot (N + \binom{N}{2})$ structured signals. For ME-JEPA at $N=12$:

| Term | Count per chunk |
|---|---|
| Per-embedder cosine to each fingerprint's centroid | $|catalog|$ × 12 (one per embedder × catalog size) |
| Pairwise cross-embedder cosine | 66 |
| Pairwise cross-embedder MI | 66 |
| Blind-spot z-scores (primary vs enhancer pairs) | 11 |
| **Total (excluding catalog scaling)** | **155 per chunk** |
| × 399,777 chunks in seed corpus | **~62 M reward signals** |

At catalog size 97: **155 + (97 × 12) = ~1,319 signals per chunk** including catalog scoring. This is the substrate the predictor head trains against.

## 9. Storage layout

```
CF_MEJEPA_FAILURE_FINGERPRINTS              <fingerprint_id> → FailureShapeFingerprint
CF_MEJEPA_FINGERPRINT_REFERENCES            <fingerprint_id, chunk_id> → reference membership record
CF_MEJEPA_FINGERPRINT_REVERSE_INDEX         <chunk_id> → Vec<fingerprint_id>  (which fingerprints does this chunk belong to)
CF_MEJEPA_ACTIVE_LEARNING_QUEUE             <queue_position> → UnknownCandidate (chunk_id + ood_score + cluster_proposal)
CF_MEJEPA_FINGERPRINT_CALIBRATION           <fingerprint_id> → ConformalResidualDistribution
CF_MEJEPA_FINGERPRINT_AUDIT                 <fingerprint_id, ts> → audit entry (catalog mutations)
```

All RocksDB CFs, sync_all=true on writes, independent reopen-readback for FSV evidence.

## 10. MCP tool surface (additions / renames)

| Tool | Job |
|---|---|
| `mejepa_fingerprint_list` (NEW) | Return the current catalog: name, kind, n_references, calibration state |
| `mejepa_fingerprint_inspect` (rename of `mejepa_constellation_inspect`) | Return one fingerprint's full geometry: centroid, variance, threshold, references |
| `mejepa_fingerprint_classify` (NEW) | Classify a given chunk against the catalog; return matched + ranked fingerprints |
| `mejepa_fingerprint_suggest_new` (NEW) | List the active-learning queue's cluster proposals for new fingerprints |
| `mejepa_fingerprint_label` (NEW) | Operator labels a queue entry; new fingerprint added to catalog |
| `mejepa_fingerprint_promote_canonical` (NEW) | Mark a chunk as a canonical reference for a fingerprint |
| `mejepa_fingerprint_recalibrate` (NEW) | Force recalibration of one fingerprint's $\tau$ |
| `mejepa_predict_latest` (existing) | Now returns fingerprint matches in addition to Q1-Q5 |
| `mejepa_predict_what_if` (existing) | Now returns fingerprint matches |
| `mejepa_explain_prediction` (existing) | Now returns the matched fingerprint's reference chunks + centroid distances |
| `mejepa_ship_gate_status` (existing) | Now reports per-fingerprint accuracy/precision/recall instead of per-cell correlation |

## 11. Why this is the right methodology

Five reasons this beats per-cell oracle correlation as the central object:

1. **Explainability.** Operator sees "this patch matches fingerprint `subtle_flip_django_comparison_operator` (0.94 confidence) — nearest exemplar `django__django-12286#subtle_flip`." That's a verdict the operator can act on. A real-valued correlation score isn't.

2. **Composability with the typed catalog.** Adding a new failure mode = adding a new fingerprint to the catalog. No retraining of a regression head; just new centroids + new threshold + recalibration.

3. **OOD by construction.** If no fingerprint matches, the verdict is `OutOfDistribution` — not a degraded confidence on the regression. The system says "I don't know," and the active-learning loop fixes that.

4. **Aligns with PAPER.md theoretical framework.** TCT, DDA, meaning compression, three-role composition — all the math transfers. The verifiability property (deterministic $O(M \cdot d_{\max})$ test, no auxiliary learned model) holds.

5. **Lifelong learning.** New fingerprints get added; old fingerprints stay stable (EWC + CBP); the catalog grows with the operator's codebase. The system's exposure to edge cases improves the system, doesn't degrade it.

## 12. What changes in the system right now

To reflect "fingerprints are the first-class object" methodology:

- **Whitepaper:** This doc (14) becomes the canonical methodology statement; `07_TELEOLOGICAL_CONSTELLATIONS.md` is retconned as the "cells = fingerprints at the corpus level" preamble.
- **GitHub issues:** Phase G issues are reframed in terms of catalog construction + per-fingerprint calibration. A new EPIC issue stitches them together.
- **Phase G plan (`docs/futurebuild/27_PHASE_G_ROADMAP.md`):** The "per-cell correlation ≥ 0.95" gate is restated as "per-fingerprint accuracy ≥ 0.95 + unknown_recall ≥ 0.90."
- **MCP tool surface:** Tools above are filed as issues.
- **Storage:** CF_MEJEPA_FAILURE_FINGERPRINTS and friends are added to the registry.
- **Calibration:** Per-fingerprint instead of per-cell.

This is not a re-architecture. It's a **rename + refocus**. The frozen-instrument panel, the 12-embedder forward cache, the predictor head, the conformal calibrator — all of it stays. What changes is what the output looks like: a typed catalog of named failure-shape fingerprints, with a growing OOD frontier, instead of a real-valued correlation score.

## 13. References

- PAPER.md §§3 (DDA), 4 (production systems), 5 (TCT protocol), 6 (case studies)
- alldata.md §§4 (Teleological Constellation), 6 (Constellation Guard), 11 (Theoretical Framing)
- `docs/futurebuild/27_PHASE_G_ROADMAP.md` (operational plan)
- `docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md` (reward signal math)
- `docs/whitepapermejepasystem/07_TELEOLOGICAL_CONSTELLATIONS.md` (preamble / cell-level summary)
- `docs/whitepapermejepasystem/04_FIVE_OPERATOR_QUESTIONS.md` (Q1-Q5 with fingerprint outputs)

---

**One-line summary:** ME-JEPA-Code is a growing, typed catalog of failure-shape fingerprints — KnownGood, KnownBad, and Unknown — each frozen as a 12-embedder centroid set; the verifier's job is to classify new patches against the catalog and route the unclassifiable ones to a lifelong-learning loop that grows the catalog with the operator's actual codebase.
