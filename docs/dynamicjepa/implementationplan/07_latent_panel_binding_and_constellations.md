# Latent Panel, Binding, And Constellations

The latent panel is the immediate JEPA input representation. Binding records make panels compositional across entities, actions, goals, and domains. Existing Context Graph constellations should be reused as aggregate anchors and reference states.

## Latent Panel Materialization

`LatentPanel` is built from a normalized transition and its instrument readings.

Construction:

1. Load domain pack by id/version.
2. Load transition, state, action, and outcome references.
3. Run or read required instrument readings.
4. Sort readings by domain-pack slot order.
5. Validate shapes and finite values.
6. Build slot vectors and masks.
7. Compute panel hash.
8. Persist panel and readings in one batch.
9. Independently read panel and readings back.

Panel slots should be typed:

- `state_slot`
- `action_slot`
- `outcome_slot`
- `entity_slot`
- `time_slot`
- `context_slot`
- `goal_slot`
- `graph_slot`
- `memory_slot`

## Panel Shape Contract

Every panel row includes:

- `slot_count`;
- per-slot instrument id;
- per-slot dimension;
- per-slot mask;
- total dense dimension;
- sparse feature count if any;
- source transition id;
- source reading ids.

Validation rejects:

- slot count mismatch;
- missing required slot;
- duplicate slot id;
- vector dimension mismatch;
- NaN or infinity;
- panel hash mismatch;
- reading referenced by panel but missing in storage.

## Binding Records

Binding connects independently observed things into reusable objects:

- entity to entity;
- event to trajectory;
- state to goal;
- action to skill;
- panel slot to memory fingerprint;
- domain object to cross-domain object.

Binding methods:

- deterministic id equality;
- explicit domain pack mapping;
- existing entity extractor evidence;
- graph-neighborhood evidence;
- constellation proximity;
- model-predicted equivalence after FSV is available.

Bindings must include:

- `method`;
- `score`;
- `evidence_refs`;
- `left_domain_pack_id`;
- `right_domain_pack_id`;
- `created_by_run_id`.

## Existing Constellation Integration

Context Graph already has `CF_CONSTELLATIONS` and constellation compiler tools. DynamicJEPA should use constellations in two ways:

1. As existing source context: a panel can include a `memory_slot` or `graph_slot` whose values are derived from stored memory constellations.
2. As DynamicJEPA aggregate states: trajectories and panels can be compiled into DynamicJEPA-specific constellation records or linked to existing constellation ids.

Do not overload existing constellation records with DynamicJEPA-only fields. Store DynamicJEPA links in DynamicJEPA CFs and reference existing constellation ids when needed.

## Required APIs

Core:

- `LatentPanel::validate_against_domain_pack`
- `LatentPanel::canonical_hash`
- `BindingRecord::validate_evidence`
- `PanelShape::from_domain_pack`

Storage:

- `put_latent_panel_batch`
- `get_latent_panel`
- `list_panels_by_transition`
- `put_binding_batch`
- `get_binding`
- `list_bindings_for_entity`

Service:

- `materialize_panel`
- `bind_event_entities`
- `bind_panel_to_constellation`
- `score_panel_against_constellation`

MCP:

- `dynamicjepa_materialize_panel`
- `dynamicjepa_get_panel`
- `dynamicjepa_create_binding`
- `dynamicjepa_list_bindings`

CLI:

- `dynamicjepa materialize-panel`
- `dynamicjepa inspect-panel`
- `dynamicjepa bind`
- `dynamicjepa list-bindings`

## Source-Of-Truth Verification

Happy path:

- before: count readings, panels, bindings;
- action: materialize panel for `counter_world` event;
- after: readings +1, panels +1;
- read: panel by id and reading by `[event_id][instrument_hash]`;
- action: bind event entity `counter:global` to panel id;
- after: binding +1 and binding-by-entity index +1;
- read: primary binding and index scan for entity.

Edge cases:

- missing required reading: panel not written, failed run/error printed;
- wrong vector dimension: panel not written, readings not committed in final batch;
- binding evidence id missing: binding not written, index count unchanged.
