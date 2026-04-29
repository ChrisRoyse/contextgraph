# Implementation Roadmap

This roadmap assumes the legal gate clears. If it does not clear, implementation stops.

## Phase 0: Planning And Gates

Deliverables:

- this doc set,
- legal gate source-of-truth design,
- privacy triage source-of-truth design,
- ME-JEPA CF/table migration plan,
- FSV harness skeletons.

Acceptance:

- docs reviewed,
- no unresolved ambiguity about source of truth,
- no implementation starts on real corpus data.

## Phase 1: Storage And Types

Deliverables:

- ME-JEPA core types,
- ME-JEPA storage namespace,
- migration/open logic,
- typed errors,
- audit write/readback pattern.

Acceptance:

- storage opens with all required CFs/tables,
- missing CF is fail-fast,
- unit tests cover serialization shape,
- FSV-0 storage readback passes.

## Phase 2: Legal Gate And Privacy Triage

Deliverables:

- legal gate record API,
- subject and consent state records,
- PII vault adapter boundary,
- jurisdiction tagging record,
- deletion request lifecycle.

Acceptance:

- FSV-0 and FSV-1 pass,
- raw PII is provably absent from ME-JEPA event payloads,
- denied/quarantined records cannot be embedded.

## Phase 3: Source Adapters To Typed Events

Deliverables:

- SQL adapter interface,
- document/OCR event adapter interface,
- email/conversation adapter interface,
- typed event normalizer,
- event provenance hash checks.

Acceptance:

- allowed source event persists,
- source metadata readback works,
- malformed/empty/unsupported inputs fail with typed errors.

## Phase 4: Latent Panel

Deliverables:

- E1-E14 reference/copy strategy,
- Erec1-Erec5 provider interfaces,
- panel materialization,
- pairwise materialization,
- validation and version registry.

Acceptance:

- FSV-2 and FSV-3 pass,
- N=19 rows contain all expected slots,
- 171 pairwise features are physically present,
- missing embedder does not fallback to N=18.

## Phase 5: Trajectories And Splits

Deliverables:

- trajectory builder,
- event ordering,
- leakage detector,
- split manifest writer,
- dataset export.

Acceptance:

- FSV-4 and FSV-5 pass,
- train/test leakage is detected,
- revoked/deleted records are excluded.

## Phase 6: Predictor Training

Deliverables:

- tiny predictor training job,
- baseline predictor,
- full objective predictor,
- artifact registry,
- evaluation reports.

Acceptance:

- FSV-6 passes,
- model artifact hashes match registry,
- baseline eval exists before full model eval,
- collapse diagnostics are reported.

## Phase 7: First Heads And Guard

Deliverables:

- `H_comp`,
- `H_match`,
- `H_anti`,
- `H_traj`,
- guard decision writer,
- constellation/anti-pole threshold calibration.

Acceptance:

- FSV-7 passes,
- guard readback blocks unaudited output,
- first head has model card and evaluation report.

## Phase 8: Deletion And Compliance Loop

Deliverables:

- deletion propagation,
- artifact impact analysis,
- consent revocation report,
- dataset/model card generation.

Acceptance:

- FSV-8 passes,
- deleted subject cannot enter future export,
- impacted artifacts are flagged.

## Phase 9: Scale

Deliverables:

- full corpus ingest,
- operational dashboards,
- batch training,
- production-serving hardening,
- external audit package.

Acceptance:

- full run evidence report,
- physical counts reconcile at every layer,
- product heads only serve approved artifacts.

