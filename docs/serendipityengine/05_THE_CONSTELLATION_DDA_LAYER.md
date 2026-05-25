# The Constellation Layer — DDA Applied to Personhood

This document defines the substrate the matching engine runs on. It is a direct reuse of the **Derived Data Abundance (DDA)** and **Teleological Constellation Training (TCT)** machinery already shipped in `docs/PAPER.md` and ContextGraph, but with a new domain: a person, not a piece of code or a video clip.

## 1. The thesis

A human being is **far too rich** to be captured by any single representation (a profile, a bio, a set of swiped interests, an embedding from a single LLM). Lunchclub failed in part because its representation of a person was a flat list of LinkedIn-style traits. Tinder's representation is essentially a photo + a sentence. Both lose 99%+ of the signal that determines whether two people will actually have a good encounter.

The DDA insight (Royse 2026, paper §3): a single piece of data (a person) projected through **N frozen independently-trained embedders** yields N independent labelled representations + C(N,2) pairwise cross-correlations + (optionally) one foundationality scalar — for N=13 that's up to 92 signals per person. This is what the matching engine consumes, not a flat profile.

## 2. The Citizen Constellation

Each citizen's **Constellation** is a frozen-at-onboarding, slowly-updating set of per-modality centroids — exactly the TCT structure of Royse 2026 §5, with modalities reinterpreted for personhood:

| # | Modality | Embedder / source | What it captures |
|---|---|---|---|
| **M1** | Semantic-self | E1 (e5-large-v2) over the 5 intake-interview answers + any subsequent free-text reflections | What the citizen cares about in plain language |
| **M2** | Causal-direction | E5 (asymmetric, nomic fine-tuned) over the intake "what would you like more of" answer | Their personal cause→effect chains; what they want to produce, not just consume |
| **M3** | Code-structural | E7 (Qodo-Embed) — for citizens whose work or hobbies involve code/structured systems | Structural-thinking fingerprint (engineers, lawyers, mathematicians cluster here) |
| **M4** | Narrative-arc | E11 (KEPLER) over the "describe a moment you felt most alive" answer | The kind of story they tell about themselves |
| **M5** | Lexical-vocabulary | E6 (SPLADE sparse) over all their stated text | What words they reach for — vocabulary as identity |
| **M6** | Hyperdimensional-typo-robust | E9 (HDC) over all text | Captures cadence, rhythm, idiomatic register at the binary level — robust to literacy / dialect / second-language artifacts (egalitarian critical) |
| **M7** | Temporal-rhythm | E2/E3/E4 (sinusoidal/Fourier/positional) over their stated availability + event-attendance history | When in the week/day they're alive; circadian compatibility for evening dinners vs morning runs |
| **M8** | Graph-position | E8 (asymmetric graph) over their attendance graph (which events they've been to, who else was there) | Their place in the social network — *not* a follower count, but a structural-hole indicator |
| **M9** | Topic-portfolio | The 13-embedder topic-cluster aggregation already shipped in ContextGraph (HDBSCAN over E1+E7+E11) | What topic-clusters they live in conceptually |
| **M10** | Energy / extraversion-axis (somatic) | Custom 12-D prosody-style features from the intake video call audio (with explicit consent), aggregated and quantized | Voice-tempo, pause-rate, energy-band — analogous to ClipCannon's prosody embedder; helps avoid matching extreme-introvert + extreme-extrovert in a 2-person dinner |
| **M11** | Visual-aesthetic (optional, opt-in) | SigLIP over the 5 images the citizen optionally curates (not a profile photo — five images of things they love: a painting, a meal, a place, a book cover, a person who is not them) | The aesthetic field they swim in |
| **M12** | Ideological / cross-perspective | A bridging-rerank embedding (per Ovadya / Belfer 2022) — captures **how** they reason, not what they conclude | Used for cross-ideological matching (Tier-3 metric #16) |
| **M13** | Constraint vector | Hard constraints — accessibility, dietary, language, schedule, no-go list — stored as structured fields, not embeddings | The hard floor the algorithm cannot violate |

Total dimensionality across M1–M11 ≈ 9,600 dimensions per citizen, plus 78 pairwise cross-correlations, plus 12 hard constraints in M13 — a **dense, multi-perspective fingerprint** that no single embedder could capture.

## 3. Frozen vs. live components

Following TCT (Royse 2026 §5.2):

- **All embedders are frozen.** Per Axiom 1 of the paper, this is what makes the system structurally outside Shumailov-form collapse.
- **The citizen's intake answers are frozen.** Subsequent updates are *additions*, not replacements. The intake answers are the "constellation core" — the citizen's Phase-1 reference set.
- **The centroids are computed once at end-of-onboarding (Phase 1) and re-computed at most quarterly.** Centroids are **not** updated per event — that would create a moving target the matching algorithm cannot stably aim at.
- **Per-event reflection prompts feed an *online head*** that nudges per-citizen weight profiles (which modalities matter more for them) but does not touch the frozen Constellation.

This is exactly the trainer architecture of ME-JEPA-Code, mapped onto a human. The "online head" is the equivalent of the mistake-loop consumer in ContextGraph #406 / #523.

## 4. The Constellation Guard (Phase 3)

At match time, the candidate match graph is filtered through a runtime guard analogous to the `G_τ` predicate in Royse 2026 §5.4:

```
G_τ(candidate_match) = 1[∀m ∈ M_required : cos(φ_m(match_signature), c_m^citizen) ≥ τ_m]
```

For matching, "match_signature" is the candidate match's panel-vector (the projected fingerprint of the proposed gathering), and `c_m^citizen` is the citizen's centroid in modality m. Thresholds τ_m are calibrated to a same-quality-band floor on a disjoint citizen calibration set, never tuned on the evaluation set (per the TCT discipline of §5.4).

This guard ensures that no matter how the matching algorithm scores something, a proposed match that catastrophically misses a slot — wrong day-of-week (M7 fail), wrong energy band (M10 fail), violation of a hard constraint (M13 fail) — is rejected at the gate, regardless of how good it looks in aggregate.

## 5. Anti-flattening (slot-identity is sacred)

Per CLAUDE.md §6.2 (ContextGraph doctrine) and Invariant **I10** of `03_NON_NEGOTIABLE_INVARIANTS.md`: **the Constellation must never be flattened into a single concatenated 9,600-D vector for similarity, distance, or any predictor input.** Per-slot cosine, slot-preserving projections, scalar pairwise relationships — yes. Single-blob comparison — no, ever.

This is enforced architecturally by the matching service: the Constellation type is `Vec<(ModalitySlot, Embedding)>` plus pairwise feature accessors. There is no `.flatten()` method exposed. A PR that adds one is rejected at review.

## 6. Privacy and provenance

Following ContextGraph's audit-log architecture (`agents/audit-log-architect`, `entity-provenance`):

- Every embedder forward-pass writes a provenance row to `CF_CITIZEN_PROVENANCE` recording: embedder model SHA, source text/audio hash, timestamp, operator (always "self" for citizen-initiated data; "welcoming citizen X" for the intake interview), confidence, and the policy version under which it was collected.
- The full Constellation is stored encrypted at rest with a per-citizen key derived from their proof-of-personhood credential — nobody but the citizen and the matching service (which holds keys ephemerally in memory only for the duration of a match computation) can decrypt.
- A citizen can pull their full provenance trail at any time via `mcp__cgreality__citizen_provenance_export` (analogous to `provenance-mcp-tools`).
- Right-to-be-forgotten purges the entire Constellation in under 24 hours, with a signed deletion receipt issued back to the citizen.

## 7. Why not just use one LLM embedding

Three reasons, all empirically validated in `docs/PAPER.md` Case 1 (Shakespeare LoRA) and Case 3 (voice cloning):

1. **Single-embedder representations miss structural defects.** The Shakespeare LoRA's "prompt-echo bug" was invisible to E1 semantic similarity; only E4 (position-ordering) + E12 (token-MaxSim) caught it. The analogous failure in matching: two people whose semantic-bio cosine is 0.95 but whose narrative-arc cosine is 0.05 will have a terrible dinner. Multi-embedder catches this.

2. **Single-embedder is gameable.** If everyone knew the algorithm ran on (say) Sentence-BERT, citizens would optimize their text for it. With 13 independent embedders + 78 cross-correlations + bridging rerank, optimizing for the algorithm requires optimizing for an infeasibly large objective.

3. **Single-embedder is fragile to model drift.** When OpenAI updates `text-embedding-3-large` in 18 months, a single-embedder system silently degrades. The multi-embedder panel degrades gracefully — drop one slot, the others carry the weight, and the inter-embedder distinctness gate (per ContextGraph #709) flags the regression.

## 8. The connection to ContextGraph and ClipCannon

- ContextGraph already implements the 13-embedder TeleologicalFingerprint storage layer (51 CFs, 75 MCP tools). The Citizen Constellation reuses this directly — Person becomes a new entity type alongside Memory.
- ClipCannon's 7-modality 4,044-D constellation provides the prior art for M10–M11 (prosody + visual). Reused directly with consent-gated audio/visual ingest.
- The auto-labeler (#413), entity-provenance (#414, #418), and skill substrate already in ContextGraph map naturally to Citizen labeling and interest-clustering.

This is one stack, not three.

## 9. Cross-references

| Topic | Doc |
|---|---|
| Matching algorithm that consumes the Constellation | `07_THE_SERENDIPITY_ENGINE_ALGORITHM.md` |
| Privacy and proof of personhood | `08_ANTI_GAMING_AND_STATUS_FORBIDDANCE.md` |
| Why frozen + Slot identity is sacred | `03_NON_NEGOTIABLE_INVARIANTS.md` I10 |
| Original DDA / TCT theory | `docs/PAPER.md` §3, §5 |
| Tech architecture & reuse from ContextGraph | `15_TECHNICAL_ARCHITECTURE.md` |

---

**Last updated:** 2026-05-24.
