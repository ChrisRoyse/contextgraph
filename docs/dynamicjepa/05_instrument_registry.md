# Instrument Registry

The instrument registry is the key to making DynamicJEPA universal.

## Instrument Definition

An instrument is a frozen or controlled model/function that transforms a typed input into a representation, score, or structured feature.

```text
Instrument {
  instrument_id,
  name,
  modality,
  domain_scope[],
  version,
  artifact_hash,
  preprocessing_hash,
  output_kind,
  dimension_contract,
  normalization_contract,
  allowed_input_event_kinds[],
  required_policy_gates[],
  validation_report_id,
  status
}
```

## Instrument Classes

| Class | Examples |
| --- | --- |
| Text | E5, BGE, ColBERT, SPLADE |
| Image/video | V-JEPA, SigLIP, CLIP, video encoders |
| Audio | WavLM, A-JEPA-style encoders |
| Graph | TransE, KEPLER, graph neural encoders |
| Code | Qodo, CodeBERT-style encoders |
| Time series | forecasting encoders, market/sensor encoders |
| Spatial/3D | depth, point cloud, pose, occupancy encoders |
| Domain expert | skills, comp, genomics, molecular, medical, legal |
| Safety | PII detector, bias detector, toxicity, policy scanner |

## Registration Requirements

An instrument cannot be used until:

1. artifact exists,
2. artifact hash is stored,
3. output shape is declared,
4. validation harness passes,
5. at least one real sample has been materialized and read back,
6. failure behavior is typed,
7. version is immutable.

No automatic fallback to another instrument is allowed.

## Dynamic Panels

Each domain pack declares a required panel:

```text
DomainPanel {
  domain_id,
  required_instruments[],
  optional_instruments[],
  pairwise_feature_policy,
  binding_policy,
  minimum_panel_version,
  validation_report_id
}
```

If a required instrument is unavailable, the domain pack is unavailable.

## Instrument Evolution

New versions create new panel versions. They do not overwrite old panels.

Serving must declare which panel version produced:

- training dataset,
- predictor artifact,
- read-out head,
- guard threshold,
- output.

This prevents silent drift.

