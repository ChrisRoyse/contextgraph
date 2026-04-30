# 25 - Privacy Gate And Phase 12 (Real Subject-Level Data)

## Goal

Define the path from public-taxonomy benchmark (current) to a hypothetical
licensed real-data run, with privacy and legal gates that fail closed before
any real subject-level record enters the pipeline. **This work is explicitly
post-paper, post-submission, and not required for any current claim.**

The reason this doc exists: the paper makes specific non-claims about real
recruiting data (smokedoc 01). Those non-claims need a forward path that
preserves the FSV discipline and protects subjects. Without that path
documented, future work on the same runtime risks regressing the
auditability story.

## Triggering Conditions

This work activates only when **all** of the following are simultaneously
true:

1. A licensed dataset becomes available with explicit allow-use for ME-JEPA-
   style research, including individual-level training data.
2. An IRB / ethics review is approved for the project (institutional or
   independent body).
3. A privacy posture has been written and approved by an independent
   reviewer.
4. The first paper has been submitted or published; the public-taxonomy
   evidence path is locked.

If any of these fail, **stay on the public taxonomy benchmark**. This doc
does not authorize starting Phase 12; it documents what Phase 12 must do if
ever started.

## Privacy Gate Layers

Five layers, all required, all fail-closed.

### Layer 1 — Source-Of-Truth Gate

A new `dj_subject_consent` CF tracks per-subject consent and license terms.
Every event ingest must:

- match a row in `dj_subject_consent`;
- prove the consent row's hash matches the hash recorded in the upstream
  source manifest;
- prove the consent timestamp predates the event ingest timestamp;
- prove the allowed-use scope covers ME-JEPA training.

Any failure: typed `CONSENT_MISMATCH`. No raw event row is written.

### Layer 2 — Pseudonymization Gate

Every subject identifier going into `dj_raw_events` must be replaced with a
deterministic pseudonym derived from the source identifier and a salt held
**outside** the bundle. The salt is recorded in `data_manifest.json` only as
a SHA-256 hash; the salt itself never enters any persisted artifact.

Operator workflow: load salt into env var; ingest writes pseudonyms; verify
that no raw identifier survives in any CF (including audit log).

A separate verifier scans every CF for known-PII patterns (regex over the
salt's canonical inputs) and fails closed on detection.

### Layer 3 — Field-Level Redaction Gate

Domain pack manifests gain a `pii_fields: [...]` list. Adapters reject any
ingest where a `pii_fields` entry is present in the input record. The reject
is at adapter-time, not after; the raw event CF never sees the value.

For derived numeric fields (e.g. age in years), bin to k-anonymous brackets
(domain-pack configurable, e.g. age in 5-year brackets). Verify k ≥ 5 in the
generated dataset shard before training.

### Layer 4 — Differential Privacy Training Gate (Optional, Strong)

If the consent license requires DP training, the JEPA optimizer is replaced
with DP-SGD. The DP budget ($\epsilon, \delta$) is recorded in
`dj_training_runs` and printed in every artifact's evaluation report.

DP is opt-in per training run, not global. The verifier asserts:

- the optimizer kind matches the declared budget;
- the recorded $\epsilon, \delta$ are within the consent allow-use scope.

DP is harder than it looks; we are not implementing it speculatively. We
record the path so that the day a real-data project starts, the integration
point is obvious.

### Layer 5 — Deletion Propagation Gate

A subject revokes consent. The system must remove their data from every CF
**and** every model artifact derived from data that included them.

Per-CF deletion is straightforward (existing FSV provides counts).
Model-artifact deletion is hard: a model trained with a record cannot be
"unlearned" from cheaply. Two options:

- **Retrain-from-scratch** (slow, expensive, but provable);
- **Approximate unlearning** (research-only; not paper-claim quality).

Phase 12 ships only the retrain-from-scratch path. The deletion verifier
asserts:

- target subject pseudonym absent from every CF after deletion;
- every model artifact that used the subject's data is marked
  `tombstoned=true` in `dj_model_artifacts`;
- a fresh artifact has been trained from the post-deletion dataset and
  registered as the new active model;
- the deletion event row exists in `dj_audit_log` with the subject pseudonym
  hash and the operator identity.

## New Storage Schema

| CF | Purpose |
| --- | --- |
| `dj_subject_consent` | Consent rows; hash-linked to upstream source manifest |
| `dj_pii_redactions` | Audit log of redactions; never contains PII itself |
| `dj_dp_budgets` | DP budgets per training run; epsilon, delta, mechanism |
| `dj_deletion_requests` | Subject deletion requests; pseudonym hash + revoked-at |
| `dj_artifact_tombstones` | Model artifacts marked dead by deletion propagation |

These add five CFs. CLAUDE.md schema-change rule applies: wipe DB on schema
change. Phase 12 starts from a fresh DB.

## CLI Additions

```bash
target/debug/context-graph dynamicjepa register-consent \
  --db "$DB" \
  --consent-manifest path/to/consent_v1.toml \
  --salt-env CONSENT_SALT \
  --json

target/debug/context-graph dynamicjepa ingest-event \
  --db "$DB" \
  --domain career_real \
  --adapter json_event_pii_aware \
  --consent-required \
  --file "$REAL_FIXTURE" \
  --json

target/debug/context-graph dynamicjepa train \
  --db "$DB" \
  --dataset-id "$DATASET" \
  --config "$TRAIN_CONFIG" \
  --dp-epsilon 8.0 \
  --dp-delta 1e-5 \
  --json

target/debug/context-graph dynamicjepa propagate-deletion \
  --db "$DB" \
  --subject-pseudonym-hash "<hex>" \
  --retrain-active-models \
  --json
```

Every command adds typed errors named in this doc.

## FSV Additions

Smokedoc 08 §Stage Sources Of Truth gains:

| Stage | Source Of Truth |
| --- | --- |
| Consent | `dj_subject_consent`, `data_manifest.json` consent_manifest_sha256 |
| PII redaction | `dj_pii_redactions` |
| DP training | `dj_dp_budgets`, `dj_training_runs` (dp_kind field) |
| Deletion | `dj_deletion_requests`, `dj_artifact_tombstones`, `dj_audit_log` |

Smokedoc 04 §Gates gain Phase-12-only gates 14-18 (consent, pseudonymization,
PII redaction, DP, deletion). All gates default to **disabled** in non-Phase-12
runs and produce a typed error if a Phase-12 flag is passed without the gate
infrastructure present.

## Legal And Ethics Prerequisites

Required before any code runs in Phase 12:

- IRB approval letter or independent ethics letter (PDF).
- Data Use Agreement (DUA) with the source provider (PDF, hash recorded).
- Privacy Impact Assessment (PIA) document.
- Non-disclosure if applicable (PDF).
- Operator's data-handling training certificate (PDF, hash recorded in
  artifact bundle).

These PDFs are bundled into a separate `legal/` archive (not Zenodo public);
their SHA-256 is recorded in the bundle manifest. The bundle ships **without**
the legal archive contents but **with** their hashes, so reviewers can verify
the legal package exists without seeing it.

## Acceptance For Phase 12 Activation

Phase 12 starts only when:

- this doc's CLI surface is implemented behind a feature flag;
- all five privacy gate layers have integration tests;
- the legal package is collected and hashed;
- the deletion propagation flow runs end-to-end on a synthetic real-shaped
  fixture (not real data) and passes;
- a separate paper or technical report is planned for the Phase 12 results.

## Out Of Scope (Even For Phase 12 v1)

- Federated training across institutions.
- Homomorphic-encryption training.
- Server-side ML inference for real subjects (only training).
- Real-time consent updates (consent is snapshot at training-run start).

These are beyond v1 scope; document them as future work in the Phase-12
follow-up paper.

## Coordination With Other Docs

- Doc 21 §Risk Register adds Phase-12-specific risks once activation begins.
- Doc 20 §Artifact extends to publish the Phase-12 evaluation paper artifact
  (separate Zenodo record with stricter access).
- Doc 18 ESCO datasets (Karrierewege, JobHop, OpenResume) are the most likely
  first-Phase-12 candidates; their licenses must be reviewed against this
  doc's Layer 1 / Layer 3 requirements.

## Acceptance

This work is done **when activation is triggered**, not before. Until then,
this doc serves as:

- a forward-compatibility contract for the runtime;
- a reviewer-visible commitment that real-data work will not regress
  auditability;
- a specification ready to implement when the conditions in §Triggering
  Conditions are met.
