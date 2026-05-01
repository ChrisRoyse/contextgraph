# Data Contract And Privacy Gate

ME-JEPA is only valuable if the data can be used defensibly. The data contract is therefore part of the system architecture, not legal paperwork outside the system.

## Processing States

Every source record and derived event must carry one of these states:

| State | Meaning | Pipeline behavior |
| --- | --- | --- |
| `allowed_full` | Approved for model training and aggregate products | May enter full latent panel and training export |
| `allowed_anonymized_only` | Approved only after de-identification | May enter latent panel only after vault separation and PII scrub |
| `aggregate_only` | May be used only for k-anonymous aggregate outputs | Excluded from individual-level prediction heads |
| `quarantined` | Legal/privacy status unresolved | No embedding, no training, no product use |
| `client_restricted` | Client contract limits use | Excluded or narrowed by client-use policy |
| `pending_deletion` | Subject request or legal hold active | No new processing |
| `deleted` | Deletion completed | Join keys destroyed; derived eligibility removed |
| `excluded` | Permanently out of scope | No processing |

The system must error if a required consent state is missing.

## PII Vault Rule

Raw identifiers belong in a separate vault:

```text
person_id_hash -> raw identifiers
```

The ME-JEPA data plane stores only pseudonymous IDs and source hashes. The vault must be independently deletable.

Deletion request behavior:

1. Record deletion request.
2. Find vault identity.
3. Destroy or tombstone join keys.
4. Mark all linked events ineligible for future training/export.
5. Write audit row.
6. Read back vault and ME-JEPA state to prove the outcome.

## Consent Envelope

Every derived object inherits the strictest consent envelope of its inputs.

Example:

```text
trajectory consent = min(consent of all events in trajectory)
training row consent = min(trajectory consent, target event consent)
model artifact consent = manifest of all dataset consent envelopes
```

If a training run includes a row later revoked, the model artifact must be flagged as impacted. The system does not silently continue serving impacted artifacts.

## Jurisdiction Tags

Every subject/event should store:

- direct jurisdiction evidence, if present,
- inferred jurisdiction evidence, if any,
- confidence and provenance,
- strictest applicable regime.

Strict-mode subjects must be excluded from any use not explicitly approved.

## Aggregation Rules

Aggregate products such as compensation intelligence require:

- minimum group size,
- differential privacy configuration where applicable,
- no raw row leakage,
- no named-person outputs,
- source cohort manifest,
- readback proof that the aggregation row exists and source IDs are traceable.

## Hard Errors

These are hard failures:

- missing title gate,
- missing consent state,
- missing jurisdiction tag for a human subject,
- raw PII detected outside vault,
- target record in `quarantined`, `pending_deletion`, `deleted`, or `excluded`,
- training export containing restricted records,
- product output that lacks an audit row,
- model artifact without dataset manifest and consent manifest.

No fallback is allowed for any of these.

