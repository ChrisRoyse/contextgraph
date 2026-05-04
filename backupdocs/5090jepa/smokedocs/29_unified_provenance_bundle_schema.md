# 29 - Unified Provenance Bundle Schema

## Goal

Define **one bundle schema that covers the entire closed loop**: DDA
substrate → ClipCannon multi-modal pipeline → ME-JEPA training → TCT
centroid construction → Constellation Guard runtime. A reviewer or
auditor reads one bundle and recomputes the full chain from raw corpus
to deployed prediction.

Smokedoc 03 already defines the ME-JEPA-only bundle. This doc extends
that schema to the synthesis layer, without forking it.

## Why One Bundle, Not Three

If DDA, ClipCannon, and ME-JEPA each emit their own bundle:

- Cross-system provenance breaks at every handoff.
- Reviewers must learn three schemas.
- A single arXiv supplementary is impossible.
- EU AI Act Article 12 logging requirements require a single
  end-to-end record per deployed system; three bundles are non-compliant.
- Drift detection (doc 23) cannot reason about whole-stack changes.

One bundle. Composable schemas. Per-stage subdirectories. Single
`run_manifest.json` at the root that names every contributing stage.

## Bundle Layout (Extends Smokedoc 03)

```text
tmp/research_runs/<run_id>/
  run_manifest.json                       # extended schema (below)
  environment.json                        # unchanged
  hardware.json                           # unchanged
  git_state.json                          # unchanged

  # Stage A — DDA substrate
  dda/
    corpus_manifest.json                  # source corpus + SHA-256
    embedder_panel_manifest.json          # 13 embedder versions + hashes
    fingerprint_index.parquet             # hash → fingerprint_id rows
    per_embedder_vectors.parquet          # n × N rows (sampled if >1M)
    pairwise_adapter_manifest.json        # rho_{jk} versions + hashes
    mi_audit.json                         # doc 30 mutual-info audit results
    paper_tables/
      table_dda_signals.csv

  # Stage B — ClipCannon (when video corpus)
  clipcannon/
    pipeline_dag.json                     # 23-stage DAG executed
    seven_modality_centroids.json         # frozen centroid per modality
    per_clip_modality_signals.parquet
    paper_tables/
      table_video_modality_coverage.csv

  # Stage C — ME-JEPA
  mejepa/
    data_manifest.json                    # smokedoc 03
    domain_pack_manifest.json
    db_counts_before.json
    db_counts_after.json
    training_metrics.json
    evaluation_report.json
    ablation_results.json
    edge_case_results.json
    reproducibility.json
    decoded_records/
    artifact_hashes.json
    paper_tables/                         # smokedoc 07
      table_runtime.csv
      ...

  # Stage D — TCT Constellation
  constellation/
    centroid_set_manifest.json            # frozen centroids per modality
    reference_set_manifest.json
    threshold_calibration.json
    guard_decisions.parquet               # one row per emitted output
    rejection_summary.json                # rejected count by stage
    paper_tables/
      table_guard_calibration.csv

  # Cross-stage
  command_log.jsonl                       # every CLI invocation across all stages
  fsv_report.json                         # extended (below)
  bundle_root_hash.txt                    # Merkle root over every file
  reviewer_recompute.sh                   # one-command end-to-end recheck
```

## Extended `run_manifest.json`

```json
{
  "schema_version": "2.0",
  "run_id": "...",
  "purpose": "smoke|paper_small|paper_med_5k|paper_full_50k|production",
  "stages_present": ["dda", "clipcannon", "mejepa", "constellation"],
  "stages_skipped": [],

  "dda": {
    "corpus_manifest_sha256": "...",
    "embedder_panel_manifest_sha256": "...",
    "n_inputs": 5000,
    "n_embedders": 13,
    "signals_per_input_upper_bound": 91,
    "signals_per_input_measured": 84,
    "mi_audit_path": "dda/mi_audit.json"
  },

  "clipcannon": {
    "n_clips": null,
    "modalities": null,
    "pipeline_dag_sha256": null
  },

  "mejepa": {
    "domain_id": "career_taxonomy",
    "domain_pack_version": "v2",
    "primary_artifact_id": "...",
    "primary_artifact_sha256": "..."
  },

  "constellation": {
    "centroid_set_id": "career_taxonomy_v1",
    "centroid_set_hash": "...",
    "reference_set_sha256": "...",
    "rejection_rate": 0.038
  },

  "bundle_root_hash": "...",
  "status": "running|passed|failed",
  "failure_details": null
}
```

`stages_skipped` is explicit so a reviewer knows when (e.g.) ClipCannon
was bypassed because the corpus was text-only.

## Bundle Root Hash (Merkle)

The bundle root hash is a Merkle root over every file in the bundle,
computed by `dynamicjepa hash-bundle --root <path> --out
bundle_root_hash.txt`. Excludes `bundle_root_hash.txt` itself.

This single hash is what:

- doc 23 CI compares to the reference;
- doc 20 publishes alongside the Zenodo DOI;
- product compliance reports emit;
- the reviewer recomputes via `reviewer_recompute.sh`.

Any byte change in any file flips the root hash. Drift is invisible
otherwise.

## Cross-Stage `fsv_report.json`

Extends smokedoc 03 to:

```json
{
  "schema_version": "2.0",
  "stage_reports": {
    "dda":          {"status": "passed", "evidence_paths": [...], "checks": [...]},
    "clipcannon":   {"status": "skipped", "reason": "text corpus"},
    "mejepa":       {"status": "passed", "evidence_paths": [...], "checks": [...]},
    "constellation":{"status": "passed", "evidence_paths": [...], "checks": [...]}
  },
  "cross_stage_consistency_checks": [
    {
      "name": "dda_fingerprint_to_mejepa_event_match",
      "expected": "every mejepa event hash is present in dda/fingerprint_index",
      "result": "passed",
      "evidence": "..."
    },
    {
      "name": "constellation_artifact_uses_mejepa_artifact",
      "expected": "centroid_set_manifest.frozen_jepa_artifact_sha256 == mejepa.primary_artifact_sha256",
      "result": "passed"
    }
  ],
  "bundle_root_hash": "..."
}
```

Cross-stage checks are the first-class evidence of system-of-systems
integrity. Without them, the four bundles would be independently
verified but mutually unverified.

## `reviewer_recompute.sh`

A reviewer-facing script committed to the bundle. One command:

```bash
./reviewer_recompute.sh \
  --bundle-root . \
  --container <registry>/contextgraph-mejepa:<tag> \
  --raw-corpus /path/to/onet_30_2_text.zip
```

What it does:

1. Pulls the pinned container.
2. Mounts raw corpus and bundle.
3. Re-runs the DDA stage; compares fingerprint hashes byte-for-byte.
4. Re-runs ME-JEPA training **deterministically** (matching seeds and
   versions); compares artifact hashes within configured tolerance
   (doc 23 tolerance budget).
5. Re-runs constellation centroid computation; compares centroid set
   hash byte-for-byte.
6. Re-runs Constellation Guard against persisted predictions; compares
   guard decisions.
7. Recomputes `bundle_root_hash`; compares to `bundle_root_hash.txt`.

Exit codes:

- 0: bit-equal recompute;
- 1: within tolerance, drift report written to `recompute_drift.json`;
- 2: out of tolerance, drift report written + reviewer should
  investigate;
- 3+: harness error (missing inputs, container mismatch, etc.).

## Schema Versioning

Bundle schemas are versioned (`schema_version` field). Reading a v1
bundle (smokedoc 03 only) under a v2 reader is supported; a v2 bundle
under a v1 reader fails closed.

CLAUDE.md anti-migration rule still applies: schema changes wipe the
DB. Old bundles remain readable forever, but new bundles always use
the latest schema. No "upgrade" path; only "produce next version".

## Privacy Considerations (Coordinates With Doc 25)

Phase 12 bundles produced under the privacy gate carry:

- `dda/per_embedder_vectors.parquet` is **dropped** (vectors derived
  from real subjects must not enter a public bundle);
- `dda/fingerprint_index.parquet` is replaced with a hash-only summary;
- `mejepa/decoded_records/` redacts PII fields per the domain pack's
  `pii_fields`;
- `constellation/reference_set_manifest.json` records hashes only;
- `bundle_root_hash` is computed over the redacted bundle.

Two parallel bundles exist: a public review bundle and a private
operator bundle. The private bundle hash is recorded in the public
bundle so the public review can attest the private exists without
exposing it.

## Edge Cases (Fail-Closed)

- A stage's manifest is missing but `stages_present` lists it:
  `BUNDLE_INCOMPLETE`.
- A stage's bundle hash mismatches the embedded manifest hash:
  `BUNDLE_HASH_MISMATCH`.
- Cross-stage check fails (e.g. mejepa event hash not in dda
  fingerprint index): `CROSS_STAGE_INCONSISTENCY`.
- `bundle_root_hash.txt` is missing or stale: `ROOT_HASH_MISSING`.
- Reviewer recompute script run on a different container digest than
  the bundle was built with: clear error naming both digests; do not
  silently proceed.

## Acceptance

This work is done when:

- the v2 bundle schema is implemented in
  `crates/context-graph-storage/src/bundle/`;
- `dynamicjepa hash-bundle` and `reviewer_recompute.sh` exist;
- one end-to-end paper-small bundle exists in v2 format with all four
  stages present;
- a deliberately corrupted bundle fails the recompute with the right
  typed error;
- doc 20 (artifact + Zenodo) publishes only v2 bundles for any release.
