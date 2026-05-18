# 30 — Self-Designed, Self-Trained, Self-Promoted Embedders (Apex Deliverable of EPIC #321)

> **Authority.** This document captures the operator-directed apex deliverable of EPIC #321 (explanatory-knowledge programme). Operator framing (2026-05-17): *"a new ontology extension mechanism. wouldn't this just be a new embedder? what if the system had the ability to create train and plug in new embedders and start learning and taking advantage of them and it could itself design train and know what it needs and self improves on itself."*
>
> **Companion issue:** [TASK-EK-018 / #341](https://github.com/ChrisRoyse/contextgraph/issues/341).
> **Parent EPIC:** [#321](https://github.com/ChrisRoyse/contextgraph/issues/321).
> **Source analysis:** `docs/explanatory_knowledge_and_mejepaupdated.md §10.2(II), (II-bis), (II-ter), (XI)–(XV)` collapse into this single primitive.

---

## §1 — Why this doc exists

The §10.2 series of additions to `docs/explanatory_knowledge_and_mejepaupdated.md` — (II) ontology-extension, (II-bis) absence-as-existence with NCD/GCD/K-SVD/DP-mixture/boosting prior art, (II-ter) mincut as structural-hole detector, (VIII) bidirectional corpus training, (IX) reverse-prediction MCP tool, (X) operator-contribution surface, (XI) chunk foundationality scoring, (XII) foundationality-weighted BatchSampler, (XIII) foundationality-weighted EWC++ Fisher, (XIV) bedrock-consistency verifier, (XV) hierarchical compression — were originally written as separate engineering additions across GitHub issues (#322, #323, #324, #325, #326, #327, #330, #334, #339, #340, etc.). Intent/north-star items #332/#333 are retired by #358 and are not part of the active self-designed-embedder architecture.

The operator's 2026-05-17 question collapses all fourteen into **one underlying architectural primitive**: *the embedder panel is no longer fixed at instrument-bind time; it grows under the system's own operation, gated by reality-anchored falsification.*

This document is the canonical reference for that primitive. Every other ontology-related EK document, issue, and code path becomes a *consumer* of the primitive specified here once TASK-EK-018 ships.

---

## §2 — The apex claim, in one sentence

**When the system detects a structural absence its existing N embedders cannot capture, it synthesises a candidate embedder, trains it against a reality-anchored objective, gates promotion on whether the candidate measurably improves predictor calibration on held-out data the proposing module did not see, and registers survivors permanently into the frozen-instrument panel with full provenance.**

This is the operator-perfect form of:
- **Deutsch's universal-explainer property** at the substrate level (the system grows its own ontology, and the new ontology elements are hard-to-vary because they must capture a measurable signal the existing N miss).
- **Schmidhuber's Gödel-machine self-rewriting agent** (1990s–2003) specialised to the JEPA frozen-instrument substrate so that the rewrite is collapse-immune by construction (target side never receives gradients; new embedders are frozen on promotion).
- **The operator's epistemic-symmetry framework** (`THE_SYMMETRY_OF_KNOWING` constructs 1–5 by Royse, abstract + §1.1–§1.3 archived at `docs/the_symmetry_of_knowing_partial.md`) applied to embedders as measurable residual-signal detectors — each new embedder is the path to a precisely specified code-reality target: *"reduce the predictor's residual variance on the held-out window the proposing module did not see."*

---

## §3 — Architecture: the six pieces

### §3.1 — Absence detection (substrate already shipped)

The system has already shipped (or has issues for) every signal needed to detect a structural absence in the embedder panel:

| Signal source | Signal shape | Existing issue / commit |
|---|---|---|
| Mincut + sparsest-cut on embedder-similarity graph | "this partition of embedders is most weakly connected" | TASK-EK-003 / #324; shipped substrate at commit `35b270ec` (2026-05-17 22:10) |
| Unknown-fingerprint clusters from active-learning queue | "this cluster of predictions has coherent cross-embedder signature but matches no existing centroid" | TASK-FP-005 / #305 shipped 2026-05-17 |
| Pairwise-MI audit residual | "embedders E_j and E_k have unexpectedly high pairwise MI on cell c; one of them is redundant" | Substrate shipped at commit `35b270ec`; consumer-side audit covered by #122 refactor |
| OOD score per prediction | "this prediction lives outside the in-distribution support" | Live |
| Curiosity score | "this entry would most increase $\mathrm{CP}_\Phi$ if labelled" | TASK-EK-006 / #327 |
| Foundationality variance | "the existing embedders fail to distinguish high-foundationality from low-foundationality chunks" | TASK-EK-016 / #339 |

Each signal produces a **structured absence-shape descriptor**: which input modality the candidate must consume, what dimensionality is suggested, what objective targets it should be trained against, what reality channel will falsify it.

The collected descriptors live in `CF_MEJEPA_EMBEDDER_PROPOSALS` keyed by `proposal_id`, with a ranked priority queue surfaced via a new MCP tool `mejepa_pending_embedder_proposals`.

### §3.2 — Candidate-embedder architecture synthesis (v1 = (a) + (b); v2 = (c))

**Sub-case (a) — Algorithmic / deterministic embedders.**

The LLM (System 1, untrusted per CLAUDE.md §15.1) writes a Rust function from the absence-shape to a candidate embedder. E2, E3, E4, and E9 are the existing examples of this shape (sinusoidal time PE, recurrence time PE, session position PE, hyperdimensional codebook — all deterministic functions, not learned models).

This is the AlphaEvolve pattern (TASK-EK-008 / #329) applied to embedder kernels instead of patches: the LLM mutates, the deterministic evaluator scores, the best surviving kernel becomes the candidate. The system has full FSV discipline around the resulting Rust code (compile, test, fuzz, sandbox).

Feasibility: high. Tooling reuses the existing FunSearch/AlphaEvolve scaffold scoped for TASK-EK-008.

**Sub-case (b) — Small learned head on an existing embedder.**

Take E7's frozen 1536-d output (or E1, E10, E14 — any existing content embedder). Attach a 128-d (or 256-d) projection head with a contrastive objective targeting the absence-shape. ~200 lines of training code; minutes to train per candidate on a single RTX 5090.

The projection head is the only learnable parameter set during training; the underlying embedder remains frozen (preserving the JEPA $\Delta\Xi.\text{target\_collapse} = 0$ invariant for the shared backbone). Once the projection head is promoted, it is itself frozen and registered as a new `EDynamic` slot.

Feasibility: high. Standard contrastive-head training; ~1 GPU-hour per candidate.

**Sub-case (c) — Fully new neural embedder from scratch.**

Neural-architecture-search territory. Defer to v2 of TASK-EK-018. The constraint is not theoretical (it's clearly possible); it's that the iteration loop for NAS is days-to-weeks per candidate, which is too slow for the v1 cadence and too expensive to be the default.

### §3.3 — Reality-anchored training objective (three sources)

**(i) Entity-dependency-graph KGE.**

TransE, RotatE, or HyperComplEx (Nov 2025 SOTA) trained on the corpus's own call / import / inheritance / type / test-verifies / patch-modifies graph. This is the asymmetric / relational signal that retired E5 (causal asymmetric, nomic-embed-based) and retired E11 (KEPLER, Wikidata5M-trained) once provided, restored without external-corpus contamination because the entities are the operator's own corpus.

**This is the natural first concrete instance of TASK-EK-018 / #341 v1.** The system identifies the asymmetric/relational structural gap via the absence-detection signals from §3.1, proposes TransE-on-corpus-entity-graph as the candidate, trains it on the existing dependency graph (shared substrate with TASK-EK-016 / #339), evaluates the held-out improvement, and promotes if it survives. **Not a recommendation a human makes; a finding the system arrives at.**

**(ii) Targeted blind-spot contrastive.**

Train the candidate to disagree with the existing embedders precisely where the mincut named the structural hole. Loss is contrastive against the existing N-embedder ensemble's consensus on a held-out shard. The candidate's representation should land *between* the two cliques the mincut identifies, capturing the relation across the structural hole.

**(iii) Distillation from LLM-proposed similarity function.**

LLM proposes a similarity function in natural language: *"two chunks are similar iff they both manipulate the same external resource"* (or any other relation outside the existing 12-embedder span). The candidate is trained to match that proposed similarity on the corpus. This is the most "creative" of the three sources because the LLM can propose relations no human has yet thought to instrument.

### §3.4 — Falsification gate (the Popperian discipline)

**Held-out-improvement criterion** applied per candidate:

1. Sample a held-out window the proposing module did not see (the active-learning queue, the mincut signal, the foundationality variance — whichever surfaced the absence — must not have observed this window during proposal generation).
2. Train predictor with `N` embedders → measure per-cell holdout correlation `r_N`.
3. Train predictor with `N+1` embedders (existing N plus the candidate) → measure per-cell holdout correlation `r_{N+1}`.
4. **Promote iff `r_{N+1} - r_N ≥ δ_global` (default `δ_global = 0.005` per the existing TASK-G-016 per-cell-promotion gate)** AND no cell regresses by more than `δ_cell` (default `δ_cell = 0.005`).

Rejected candidates persist to `CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS` with full provenance so the system does not re-propose the same architecture in the next round. **This is the Popperian discipline that prevents the panel from filling with junk embedders.** Without it the system can fit to residuals indefinitely and produce overfit panels that do not generalise.

### §3.5 — Dynamic registration (the biggest wiring lift)

The routing table moves from compile-time (`crates/context-graph-mejepa-embedders/src/routing.rs`) to runtime-mutable:

- **`EmbedderId` enum becomes extensible.** Existing E1–E21 stay; add `EDynamic(u32, String)` variant for self-promoted embedders. `u32` is a stable promotion sequence number; `String` is the operator-friendly name (e.g., `"corpus_transe_v1"`, `"e7_external_resource_head_v3"`).
- **Pairwise-MI matrix cardinality grows $\binom{N}{2} \to \binom{N+1}{2}$ on each promotion.** All consumers of the matrix (DDA signal compute, mincut over embedder graph, blind-spot z-score features) must handle dynamic cardinality. Cached matrices invalidate on promotion.
- **Constellation centroids per cell gain a dimension for the new embedder.** Existing centroids in `CF_MEJEPA_CONSTELLATION` are re-fitted via the same Phase 1 procedure for affected cells.
- **Gτ guard gains a cosine threshold for the new embedder**, calibrated via the same same-session-band procedure as existing thresholds (`PAPER.md §5.4`).
- **DDA counting identity (`PAPER.md §3.1` Identity 1) becomes time-varying.** Updated form:
  ```
  |D'(t)| = n · (N(t) + binom(N(t), 2) + foundationality_term)
  ```
  where `N(t)` is the active embedder count at time `t`. The static "up to 91 signals per input at N=13" headline becomes a time-graded curve.
- **Persistence.** Each dynamic embedder lives at `/mnt/d/contextgraph/models/dynamic/<embedder_id>/` with:
  - Training certificate hash-chain.
  - Witness chain entry.
  - Forward-pass implementation (compiled Rust shared library for sub-case (a), PyTorch / Candle checkpoint for sub-case (b)).
  - Full provenance per §3.6.
- **Hot-swap.** The panel update happens atomically via a new `CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY` version increment. In-flight prediction queries either complete with the old panel or restart with the new one; the daemon supervisor handles the cutover the same way it handles A/B/C model promotion.

### §3.6 — Freeze + audit (preserving the JEPA invariant)

Once promoted, the new embedder is frozen permanently. No further training without going through the full A/B/C promotion gate again. This preserves the JEPA `ΔΞ.target_collapse = 0` property by extension: the target panel grows in dimensionality, but every dimension is a deterministic function of bytes-on-disk (sub-case (a)) or a frozen learned function with no gradient flowing into it during downstream predictor training (sub-case (b)).

Full provenance per promoted embedder, persisted to `CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE`:

- The residual signal that triggered the proposal (which absence-detection signal, which window, which rank in the priority queue).
- The architecture generator (which sub-case (a)/(b)/(c), which LLM model version, which prompt template hash).
- The training certificate hash chain.
- The held-out improvement measurement (per-cell deltas + global delta + which held-out window).
- Operator approval if catastrophic-class (cross-ship-gate-threshold promotions require `mejepa_promote_approval` per existing Phase E).
- Final forward-pass artefact hash (the binary or checkpoint that the system will execute when the embedder is invoked).

---

## §4 — Operator-in-the-loop discipline (CRITICAL — non-negotiable)

The auto-promotion path applies only when:

1. The held-out improvement criterion passes by margin > `δ_global`.
2. No cell regresses by more than `δ_cell`.
3. The candidate does not push the panel over the VRAM budget (existing gate).
4. The candidate is not functionally redundant with an existing embedder (pairwise-MI ≤ 0.95 to every E_k).
5. The catastrophic-class flag is NOT set (the promotion would not cross the ship-gate threshold or modify a cell currently below threshold).

If condition (5) fails, the promotion requires two distinct operator approvals via `mejepa_promote_approval` before the embedder is registered. This preserves the existing Phase E discipline applied to model promotion, extended to embedder promotion.

Additionally, **the first time a candidate is auto-promoted in a deployment**, operator approval is required regardless of the catastrophic flag. This is a cold-start discipline that ensures the operator sees the first self-promoted embedder before the system starts auto-promoting.

---

## §5 — Three convergent verdicts

| Framework | Pre-TASK-EK-018 verdict | Post-TASK-EK-018 verdict |
|---|---|---|
| **Deutsch (universal-explainer)** | Strict no — ontology is closed, fixed at instrument-bind time. Weak yes possible with (II)–(V) additions but they treat ontology growth as a discrete primitive. | **Strong yes at the substrate level.** The system grows its own ontology under its own operation. Each new embedder must capture a measurable signal the existing N miss (hard-to-vary by the falsification gate). Each new embedder has reach beyond the residual that proposed it (because it must survive held-out evaluation on a window the proposing module did not see). This is exactly the property Deutsch identifies as universal-explainer-by-construction. |
| **Schmidhuber (compression progress = understanding)** | Substrate density (DDA) shipped; predictor compression-progress shipping via TASK-EK-001 / #322. Static-substrate-density does not satisfy the *dynamic* compression-progress claim by itself. | **Strong yes.** Each promoted embedder is a measurable increase in $\mathrm{CP}_\Phi$ — that's literally the gate. The substrate grows in the direction of maximal compression progress, gated by reality. This is the Schmidhuberian Gödel-machine specialised to JEPA. |
| **Operator (epistemic symmetry + measurable objectives)** | Intent/north-star work #332/#333 is retired by #358. Active work is limited to observable code-reality signals, operator-approved objectives, and held-out improvement criteria. | **Strong yes at the substrate-design level.** Construct 4 (Bidirectional Epistemology) applies to embedders as measurable residual-signal detectors: the system specifies the means (existing N embedders + the absence-shape), the LLM proposes the path (candidate embedder architecture), and the verifier checks the destination (held-out improvement). |

---

## §6 — Implementation order and scope

**v1 (this issue, ~6–12 weeks):**

1. CFs and dynamic-registration scaffolding (Piece 5).
2. Absence-detection collection into ranked proposal queue (Piece 1).
3. Falsification gate + per-cell-promotion-gate reuse (Piece 4).
4. Sub-case (a) algorithmic-embedder synthesis loop (Piece 2 + Piece 3 source (ii) or (iii)).
5. Sub-case (b) learned-head synthesis loop (Piece 2 + Piece 3 source (i) for the E11-replacement first instance).
6. First concrete instance: TransE-on-corpus-entity-graph promoted as the E11 replacement.
7. Freeze + audit machinery (Piece 6).
8. Operator-in-the-loop gates (§4).

**v2 (TASK-EK-018-v2, future issue, ~6+ months out):**

1. Sub-case (c) fully neural embedder from scratch via NAS.
2. Cross-library candidate proposals (per-library proposals + cross-library generalisation tests).
3. Self-rewriting of the absence-detection priors based on which candidates have historically improved which cells.
4. Status-reallocation extension: operator-contribution surface (TASK-EK-014 / #336) tracks per-operator embedder-proposal review quality.

---

## §7 — Open questions / risks (honest)

1. **Compute budget.** Each candidate costs training time. v1 must include a candidate-priority queue (curiosity score from TASK-EK-006 / #327 + foundationality variance from TASK-EK-016 / #339 + absence-signal magnitude). Without it, the system can spawn an unbounded number of candidates.
2. **Architecture-space search is high-dimensional.** Sub-case (a) has the AlphaEvolve scaffold; sub-case (b) is bounded by the choice of contrastive objective. The search is tractable only because we are constraining ourselves to (a) and (b) for v1.
3. **The fail-closed gate must dominate.** If a bug in the falsification gate auto-promotes a regressed embedder, the panel is corrupted. The per-cell-promotion-gate (existing TASK-G-016 / #126) is the right model: regression on *any* cell by > `δ_cell` blocks promotion, full stop.
4. **Adversarial-embedder-injection threat.** A malicious actor could engineer an absence-signal pattern that, when fed to the system, proposes a candidate embedder serving an attacker's purpose. Mitigation: the TASK-EK-007 / #328 held-out-failure-mode falsifier should be extended to include adversarial-absence-signal injection; the per-cell-promotion-gate prevents subtle attacks from being auto-promoted; the operator-in-the-loop discipline (§4) catches first-of-its-kind embedders. Filed as a §6 of `docs/futurebuild/22_ADVERSARIAL_ROBUSTNESS.md` extension (this doc updates that doc).
5. **The JEPA $\Delta\Xi.\text{target\_collapse} = 0$ invariant.** For sub-case (a) it is preserved by construction (deterministic). For sub-case (b) the projection head is trained with the backbone frozen and is itself frozen on promotion; gradient never flows into the frozen-instrument target through the promoted embedder during downstream predictor training. This must be enforced by a Fisher-snapshot check at every training step, identical to the existing `MEJEPA_INSTRUMENT_GRADIENT_LEAK` guard.
6. **Cardinality explosion.** If the system promotes one embedder per week, the pairwise-MI matrix grows by O(N) entries per promotion. After a year the matrix is $\binom{65}{2} = 2080$ pairs. Storage scales; compute scales linearly per query. The mincut on the embedder graph remains tractable. The TCT cell centroids grow proportionally. None blocking but worth monitoring.
7. **The Bible-foundationality framing from §10.2(XI)–(XV).** Some self-promoted embedders will themselves become foundational (heavily depended on by many downstream predictor heads). TASK-EK-016 / #339 chunk-foundationality should be extended to *embedder*-foundationality once TASK-EK-018 ships, with the same PageRank-on-dependency-graph framing. Filed as a §7 extension of TASK-EK-016 / #339.

---

## §8 — Cross-references

- TASK-EK-018 / [#341](https://github.com/ChrisRoyse/contextgraph/issues/341) — the canonical issue.
- EPIC #321 — the parent.
- `docs/explanatory_knowledge_and_mejepaupdated.md §10.2(II), (II-bis), (II-ter), (XI)–(XV)` — the source analyses that this doc collapses.
- `docs/futurebuild/05_EMBEDDER_SIGNAL_MAP.md` — the 21-slot static registry; updated by this work to support dynamic extension.
- `docs/futurebuild/03_ARCHITECTURE_BLUEPRINT.md` — the architecture blueprint; updated by this work to show dynamic embedder-panel growth.
- `docs/futurebuild/04_DDA_TCT_CODE_ENRICHMENT.md` — the DDA framework; updated by this work because DDA cardinality is now time-varying.
- `docs/futurebuild/09_SELF_HEALING_AUTONOMY.md` — the self-healing framework; updated by this work to add embedder self-design as a new healing class.
- `docs/futurebuild/10_LEARNING_FORMULA_MEJEPA_TRUTH.md` — the UTML formula; updated by this work because the formula must accommodate dynamic N.
- `docs/futurebuild/12_IMPLEMENTATION_PHASES.md` — the phase plan; updated by this work to add Phase I (TASK-EK-018) as the apex.
- `docs/futurebuild/13_TASK_INDEX.md` — the task index; updated to include TASK-EK-018.
- `docs/futurebuild/17_BLIND_SPOTS_AND_MISSING_CAPABILITIES.md` — updated to note that the universal-explainer gap is now closed by this work.
- `docs/futurebuild/22_ADVERSARIAL_ROBUSTNESS.md` — updated with the adversarial-embedder-injection threat model.
- `docs/futurebuild/27_PHASE_G_ROADMAP.md` — updated to include TASK-EK-018 in the post-Phase-G queue.
- `docs/futurebuild/28_LECUN_JEPA_ALIGNMENT.md` — updated to note that pillars 1 (frozen target panel) and 9 (System 1 vs System 2) are now self-extending under TASK-EK-018.
- `docs/futurebuild/29_REWARD_SIGNAL_COMPLETENESS.md` — updated to note that newly promoted embedders create new tier-2 reward-signal sources (per-embedder cosine to centroid, new pairwise-MI rows).

---

**TL;DR.** The apex deliverable of EPIC #321 is one issue (#341 / TASK-EK-018) that collapses fourteen prior §10.2 additions into one underlying primitive: *the embedder panel grows under the system's own operation, gated by reality-anchored falsification, preserving the JEPA collapse-immunity invariant by freezing promoted candidates*. v1 ships sub-cases (a) algorithmic and (b) small-learned-head with TransE-on-corpus-entity-graph as the first concrete instance (the system's own E11 replacement). v2 adds neural-architecture-search. Operator-in-the-loop discipline preserved via the existing Phase E catastrophic-class promotion gate plus a first-promotion cold-start gate. The Deutsch / Schmidhuber / operator-framework verdicts all converge to strong-yes once this ships.
