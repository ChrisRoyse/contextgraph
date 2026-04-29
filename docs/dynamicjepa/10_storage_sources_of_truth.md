# Storage Sources Of Truth

DynamicJEPA needs physical state that can be inspected independently of API return values.

## Proposed Storage Namespace

| Source of truth | Stores |
| --- | --- |
| `dynamic_domains` | registered domain packs |
| `dynamic_instruments` | registered instruments and artifact hashes |
| `dynamic_adapters` | adapter versions and validation reports |
| `dynamic_events` | universal events |
| `dynamic_actions` | universal actions |
| `dynamic_outcomes` | observed outcomes |
| `dynamic_states` | state snapshots |
| `dynamic_panels` | materialized latent panels |
| `dynamic_bindings` | cross-modal/entity/action bindings |
| `dynamic_pairwise` | pairwise instrument features |
| `dynamic_trajectories` | ordered state/action/outcome sequences |
| `dynamic_splits` | train/validation/test split manifests |
| `dynamic_constellations` | target manifolds and anti-poles |
| `dynamic_training_datasets` | exported datasets |
| `dynamic_training_runs` | run configs, logs, metrics |
| `dynamic_model_artifacts` | predictor/head artifact registry |
| `dynamic_skills` | verified skills and workflows |
| `dynamic_plans` | proposed and executed plans |
| `dynamic_guard_decisions` | verifier decisions |
| `dynamic_surprise_events` | prediction/observation mismatches |
| `dynamic_audit_log` | append-only audit stream |

## Trigger Matrix

| Trigger | Mutates | Verification |
| --- | --- | --- |
| register domain pack | `dynamic_domains` | read row, hash pack file |
| register instrument | `dynamic_instruments` | read row, hash artifact |
| ingest event | `dynamic_events` | read event, source hash |
| record action | `dynamic_actions` | read action, params hash |
| materialize panel | `dynamic_panels` | read panel, validate shapes |
| bind modalities/entities | `dynamic_bindings` | read binding and evidence refs |
| build trajectory | `dynamic_trajectories` | read ordered IDs |
| export dataset | `dynamic_training_datasets`, `dynamic_splits` | read manifest, count rows |
| train predictor | `dynamic_training_runs`, `dynamic_model_artifacts` | hash artifact and logs |
| propose plan | `dynamic_plans` | read plan and predicted states |
| execute skill | `dynamic_skills`, `dynamic_actions`, `dynamic_outcomes` | read before/action/after |
| guard output | `dynamic_guard_decisions`, `dynamic_audit_log` | read guard and audit rows |
| surprise event | `dynamic_surprise_events` | read predicted/observed refs |

## Artifact Store

Large files live in an artifact directory or object store, but the database stores:

- logical artifact ID,
- path,
- sha256,
- byte length,
- producing command,
- source dataset IDs,
- approval status.

An artifact row without a matching file hash is invalid.

## Compatibility With Context Graph

DynamicJEPA should reuse Context Graph patterns:

- RocksDB column-family discipline,
- append-only audit log,
- source metadata,
- embedding registry,
- constellations,
- typed-edge storage,
- FSV reports.

It should not destabilize existing `SemanticFingerprint` storage.

