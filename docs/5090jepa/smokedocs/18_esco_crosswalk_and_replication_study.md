# 18 - ESCO Crosswalk And Replication Study

## Goal

Demonstrate that the career-taxonomy benchmark and ME-JEPA runtime are not
overfitted to a single occupational taxonomy. Add ESCO as a second taxonomy
source, run the same pipeline end-to-end, and report a small **crosswalk**
result mapping O*NET-trained representations to ESCO-derived labels (and vice
versa). This addresses smokedoc 11 §Open Question Q3 directly.

## Why ESCO

- ESCO is the European Skills, Competences, Qualifications and Occupations
  classification, maintained by the European Commission. It is the
  cross-national counterpart to O*NET.
- Public, downloadable, hashable.
- Three of the most-cited recent career-trajectory datasets (Karrierewege,
  JobHop, OpenResume) are ESCO-linked, not O*NET-linked. Adding ESCO opens the
  door to using any of them in a future tier without redesigning the benchmark.
- Runtime universality is one of the paper's three claims (smokedoc 01
  §Contributions); ESCO replication makes that claim real instead of asserted.

## Scope (For Paper Submission)

Two tiers, in order:

### Tier 1 — ESCO standalone replication

Re-run `paper_med_5k` (doc 17) against an ESCO-derived walk graph. Compare:

- prediction metrics on ESCO labels;
- top-k cluster accuracy against ESCO ISCO-08 four-digit groupings;
- ablation matrix A0-A5;
- collapse diagnostics.

The expected paper claim: **the same runtime, same model, same FSV protocol,
delivers comparable evidence on a second public taxonomy.** No claim about
"cross-cultural validity"; the data is taxonomic, not behavioral.

### Tier 2 — O*NET ↔ ESCO crosswalk (small, additive)

Train two heads (per doc 16) on JEPA latents from the **O*NET-trained** model:

- H1: predict O*NET cluster (existing).
- H3: predict the **ESCO** ISCO-08 group of the next state, using the public
  O*NET-to-ISCO crosswalk maintained by the U.S. Bureau of Labor Statistics
  and ESCO's ISCO mapping.

If H3 trained on frozen O*NET-JEPA latents reaches a non-trivial top-k against
ESCO labels, that is direct evidence the latent representation is
taxonomy-portable. If it doesn't, that is also a paper-quality result; report
it honestly.

Tier 2 is appendix-only. The paper headline does not depend on it.

## Data Sources

| Source | URL | License | Local Hash Field |
| --- | --- | --- | --- |
| ESCO classifications (CSV bundle) | https://esco.ec.europa.eu/en/use-esco/download | EUPL-1.2 / CC-BY-4.0 (component-dependent) | `esco_csv_sha256` |
| O*NET ↔ ISCO crosswalk (BLS) | published in the U.S. BLS / O*NET cross-classification table | CC-BY (record exact license at download) | `onet_isco_crosswalk_sha256` |
| Existing O*NET 30.2 ZIP | already used | CC-BY-4.0 | `b54792...3741` (recorded) |

`data_manifest.json` is extended with one entry per source, mirroring the
existing O*NET schema. Missing license text or hash mismatch is a typed
`PROVENANCE` error and the run aborts before ingest.

## ESCO Walk Graph

Mirror the O*NET walk generator behavior:

- nodes are ESCO occupations (ISCO-08 four-digit groupings, ≈436 codes);
- edges come from ESCO `broaderOccupation` / `narrowerOccupation` /
  `essentialSkill` / `optionalSkill` links;
- start node: `ICT system architects` (ISCO-08 group `2511`) for parity with
  the O*NET smoke starting at Software Developers;
- record per-edge SHA-256 in `career_taxonomy_esco_walk.csv`.

Walk graph generator must accept `--taxonomy {onet,esco}` and write
`generated_fixtures/<taxonomy>/...` so both runs are isolated in the bundle.

## CLI Additions

```bash
target/debug/context-graph dynamicjepa research-smoke \
  --run-id "$RUN_ID" \
  --purpose paper_med_5k \
  --taxonomy esco \
  --esco-csv-bundle tmp/esco_probe/esco_v1_2_x.zip \
  --career-event-count 5000 \
  --career-training-seed 42 \
  --json
```

When `--taxonomy esco` is set, `--onet-text-zip` is forbidden (and vice
versa). Forcing one taxonomy per bundle keeps the FSV story crisp.

## Crosswalk CLI

```bash
target/debug/context-graph dynamicjepa train-decoder \
  --jepa-artifact-id "$ONET_JEPA_ARTIFACT" \
  --head occupation_crosswalk \
  --crosswalk-table tmp/onet_isco_crosswalk/v2026.csv \
  --target-taxonomy esco \
  --json
```

Stored as a decoder-head artifact (per doc 16) with two extra fields:

```json
{
  "crosswalk_table_sha256": "<hex>",
  "target_taxonomy": "esco"
}
```

## Domain Pack Versioning

`career_taxonomy.v1.toml` stays as is. Add `career_taxonomy_esco.v1.toml` as a
sibling pack. Both packs share the JSON adapter and instrument schema; only
the categorical label sets and source manifests differ.

This is the litmus test that runtime universality (claim 2) holds: no
DynamicJEPA core code changes; only configuration changes.

## FSV Additions

Smokedoc 08 §Stage Sources Of Truth adds:

| Stage | Source Of Truth |
| --- | --- |
| ESCO data manifest | `data_manifest.json` `esco_csv_sha256`, `esco_walk_csv_sha256` |
| Crosswalk table | `data_manifest.json` `onet_isco_crosswalk_sha256` |
| Crosswalk decoder | `dj_model_artifacts` (kind=decoder_head, target_taxonomy=esco) |

Smokedoc 04 gains §Gate 12 (Cross-Taxonomy Replication):

> **Gate 12 — Cross-Taxonomy Replication**
>
> Pass criteria:
> - one passed `paper_med_5k` bundle per taxonomy (`onet`, `esco`);
> - both bundles use the same JEPA architecture, latent dim, and ablation
>   matrix;
> - both bundles have full FSV;
> - the ESCO walk SHA-256 is recomputed and matches the manifest;
> - per-bundle metric tables exist with parity columns.
>
> A failed Gate 12 does not block paper submission, but the paper must drop
> the cross-taxonomy claim from §Contributions.

## Statistical Reporting

Reuse the multi-seed aggregator (doc 15) per taxonomy. The paper reports two
side-by-side tables:

```text
table_prediction_metrics_multiseed_onet.csv
table_prediction_metrics_multiseed_esco.csv
```

A "comparable across taxonomies" claim requires:

- both tables exist with same n;
- BCa CIs overlap on the primary metric (val cosine);
- if they don't overlap, the paper says so explicitly.

## Optional: Karrierewege / JobHop / OpenResume

These are large ESCO-linked **trajectory** datasets (not taxonomy walks):

- Karrierewege: ~500K career paths, ESCO-linked.
- JobHop: ~2.3M work experiences, anonymized resumes, ESCO codes.
- OpenResume: anonymized + synthetic resumes, ESCO-normalized, on Zenodo.

Using any of them moves the benchmark from "controlled taxonomy walk" to
"public trajectory dataset". This is a strict scope expansion and:

- requires a privacy posture review;
- requires accepting a non-CC license in some cases;
- is **out of scope** for the first paper (doc 11 §Open Question Q4 stays
  open);
- is **in scope** for a Phase 12 follow-up (doc 25).

For the first paper, mention these datasets in §Related Work and note them as
the natural next step. Do not claim results on them.

## Edge Cases (Fail-Closed)

- Both `--onet-text-zip` and `--esco-csv-bundle` set: `VALIDATION`.
- ESCO bundle ZIP missing required member files (`occupations_en.csv`,
  `skills_en.csv`, `broaderRelationsOccPillar.csv`): `PROVENANCE`.
- ESCO walk start node not present in current ESCO version: `SCHEMA_MISMATCH`
  with the version string.
- Crosswalk table maps to occupations not in either taxonomy: `SCHEMA_DRIFT`
  naming the offending rows.

## Acceptance

This work is done when:

- `--taxonomy esco` and `--esco-csv-bundle` are supported by `research-smoke`;
- ESCO `paper_med_5k` bundle exists with full FSV;
- crosswalk decoder head trained and stored as artifact;
- doc 13 includes both taxonomy headline numbers;
- doc 11 §Open Question Q3 is moved to "Resolved" with a pointer to this doc
  and the bundle paths.
