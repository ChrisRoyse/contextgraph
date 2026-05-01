# Latent Panel And Binding

The latent panel is the DynamicJEPA state representation. Binding is how the system knows which instrument outputs refer to the same event, entity, state, or action.

## Latent Panel Record

```text
DynamicPanelRecord {
  panel_id,
  domain_id,
  event_id_or_state_id,
  panel_version,
  instrument_outputs[],
  output_hashes[],
  vector_norms,
  missing_required_outputs[],
  validation_status,
  created_at,
  audit_id
}
```

`missing_required_outputs` must be empty for a valid panel. It exists for diagnostics, not fallback.

## Instrument Output

```text
InstrumentOutput {
  instrument_id,
  version,
  output_kind,
  shape,
  storage_ref,
  hash,
  normalization_status,
  validation_status
}
```

## Binding Record

```text
BindingRecord {
  binding_id,
  domain_id,
  subject_key,
  event_ids[],
  panel_ids[],
  alignment_kind,
  confidence,
  evidence_refs[],
  created_at,
  audit_id
}
```

Binding kinds:

- same event,
- same entity,
- same actor,
- same object across modalities,
- same time window,
- cause-effect relation,
- action-outcome relation,
- source-derived duplicate.

## Pairwise And Higher-Order Features

For N instruments:

```text
signals = N + choose(N, 2)
```

Pairwise features include:

- cosine agreement,
- rank disagreement,
- modality contradiction,
- causal direction disagreement,
- sparse/dense mismatch,
- temporal surprise,
- safety proximity.

Higher-order features should be added only when a domain needs them and they have a defined validation harness.

## Dynamic Binding Problem

The hard part of a universal world model is not just encoding observations. It is binding:

- which object in video corresponds to which text reference,
- which database row corresponds to which event,
- which code commit caused which test failure,
- which action caused which observed outcome,
- which sensor stream belongs to which physical entity.

DynamicJEPA must store binding as explicit state, not an implicit side effect of embeddings.

