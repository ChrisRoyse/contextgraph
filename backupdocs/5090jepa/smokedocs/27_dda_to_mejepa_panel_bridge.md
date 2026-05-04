# 27 - DDA → ME-JEPA Panel Bridge

## Goal

Connect Context Graph's 13-embedder DDA substrate to ME-JEPA's
multi-instrument latent panel so DDA signals become first-class
**instrument readings** in a domain pack. After this work, training a
ME-JEPA predictor on a text or video corpus consumes up to 91
structured signals per input as panel components, all FSV-verified.

This is the integration that makes "meaning compression" operationally
real for the ME-JEPA training loop.

## Why This Bridge Is Load-Bearing

ME-JEPA today consumes typed events and produces panels via
domain-pack-defined adapters. The instruments are scalar/categorical/
vector fields drawn from the event itself. The DDA substrate already
projects every input through 13 frozen embedders into Context Graph's
RocksDB; today, those projections are used for retrieval, not training.

The bridge:

- exposes DDA per-embedder vectors and pairwise interaction features as
  ME-JEPA instruments;
- carries the embedder/projection hash through the FSV chain;
- lets a domain pack declare "use Context Graph fingerprint X as
  instrument I_m" without bespoke ingest code.

After the bridge, a 1K text corpus that was 1K events becomes
**91K supervisory signals** in the panel.

## Architecture

### New Adapter: `dda_signals` adapter

A new ME-JEPA adapter kind `dda_signals` reads from a Context Graph
RocksDB instance. Inputs:

```toml
[adapters.dda_career_panel]
kind = "dda_signals"
context_graph_db = "tmp/context_graph_paper_corpus.db"
fingerprint_index_cf = "fingerprint_index"
embedders = ["e1", "e7", "e10", "e11", "e12"]
pairwise_interactions = [
  ["e1", "e10"],
  ["e7", "e11"],
]
```

Per event, the adapter:

1. Looks up the fingerprint UUID in `fingerprint_index_cf` by the
   event's content hash.
2. Reads each requested per-embedder vector from
   `cf_e{m}_active_vectors`.
3. Computes the requested pairwise interaction features
   (cosine + a frozen pairwise adapter, per PAPER.md §3.1) on the fly.
4. Emits one `InstrumentReading` row per (event, embedder) and one per
   (event, embedder-pair).
5. Records the embedder versions and the pairwise adapter hash in the
   reading metadata.

Failure modes (typed, fail-closed):

- Fingerprint not present → `DDA_FINGERPRINT_MISSING`.
- Embedder version mismatch with what the domain pack declared →
  `DDA_EMBEDDER_VERSION_DRIFT`.
- Pairwise adapter hash mismatch → `DDA_PAIRWISE_DRIFT`.
- Vector zero-norm (e.g. legacy E2 placeholder) → `DDA_DEGENERATE_VECTOR`,
  not silently skipped.

### Panel Composition

Domain pack `career_taxonomy.v2.toml` extends v1:

```toml
[panel.career_taxonomy_dda_v2]
slots = [
  { name = "occupation_cluster", instrument = "occupation_one_hot", dim = 24 },
  { name = "skill_vector",       instrument = "skill_dense", dim = 16 },
  { name = "dda_e1_text",        instrument = "dda_career_panel", embedder = "e1",  dim = 1024 },
  { name = "dda_e7_code",        instrument = "dda_career_panel", embedder = "e7",  dim = 1536 },
  { name = "dda_e10_para",       instrument = "dda_career_panel", embedder = "e10", dim = 768 },
  { name = "dda_e11_entity",     instrument = "dda_career_panel", embedder = "e11", dim = 768 },
  { name = "dda_e1_e10_pair",    instrument = "dda_career_panel", pair = ["e1","e10"], dim = 1 },
]
```

Total panel input dim grows from ~50 (v1 smoke) to ~4,140. Latent
predictor envelope (doc 17 ladder) absorbs this from `paper_med_5k` tier
upward.

### FSV Extension

Smokedoc 08 §Stage Sources Of Truth gains:

| Stage | Source Of Truth |
| --- | --- |
| DDA fingerprint resolution | Context Graph `fingerprint_index` row + recomputed content hash |
| DDA per-embedder reading | `dj_instrument_readings` row + `embedder_version_hash` field |
| DDA pairwise reading | `dj_instrument_readings` row + `pairwise_adapter_hash` field |

`dj_instrument_readings` schema gains optional fields:

```rust
pub struct DdaProvenance {
    pub embedder_id: String,        // "e1"
    pub embedder_version_hash: [u8; 32],
    pub fingerprint_id: Uuid,
    pub source_corpus_hash: [u8; 32], // hash of corpus manifest
    pub pairwise_adapter_hash: Option<[u8; 32]>,
}
```

Every reading carries this when the adapter kind is `dda_signals`.

## CLI Surface

```bash
target/debug/context-graph dynamicjepa register-domain \
  --db "$DB" \
  --file configs/dynamicjepa/domain_packs/career_taxonomy.v2.toml \
  --context-graph-db "$CG_DB" \
  --json
```

`--context-graph-db` is required when the pack uses `dda_signals`.
Registration validates the requested embedders exist in the Context
Graph DB and records their version hashes in `dj_domain_packs`.

```bash
target/debug/context-graph dynamicjepa materialize-panel \
  --db "$DB" \
  --domain career_taxonomy \
  --panel career_taxonomy_dda_v2 \
  --all-pending \
  --json
```

The materialize-panel command now resolves DDA fingerprints in batch
and writes one reading row per (event, embedder) and (event, pair).

## Provenance Math (For Reviewer)

Per the bundle, a reviewer can recompute, byte-for-byte:

1. Source corpus SHA-256 → recorded in `data_manifest.json`.
2. For event $i$: re-embed $x_i$ with the pinned model versions; compare
   hash to `embedder_version_hash` and vector to `dj_instrument_readings`.
3. For pair $(j, k)$: recompute $\rho_{jk}(\phi_j(x_i), \phi_k(x_i))$
   with the pinned `pairwise_adapter_hash`; compare to the persisted
   reading.
4. ME-JEPA panel is the deterministic concatenation of (1)-(3); compare
   to `dj_latent_panels`.

Any drift fails closed at exactly the layer it first appeared. Reviewers
do not need to "trust" any number in the paper; they recompute.

## Performance Budget

DDA panel materialization is I/O-bound, not compute-bound. Per-event
cost on a warm RocksDB:

- 13 vector lookups (~50 µs each) = ~0.65 ms/event;
- 78 pairwise computations (cosine ~ 5 µs each) = ~0.4 ms/event;
- 1 RocksDB write per reading × 91 = ~0.5 ms/event with batched writes.

At 1K events: ~1.5 s panel materialization + writes.
At 50K events (paper_full per doc 17): ~75 s. Comfortably within
doc-17's 10-min panel-materialize budget at the 50K tier.

## Edge Cases (Fail-Closed)

- DDA adapter declared but `--context-graph-db` not supplied at register
  time: `VALIDATION` error; no domain registered.
- Domain pack declares an embedder ID Context Graph does not know:
  `DDA_EMBEDDER_UNKNOWN` error.
- Mid-run, Context Graph DB schema migrates and embedder version hash
  changes: `DDA_EMBEDDER_VERSION_DRIFT`; the run aborts and bundle
  records the drift.
- Pairwise adapter is updated after a domain pack is registered:
  `DDA_PAIRWISE_DRIFT`; new domain pack version required.
- Asymmetric embedders (E5/E8/E10) requested without specifying which
  vector (cause/effect, source/target, doc/query): `VALIDATION`.

## Coordination With Other Docs

- Doc 22 (collapse diagnostics) operates on the panel after this bridge
  fills it; expect higher-dim latent panels to need larger latent dims
  (already in doc-17 ladder).
- Doc 16 (decoder heads) operates on the post-training latent; no
  change required, but top-k metrics will improve from the richer
  panel.
- Doc 30 (mutual-information audit) directly measures the tightness of
  the 91-signal bound using this bridge as the data source.
- Doc 33 (EU AI Act compliance) leans on `DdaProvenance` rows as the
  Article 10 data-governance evidence.

## Acceptance

This work is done when:

- the `dda_signals` adapter exists in `crates/context-graph-dynamicjepa`;
- `career_taxonomy.v2.toml` exists and registers cleanly against a
  populated Context Graph DB;
- a `paper_med_5k` bundle trained with the v2 panel passes FSV;
- `dj_instrument_readings` rows carry `DdaProvenance` and a reviewer can
  byte-recompute one row from raw corpus → panel.
