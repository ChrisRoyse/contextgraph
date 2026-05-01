# 01 — Motivation: Why Combine ME-JEPA with Meaning Compression

This document states why the combination is worth doing. It is a planning
artefact; the *measured* arguments belong in the research manuscript
(`docs/5090jepa/researchpaper/paper.tex` after the v1.1 update planned in
`16_paper_v1_1_outline.md`).

## 1. What each system already brings

**ME-JEPA** (this repository's primary contribution) brings:

- A working domain-pack-driven JEPA runtime: 29 CLI subcommands compose
  into one immutable bundle per run.
- 25 typed RocksDB column families plus an append-only audit log; every
  mutating operation writes through one of them.
- A separate-code-path verifier (`inspect-counts`, `inspect-cf`,
  `get-*`) that re-reads every persisted output through code that cannot
  mutate the source of truth.
- A deterministic public-source benchmark (`career_taxonomy.v1` walking
  O*NET 30.2) with cryptographic provenance per record.
- Deterministically built Docker + Singularity (SIF) reproducibility
  artefacts.
- A novel *training-data class argument* (the v1 paper §7): bundles are
  audited execution traces with cryptographic provenance, distinct from
  web text, pure model-generated synthetic data, and rejection-sampled
  chain-of-thought.

**Meaning compression / DDA / TCT** (`docs/PAPER.md`) brings:

- A counting identity: $n$ inputs through $N$ frozen embedders yield up
  to $n \cdot (N + \binom{N}{2})$ structured supervisory signals, with
  the pairwise $\binom{N}{2}$ term carrying cross-instrument structure
  the per-instrument readings miss.
- A formal *meaning-compression* ratio (signals per raw input) proposed
  as a fourth taxonomic entry alongside bit-, weight-, and activation-
  compression.
- A constellation object $C_P$ — a frozen unit-norm centroid set per
  modality — that admits per-output runtime verifiability.
- A three-role composition rule: the same frozen panel serves as
  retriever, labeller, and runtime guard simultaneously, with no
  parameter updates between roles.
- A pool-relative novelty argument vs.\ scalar-reward alignment.

## 2. The architectural fit is unusually clean

ME-JEPA's "instruments" *already are* a frozen embedder panel.
Per-domain TOML domain packs already declare them with dimension
contracts, allowed input event kinds, and validation status (see
`backupdocs/dynamicjepa/05_instrument_registry.md`). The latent panel
materialised per event *already is* a DDA decomposition of that event,
with all the provenance properties PAPER.md asks for.

What ME-JEPA does *not* yet do, but PAPER.md prescribes:

1. Persist the $\binom{N}{2}$ **pairwise instrument features** as
   first-class records. They are computed transiently inside
   `compute_embedder_scores_sync` for ranking purposes; nothing reads
   them back from durable storage today.
2. Maintain a frozen **constellation set** $C_P$ per (domain, optionally
   subject, modality), built once at registration time, hashed into
   provenance, and used as a runtime target.
3. Make the **runtime guard predicate** $G_\tau$ explicit: the current
   `dj_guard_decisions` row records a utility verdict but not a per-
   modality cosine-to-centroid score that can be audited post-hoc.
4. Report a **meaning-compression ratio** (signals per raw input) as a
   bundle-level metric.
5. Run a **pairwise mutual-information audit** to test the approximate-
   independence axiom of PAPER.md and to convert the structural upper
   bound $N + \binom{N}{2}$ into a measured effective signal count
   $N_{\text{eff}}$.

## 3. Concrete supervision-density gain

A career\_taxonomy event today produces about 25 deterministic signals
end-to-end (10 per-instrument readings + 10 next-state readings + the
action vector + a handful of audit / planning rows).

Under the combined runtime the same event produces about 175 signals
(see `03_signal_yield_math.md` for the formal counting):

```
 10 per-instrument readings (Slice 1, already there)
 10 next-state readings (Slice 1, already there)
 45 pairwise (instrument_j, instrument_k) at t            ← new
 45 pairwise at t+1                                       ← new
  5 candidate actions × 10 predicted-panel slots (Slice 2)
  5 cosine-to-centroid scores per candidate               ← new
  5 G_tau verdicts (Slice 3)
  1 surprise (conditional, Slice 2)
≈ 175 structured signals per event
```

That is roughly **a 7× density increase per raw event** with no new
data ingestion. The bulk of the additional bytes (the pairwise
features) live in Slice 1 — deterministic functions of the event and
the frozen panel — so the model-collapse hazard ratio of the bundle
*improves*, not worsens, as Slice 2's (model-output) share shrinks
relative to the bundle as a whole.

For the smaller domains, the same math gives:

| Domain | $N$ panel slots | Per-event today | Per-event combined | Multiplier |
| --- | ---: | ---: | ---: | ---: |
| counter\_world | 2 | $\sim$6 | $\sim$15 | $2.5\times$ |
| gridworld\_5x5 | 3 | $\sim$8 | $\sim$22 | $2.8\times$ |
| career\_taxonomy A1 | 10 | $\sim$25 | $\sim$175 | $7.0\times$ |

The career\_taxonomy domain is where the multiplier is largest precisely
because $\binom{N}{2}$ dominates at $N{=}10$ but not at $N{=}2$.

## 4. The three reasons this is worth doing

**Reason 1. The training-data class argument gets sharper.**

The v1 paper §7 already argues that ME-JEPA bundles are a new training-
data class with low recursion-replacement collapse hazard. With the
combined runtime, the argument has *two new measurable handles*:
(a) the MC ratio is a bundle-level number that can be reported, plotted
across domains, and tracked for drift; (b) the pairwise MI audit
converts the "approximately independent" hedge into a measured
$N_{\text{eff}}$. Both of these directly answer questions a reviewer
would raise about the v1 paper.

**Reason 2. The constellation guard fills a real slot.**

ME-JEPA's existing guard is utility-thresholded — a heuristic per-domain
verdict. With $G_\tau$ as a deterministic geometric test against a
frozen $C_P$, the guard becomes an *audit-ready predicate* downstream
agents can be trained to emit, log, and verify post-hoc. PAPER.md §5.4
makes the case that this property is absent from the entire scalar-
reward alignment tradition; the combined runtime carries it natively.

**Reason 3. The pairwise MI audit is the cleanest open question both
papers leave hanging.**

PAPER.md §3.1's Axiom 2 ("approximate independence across $\Phi$") is
hedged. ME-JEPA's instrument panel hasn't measured it either. Running
KSG / MINE on the combined system — on the substrate that already has
the FSV machinery, the deterministic public source, and the per-domain
TOML manifests — answers a question both papers leave open, in a place
where the answer is reproducible.

## 5. Honest limits of this combination

- **It is composition, not invention.** Both source contributions
  remain valid in their original framing; this upgrade reinforces
  rather than replaces them.
- **The 7× number is an upper bound** until the pairwise MI audit
  measures redundancy. PAPER.md §8.L4 carries this hedge; this plan
  inherits it.
- **Storage cost grows roughly $N^2 / 2$.** A 1{,}000-event
  career\_taxonomy bundle with $N{=}10$ stores 45{,}000 pairwise rows
  on top of 10{,}000 instrument readings. The quantitative impact is
  reviewed in `07_storage_schema_changes.md`; the headline number is
  $\sim$30 % increase in RocksDB total size on disk for the typical
  paper-small bundle.
- **The constellation construction discipline (LOO stability,
  reference-set manifest, weight hash)** is unfamiliar to the existing
  ME-JEPA codebase and adds a non-trivial new persistence layer. The
  detailed protocol is in `04_constellation_design.md`.
- **Threshold calibration is per-panel and per-domain.** Per
  PAPER.md §8.L5, $\tau_m$ values do not transport across encoders or
  recording conditions. This upgrade does not propose universal
  thresholds; it proposes a calibration *protocol* that records the
  per-domain $\tau_m$ as a first-class persisted artefact.

## 6. What changes vs.\ what stays

**Stays unchanged:**
- All 25 existing CFs, all existing per-CF row counts for the existing
  paper-small numbers (the v1 paper's `paper_tables_reference/` CSVs
  remain valid against pre-upgrade bundles).
- The 29 existing CLI subcommands; existing tests; existing FSV
  harnesses FSV-1 through FSV-8.
- The deterministic Docker / SIF reproducibility chain (Phase 4 of
  this plan adds new outputs, does not change existing ones).
- The "no recruiting / hiring / fairness validity" scope discipline.

**Adds:**
- 3 new CFs: `dj_pairwise_readings`, `dj_constellations`,
  `dj_threshold_calibrations`.
- 1 augmented CF: `dj_guard_decisions` gains
  `cosine_to_centroid_per_modality` and `predicate_verdict`.
- 1 augmented CF: `dj_audit_log` gains a `signal_yield` field.
- 6 new CLI subcommands (`build-constellation`, `list-constellations`,
  `get-constellation`, `audit-pairwise-mi`, `compute-mc-ratio`,
  `calibrate-threshold`).
- 6 new MCP tools mirroring those subcommands.
- 4 new FSV harnesses (FSV-9 through FSV-12).
- 4 new paper tables (`table_pairwise_mi_summary.csv`,
  `table_mc_ratio.csv`, `table_constellation_guard.csv`,
  `table_threshold_calibration.csv`) and 3 new plots.
- A schema-version bump on the bundle from `v1` to `v2`.

## 7. What stays the same about the ME-JEPA paper claims

Per the v1 paper's existing scope discipline:

- The combined runtime is still a *systems contribution* on a
  *public-taxonomy benchmark*, not a representational-quality JEPA
  paper.
- The benchmark remains O*NET 30.2 walked from
  `15-1252.00`; ESCO replication remains future work.
- Models remain tiny (480 / 16{,}960 / 15{,}344 / under 25{,}000
  trainable parameters across the four domain configurations).
- Single-host RTX 5090 reproducibility remains the constraint.
- No fairness, hiring, real-person, or production-deployment claims.

The combined runtime *strengthens* the existing claims; it does not
invent new application claims.
