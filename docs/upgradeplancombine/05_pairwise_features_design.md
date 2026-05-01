# 05 — Pairwise Features Design ($\binom{N}{2}$ Cross-Instrument Features)

This document specifies the new `dj_pairwise_readings` column family,
the kinds of pairwise features computed, the computation strategy, and
the interaction with the existing `dj_instrument_readings` and
`dj_latent_panels` records.

## 1. What a pairwise feature is

For each event $e_i$ and each instrument pair $(j, k)$ with
$1 \le j < k \le N_d$, a pairwise feature
$\rho_{j,k}(\phi_j(e_i), \phi_k(e_i)) \in \mathbb{R}^{D_{j,k}}$ is a
deterministic function of the two per-instrument readings on the same
event. The function family is fixed per pair and persisted in the
domain pack so both compute and read sides agree.

Following `backupdocs/dynamicjepa/06_latent_panel_and_binding.md` —
which already lists the pairwise-feature kinds as design intent — and
PAPER.md §3.4, the upgrade ships **seven** pairwise feature kinds:

| Pair-kind | Symbol | Output dim $D_{j,k}$ | Definition |
| --- | --- | --- | --- |
| Cosine agreement | $\rho^{\cos}_{j,k}$ | 1 | $\cos(\phi_j(e_i), \phi_k(e_i))$ when both have $\geq 1$ vector slot. |
| Rank disagreement | $\rho^{\Delta r}_{j,k}$ | 1 | mean Spearman rank-distance of two top-$K$ retrieval lists from $\phi_j$ and $\phi_k$ on a fixed reference window. |
| Modality contradiction | $\rho^{\text{mod}}_{j,k}$ | 1 | indicator: $\rho^{\cos}_{j,k} < 0$ XOR a domain-pack-declared "should-agree" flag. |
| Sparse/dense mismatch | $\rho^{\text{s/d}}_{j,k}$ | 1 | when $j$ is sparse (e.g. SPLADE-like) and $k$ dense, the L1 mass-agreement between sparse weight vector and dense projection norm; 0 otherwise. |
| Temporal surprise | $\rho^{\text{surp}}_{j,k}$ | 1 | when one of $j, k$ is a temporal embedder (E2/E3/E4 in Context Graph terminology), the gap between its predicted next-position vector and the other instrument's observed next-step vector. |
| Causal direction disagreement | $\rho^{\text{caus}}_{j,k}$ | 1 | when one of $j, k$ is asymmetric (E5-style cause/effect), the boolean mismatch between cause-direction prediction and effect-direction prediction. |
| Safety proximity | $\rho^{\text{safe}}_{j,k}$ | 1 | when one of $j, k$ is a safety-tagged embedder, the cosine of the predicted-output to the nearest declared anti-pole vector. |

Most pairs in a typical career-domain panel produce only the
**cosine agreement** feature (the canonical, always-defined pair-kind);
the other six are emitted only when the two instruments declare matching
metadata. The pair record always includes the cosine and a bitset
indicating which other pair-kinds were emitted.

## 2. The `dj_pairwise_readings` row schema

Following the workspace persistence convention (V1 lesson: bincode +
`skip_serializing_if` is broken; use JSON):

```rust
pub struct PairwiseReading {
    pub pairwise_id: PairwiseReadingId,           // uuid
    pub event_id: EventId,
    pub instrument_j: InstrumentId,
    pub instrument_k: InstrumentId,                // j < k strict
    pub instrument_j_artifact_hash: [u8; 32],
    pub instrument_k_artifact_hash: [u8; 32],
    pub kinds_emitted: PairKindBitset,             // u8 covers the 7 kinds
    pub cosine_agreement: f32,                     // always present
    pub rank_disagreement: Option<f32>,
    pub modality_contradiction: Option<bool>,
    pub sparse_dense_mismatch: Option<f32>,
    pub temporal_surprise: Option<f32>,
    pub causal_direction_disagreement: Option<bool>,
    pub safety_proximity: Option<f32>,
    pub created_at_unix_ms: u64,
    pub validation_status: ReadingStatus,
}
```

`PairKindBitset` is a single byte; bit $b$ set iff pair-kind $b$ is
emitted on this row.

**Storage estimate.** A pair row serialised with the canonical fields
present plus four optional `f32`s averages 220 bytes (JSON + RocksDB
key overhead). For career\_taxonomy at 1{,}000 events, 45{,}000 rows
$\times$ 220 bytes $\approx$ 10 MB raw. After RocksDB Lz4 compression
(§7), 4–5 MB on disk. The full bundle's `dbs/...career_taxonomy_rocksdb`
grows from $\sim$13 MB to $\sim$18 MB, a $\sim$30 % increase.

## 3. Computation strategy

**Compute-on-write.** Pairwise features are computed at panel-
materialisation time (not lazily). Rationale:

- The instrument readings are already fully resident at that stage.
- Lazy computation makes FSV non-deterministic and complicates the
  audit log (each "lazy fetch" would need its own audit row).
- The cost is bounded: $O(N_d^2 \cdot d_{\max})$ per event for the
  cosine kind alone, where $d_{\max}$ is the largest output dimension.
  For career\_taxonomy A1 at $N{=}10$, $d_{\max} \le 64$: 45 cosine
  computations × 64 dims = 2{,}880 multiply-adds per event,
  trivially under 1 ms on the documented host.

The materialisation entrypoint
`materialize_panel(domain, event)` extends to:

```text
materialize_panel(domain, event):
  1. for each instrument m: phi_m(event) -> dj_instrument_readings  [existing]
  2. for each (j, k), 1 <= j < k <= N_d:                           [new]
       compute rho_{j,k} according to declared pair-kinds
       persist dj_pairwise_readings row with audit log entry
  3. compose latent panel referencing both instrument and pairwise
     reading uuids -> dj_latent_panels  [extended schema]
```

The transactional invariant: either the entire panel + pairwise set is
written, or nothing is written. Existing `put_panel_with_readings_batch`
is extended to `put_panel_with_readings_and_pairwise_batch`; the old
helper is removed (no fallback path, per CLAUDE.md).

## 4. Domain-pack pair-kind declaration

Domain packs declare which pair-kinds an instrument participates in via
a new TOML section. Example for `career_taxonomy.v2.toml`:

```toml
[[instrument]]
id = "occupation_cluster_onehot"
kind = "categorical_onehot"
output_dim = 4
pair_kinds = ["cosine", "modality_contradiction"]

[[instrument]]
id = "skill_vector"
kind = "vector"
output_dim = 4
pair_kinds = ["cosine", "rank_disagreement", "temporal_surprise"]

# ... etc for each of the 10 instruments
```

A pair $(j, k)$ emits pair-kind $\kappa$ iff *both* instruments declare
$\kappa$ in their `pair_kinds` list. The validator rejects domain
packs in which no instrument declares `cosine` (the always-on pair-kind
required to populate the pairwise row at all).

## 5. Per-event yield, formalised

For domain $d$ with panel size $N_d$:

- **Cosine agreement** is emitted for every pair: $\binom{N_d}{2}$ rows
  per event.
- The other six pair-kinds are emitted *additively* on the same row
  iff both instruments declare them, as flags in `kinds_emitted`.

Total `dj_pairwise_readings` row count per event = $\binom{N_d}{2}$,
identical for all events of the same domain.

For career\_taxonomy A1: $\binom{10}{2} = 45$ rows / event ⇒ 45{,}000
rows / 1K career bundle.

## 6. Interaction with the predictor

For v2.0 of this upgrade, the predictor consumes:
- The same per-instrument readings it already consumes (no schema
  change to predictor input).
- **Optionally**, the pairwise readings as concatenated feature
  vector input. Whether this is enabled is controlled by the
  per-domain TOML `[predictor.use_pairwise]` flag (default `true` for
  career\_taxonomy A4 only, matching the existing A4 panel's
  pairwise-feature column; default `false` for A0–A3, A5,
  counter\_world, gridworld\_5x5).

This preserves the existing baseline behaviour and isolates the
*runtime-side* contribution of pairwise features (their use as
training-data targets and as MC-ratio numerator) from the predictor's
input feature set.

The *full*-form predictor that consumes all pairwise features is a
deliberate v2.1 axis (`13_implementation_phases.md` §6). In v2.0 the
pairwise readings exist primarily for downstream training-data use
and for the MC-ratio measurement, not as predictor inputs.

## 7. Read-back protocol for FSV-9

`FSV-9: Pairwise readings integrity` runs at end-of-bundle:

1. For each event $e_i \in \text{dj\_normalized\_states}$ in the
   bundle, count its `dj_pairwise_readings` rows; assert
   $= \binom{N_d}{2}$.
2. For each pair row, recompute $\cos(\phi_j(e_i), \phi_k(e_i))$ from
   the persisted instrument readings; assert that the persisted
   cosine matches within $10^{-4}$ tolerance.
3. Verify both `instrument_j_artifact_hash` and
   `instrument_k_artifact_hash` match the current
   `dj_instrument_registry`.
4. Assert `kinds_emitted` bitset is consistent with the domain-pack
   declaration (every emitted pair-kind has a non-`None` value;
   every non-emitted pair-kind has `None`).

A failure on any of these writes a typed FSV row with `fsv_check =
"pairwise_readings_integrity"` and surfaces a non-zero exit code.

## 8. Generalisation: what about $N=2$?

For sanity domains where $N_d \le 3$, pairwise features still land,
but the row count is small ($\binom{2}{2} = 1$ for counter\_world;
$\binom{3}{2} = 3$ for gridworld\_5x5). FSV-9 still runs; the per-
event yield matches the formula in `03_signal_yield_math.md`. There is
no special case for small $N_d$; the implementation is uniform.

## 9. Edge cases (typed-error catalog)

The pairwise layer adds these typed errors to
`paper_tables/table_edge_cases.csv`:

| Edge | Trigger | Typed error code |
| --- | --- | --- |
| Pairwise count mismatch | event has $\ne \binom{N_d}{2}$ pair rows | `PAIRWISE_ROW_COUNT_MISMATCH` |
| Cosine recompute drift | persisted cosine differs from recomputed by $> 10^{-4}$ | `PAIRWISE_COSINE_DRIFT` |
| Pair-kind bitset mismatch | declared pair-kind not emitted, or vice versa | `PAIRWISE_KIND_BITSET_MISMATCH` |
| Asymmetric ordering | row stored with $j > k$ | `PAIRWISE_ASYMMETRIC_ORDERING` |
| Instrument hash drift | pair row's recorded instrument hash differs from registry | `PAIRWISE_INSTRUMENT_HASH_DRIFT` |
| Pair-kind without declaration | TOML `pair_kinds` does not include "cosine" | `PAIRWISE_COSINE_KIND_MISSING` |

Six new edges. Combined with the nine constellation edges from
`04_constellation_design.md` §9, the upgrade extends the per-bundle
edge-case row count from 11 to 26 (career\_taxonomy A1).

## 10. Compute budget

For a 1{,}000-event career bundle on the documented RTX 5090 host:

| Stage | Today | With pairwise (estimate) |
| --- | ---: | ---: |
| Panel materialisation wall time | $\sim 30$ s | $\sim 35$ s (+15 %) |
| RocksDB write throughput | adequate | adequate (LZ4 batch writes) |
| Peak memory | < 8 GiB | < 8 GiB (pairwise rows are tiny) |

The 15 % stage overhead is well within the 2-hour release-C budget;
no compute-side changes to the existing tier ladder are required.

## 11. What is *not* added in v2.0

To keep the changeset reviewable:

- **No higher-order features** ($N$-tuples for $N > 2$); only the
  $\binom{N_d}{2}$ pairwise term.
- **No cross-event pairwise features** (e.g., "cosine between event
  $i$'s instrument $j$ and event $i+1$'s instrument $j$"); only
  within-event.
- **No learned pairwise adapters** (PAPER.md §3.1 mentions
  "concatenation followed by a frozen adapter"); v2.0 ships only
  fixed mathematical pair-kinds.

Higher-order, cross-event, and adapter pair-kinds are deferred to a
v2.2 axis if measured value warrants the complexity.
