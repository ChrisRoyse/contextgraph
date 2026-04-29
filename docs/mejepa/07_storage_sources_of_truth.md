# Storage Sources Of Truth

This document defines where ME-JEPA state lives. Exact Rust implementation may evolve, but these source-of-truth boundaries should not.

## Existing Source-Of-Truth Layers

| Existing layer | Current source of truth |
| --- | --- |
| content fingerprints | `CF_FINGERPRINTS` |
| source text/content | `CF_CONTENT` |
| source provenance | `CF_SOURCE_METADATA` |
| embedder versions | `CF_EMBEDDING_REGISTRY` |
| content training export | `CF_TRAINING_RECORDS` |
| constellations | `CF_CONSTELLATIONS`, `CF_CONSTELLATION_BY_SELECTOR` |
| audit | `CF_AUDIT_LOG`, `CF_AUDIT_BY_TARGET` |
| typed edges | `CF_TYPED_EDGE_RECORDS`, `CF_TYPED_EDGE_VALIDATIONS` |
| learning/outcome pattern | `CF_LEARNING_EVENTS` |

## Proposed ME-JEPA Storage Namespace

Use dedicated ME-JEPA column families or equivalent physical tables. Do not overload existing E1-E14 content CFs for trajectory-specific state.

| Proposed source of truth | Stores |
| --- | --- |
| `mejepa_legal_gates` | title/privacy/client-review gate artifacts and hashes |
| `mejepa_subjects` | pseudonymous candidate/company/recruiter/hiring-manager IDs |
| `mejepa_consent_state` | consent envelope per subject/event/client |
| `mejepa_jurisdiction_tags` | jurisdiction evidence and strict-mode flags |
| `mejepa_deletion_requests` | deletion/opt-out lifecycle |
| `mejepa_source_events` | normalized typed event stream |
| `mejepa_event_payloads` | de-identified payload blobs or hashes |
| `mejepa_panel_records` | E1-E14 + Erec1-Erec5 materialized latent panels |
| `mejepa_pairwise_records` | 171 pairwise feature records |
| `mejepa_trajectories` | ordered event sequences |
| `mejepa_split_manifests` | train/validation/test assignments |
| `mejepa_constellations` | domain constellations and anti-poles, if separated from global CF |
| `mejepa_training_datasets` | predictor/head dataset exports |
| `mejepa_training_runs` | training run metadata and logs |
| `mejepa_model_artifacts` | model artifact registry and hashes |
| `mejepa_eval_reports` | evaluation reports with source dataset links |
| `mejepa_guard_decisions` | G_tau output decisions |
| `mejepa_audit_log` | ME-JEPA-specific audit stream if not merged into global audit |

## Trigger -> Source Of Truth Matrix

| Trigger | Mutates | Independent verification |
| --- | --- | --- |
| legal gate approval | `mejepa_legal_gates` | read gate row and hash source documents |
| subject upsert | `mejepa_subjects` | read subject row, count before/after |
| consent classification | `mejepa_consent_state` | read consent row, verify policy decision |
| source adapter ingest | `mejepa_source_events`, `mejepa_event_payloads` | read event/payload rows and source hashes |
| panel materialization | `mejepa_panel_records`, `CF_EMBEDDING_REGISTRY` | read panel row, verify dimensions/norms/version set |
| pairwise materialization | `mejepa_pairwise_records` | read row, verify 171 expected pair slots |
| trajectory build | `mejepa_trajectories` | read sequence, verify event order and IDs |
| split assignment | `mejepa_split_manifests` | read split, verify no leakage |
| training export | `mejepa_training_datasets` | read dataset manifest and row count |
| predictor training | `mejepa_training_runs`, `mejepa_model_artifacts` | hash weights/logs/eval; read registry row |
| head training | `mejepa_training_runs`, `mejepa_model_artifacts` | same as predictor |
| guarded inference | `mejepa_guard_decisions`, audit log | read guard decision and audit row before returning output |
| deletion request | `mejepa_deletion_requests`, `mejepa_consent_state`, vault | read deletion row, consent status, and vault state |

## Physical Verification

Every FSV report must include:

- storage path,
- list of column families/tables inspected,
- before counts,
- after counts,
- sample keys,
- decoded sample values or value hashes,
- audit IDs,
- model/dataset artifact hashes when relevant.

For RocksDB:

```bash
ldb --db=<db> --column_family=<cf> dump --count_only
```

For artifact directories:

```bash
sha256sum <artifact>
find <artifact-dir> -maxdepth 2 -type f -print
```

No test passes only because an API returned `ok`.

