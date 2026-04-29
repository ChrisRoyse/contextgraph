# Latent Panel And Schema

The ME-JEPA latent panel is the target representation. It is built from frozen embedders and persisted as a source-of-truth artifact before training.

## Panel Composition

Base content panel from Context Graph:

| Slot | Role |
| --- | --- |
| E1 | semantic |
| E2 | temporal recent |
| E3 | temporal periodic |
| E4 | temporal positional |
| E5 | causal |
| E6 | sparse keyword |
| E7 | code/pattern |
| E8 | graph |
| E9 | typo/noise robust |
| E10 | contextual/paraphrase |
| E11 | entity/KEPLER |
| E12 | late interaction |
| E13 | SPLADE expansion |
| E14 | BGE-M3 dense |

Recruiting panel additions:

| Slot | Role | Training/source |
| --- | --- | --- |
| Erec1 | skills taxonomy | ESCO/O*NET/Lightcast-style skills mapping plus corpus phrases |
| Erec2 | compensation context | role, seniority, geography, year, compensation outcome |
| Erec3 | career trajectory | ordered role/company/tenure transitions |
| Erec4 | industry vertical | NAICS/industry classifier and corpus labels |
| Erec5 | conversation/soft-skill | conversation segments and placement/retention outcomes |

Total: N = 19 embedders.

Pairwise interaction count:

```text
19 + choose(19, 2) = 19 + 171 = 190 structured signals per event
```

## Do Not Mutate SemanticFingerprint For ME-JEPA

`SemanticFingerprint` remains the E1-E14 content wire format. ME-JEPA needs a separate record:

```text
MejepaPanelRecord {
  event_id,
  source_content_id?,
  embedder_version_set_id,
  vectors: E1-E14 references or copies,
  recruiting_vectors: Erec1-Erec5,
  vector_norms,
  validation_status,
  created_at,
  audit_id
}
```

Reason:

- avoids breaking existing storage,
- allows domain-specific Erec evolution,
- supports multiple recruiting panels per event version,
- makes FSV source-of-truth boundaries explicit.

## Pairwise Feature Record

```text
MejepaPairwiseRecord {
  event_id,
  panel_record_id,
  pairwise_version,
  pairs: [
    { left_slot, right_slot, feature_kind, value_or_vector_hash }
  ],
  dense_blob_hash,
  created_at,
  audit_id
}
```

The first implementation should choose a compact, explicit pairwise feature set. The plan should not store undefined "magic interaction features." Each feature kind must be versioned.

Recommended initial feature kinds:

- cosine agreement,
- signed distance delta when applicable,
- rank disagreement bucket,
- normalized dot product,
- sparse/dense overlap score for E13/E6 vs dense spaces.

## Validation Invariants

Panel materialization must fail if:

- any required vector is missing,
- any vector has the wrong dimension,
- any dense vector contains NaN or infinity,
- any dense vector is all-zero unless the embedder explicitly supports zero for that modality and the event records that reason,
- any sparse vector has invalid indices,
- E12 token-level rows have impossible token counts,
- Erec slots are computed with unregistered model versions,
- source content hash does not match the content row,
- consent envelope does not allow embedding.

## Source Of Truth

The source of truth is not the model return value. It is:

- the event row,
- the persisted panel row,
- the persisted pairwise row,
- the embedding registry/model version row,
- the audit row.

FSV must read all of them after materialization.

