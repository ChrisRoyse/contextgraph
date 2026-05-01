# 22 — Core Records, ID Wrappers, Serde

This document is the file-level diff plan for the
`crates/context-graph-core` changes (primarily PR 2 of
`20_implementation_plan_overview.md`).

## 1. The principle

The core crate is the *type definition layer*; it has no I/O. Every
new persistence target in the storage crate reads / writes typed
structs defined here. Per the v1 codebase's lesson:

> bincode + skip\_serializing\_if = BROKEN; use JSON for all
> provenance/SourceMetadata.

All new records use `serde_json` for serialization, with
`#[serde(default)]` on optional fields to maintain forward-compat
under the no-backwards-compat rule.

## 2. New typed records

### 2.1 `PairwiseReading`

`crates/context-graph-core/src/dynamicjepa/records/pairwise.rs` (NEW)

```rust
use serde::{Serialize, Deserialize};
use validator::Validate;
use crate::dynamicjepa::ids::{
    EventId, PairwiseReadingId, InstrumentId,
};
use crate::dynamicjepa::pair_kinds::PairKindBitset;
use crate::dynamicjepa::reading_status::ReadingStatus;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct PairwiseReading {
    /// Unique identifier for this pairwise feature row.
    pub pairwise_id: PairwiseReadingId,

    /// The raw event whose two per-instrument readings produced this pair.
    pub event_id: EventId,

    /// The lower-indexed instrument in the pair (j < k strict).
    pub instrument_j: InstrumentId,

    /// The higher-indexed instrument in the pair.
    pub instrument_k: InstrumentId,

    /// SHA-256 of phi_j's frozen weights at the time this row was produced.
    pub instrument_j_artifact_hash: [u8; 32],

    /// SHA-256 of phi_k's frozen weights at the time this row was produced.
    pub instrument_k_artifact_hash: [u8; 32],

    /// Bitset of pair-kinds emitted (see PairKindBitset constants).
    pub kinds_emitted: PairKindBitset,

    /// Always-present cosine agreement of the two readings.
    /// fp32; range [-1.0, 1.0].
    pub cosine_agreement: f32,

    /// Optional rank-disagreement; populated only when both instruments
    /// declare "rank_disagreement" in their TOML pair_kinds list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank_disagreement: Option<f32>,

    /// Optional modality-contradiction flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modality_contradiction: Option<bool>,

    /// Optional sparse/dense mismatch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sparse_dense_mismatch: Option<f32>,

    /// Optional temporal-surprise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal_surprise: Option<f32>,

    /// Optional causal-direction-disagreement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub causal_direction_disagreement: Option<bool>,

    /// Optional safety-proximity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety_proximity: Option<f32>,

    pub created_at_unix_ms: u64,

    pub validation_status: ReadingStatus,
}

pub const PAIRWISE_READING_RECORD_VERSION: u32 = 1;

impl PairwiseReading {
    /// Return Vec of (kind_name, present_value_string) for emitted kinds.
    pub fn emitted_kinds_summary(&self) -> Vec<(String, String)> {
        let mut out = vec![
            ("cosine_agreement".to_string(), format!("{:.4}", self.cosine_agreement)),
        ];
        if let Some(v) = self.rank_disagreement {
            out.push(("rank_disagreement".to_string(), format!("{:.4}", v)));
        }
        // ... etc for the 5 other optional kinds ...
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pairwise_reading_serde_roundtrip_minimal() {
        let pr = PairwiseReading {
            pairwise_id: PairwiseReadingId::new_v4(),
            event_id: EventId::new_v4(),
            instrument_j: InstrumentId(1),
            instrument_k: InstrumentId(2),
            instrument_j_artifact_hash: [0u8; 32],
            instrument_k_artifact_hash: [1u8; 32],
            kinds_emitted: PairKindBitset(PairKindBitset::COSINE_AGREEMENT),
            cosine_agreement: 0.42,
            rank_disagreement: None,
            modality_contradiction: None,
            sparse_dense_mismatch: None,
            temporal_surprise: None,
            causal_direction_disagreement: None,
            safety_proximity: None,
            created_at_unix_ms: 1779000000000,
            validation_status: ReadingStatus::Valid,
        };

        let json = serde_json::to_vec(&pr).unwrap();
        let pr2: PairwiseReading = serde_json::from_slice(&json).unwrap();
        assert_eq!(pr.cosine_agreement, pr2.cosine_agreement);
    }

    #[test]
    fn test_pairwise_reading_validate_rejects_inverted_ordering() {
        let pr = PairwiseReading {
            // ... j=2, k=1 deliberate inversion ...
            instrument_j: InstrumentId(2),
            instrument_k: InstrumentId(1),
            // ...
        };
        assert!(pr.validate().is_err());
    }

    // ... more tests ...
}
```

### 2.2 `ConstellationCentroid`

`crates/context-graph-core/src/dynamicjepa/records/constellation.rs` (NEW)

Same pattern. Key field is `centroid: Vec<f32>` of length $d_m$.
Stability and provenance fields all required.

### 2.3 `ThresholdCalibration`

`crates/context-graph-core/src/dynamicjepa/records/threshold.rs` (NEW)

Same pattern. Key field is `tau: f32` plus `supersede_seq: u32` and
optional `supersedes_uuid: Option<ThresholdCalibrationId>`.

## 3. ID newtype additions

`crates/context-graph-core/src/dynamicjepa/ids.rs`

The existing file has the pattern:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AuditId(pub Uuid);
```

Three new types follow the same pattern:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PairwiseReadingId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConstellationId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ThresholdCalibrationId(pub Uuid);

impl PairwiseReadingId {
    pub fn new_v4() -> Self { Self(Uuid::new_v4()) }
    pub fn nil() -> Self { Self(Uuid::nil()) }
}

impl ConstellationId {
    pub fn new_v4() -> Self { Self(Uuid::new_v4()) }
    pub fn nil() -> Self { Self(Uuid::nil()) }
}

impl ThresholdCalibrationId {
    pub fn new_v4() -> Self { Self(Uuid::new_v4()) }
    pub fn nil() -> Self { Self(Uuid::nil()) }
}
```

## 4. PairKindBitset

`crates/context-graph-core/src/dynamicjepa/pair_kinds.rs` (NEW)

```rust
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct PairKindBitset(pub u8);

impl PairKindBitset {
    pub const COSINE_AGREEMENT: u8       = 0b0000_0001;
    pub const RANK_DISAGREEMENT: u8      = 0b0000_0010;
    pub const MODALITY_CONTRADICTION: u8 = 0b0000_0100;
    pub const SPARSE_DENSE_MISMATCH: u8  = 0b0000_1000;
    pub const TEMPORAL_SURPRISE: u8      = 0b0001_0000;
    pub const CAUSAL_DIRECTION: u8       = 0b0010_0000;
    pub const SAFETY_PROXIMITY: u8       = 0b0100_0000;

    pub const ALL_KIND_BITS: u8 = 0b0111_1111;

    /// Iterate the kind names that are set in this bitset.
    pub fn iter_kinds(&self) -> impl Iterator<Item = &'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        if self.has(Self::COSINE_AGREEMENT)       { out.push("cosine_agreement"); }
        if self.has(Self::RANK_DISAGREEMENT)      { out.push("rank_disagreement"); }
        if self.has(Self::MODALITY_CONTRADICTION) { out.push("modality_contradiction"); }
        if self.has(Self::SPARSE_DENSE_MISMATCH)  { out.push("sparse_dense_mismatch"); }
        if self.has(Self::TEMPORAL_SURPRISE)      { out.push("temporal_surprise"); }
        if self.has(Self::CAUSAL_DIRECTION)       { out.push("causal_direction_disagreement"); }
        if self.has(Self::SAFETY_PROXIMITY)       { out.push("safety_proximity"); }
        out.into_iter()
    }

    pub fn has(self, kind: u8) -> bool { (self.0 & kind) != 0 }
    pub fn set(&mut self, kind: u8) { self.0 |= kind; }
}
```

## 5. Audit-log extension

`crates/context-graph-core/src/dynamicjepa/audit_log.rs`

Add the `signal_yield` field (with `#[serde(default)]` so v1
deserialisers see 0):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DjAuditRecord {
    pub audit_id: AuditId,
    pub operation: String,
    pub status: AuditStatus,
    pub created_at_unix_ms: u64,
    pub before_counts: Option<BTreeMap<String, u64>>,
    pub after_counts: Option<BTreeMap<String, u64>>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,

    // ... other existing fields ...

    /// Number of structured supervisory signals this operation produced.
    /// 0 for operations that don't produce signals (failures,
    /// pure-verification ops). Non-zero for ingest, panel materialisation,
    /// pairwise materialisation, prediction, planning, surprise, etc.
    /// Filled in at write time from the lookup table in
    /// signal_yield_for_operation(); FSV-12 verifies aggregate consistency.
    #[serde(default)]
    pub signal_yield: u32,
}
```

The `#[serde(default)]` ensures v1 audit-log JSON without
`signal_yield` deserialises with `signal_yield = 0`. v2 writers
always populate it.

## 6. Domain pack info helper

`crates/context-graph-core/src/dynamicjepa/types.rs`

```rust
/// Per-domain configuration consumed by signal-yield calculation
/// and other derived metrics. Read from the registered domain pack.
#[derive(Debug, Clone)]
pub struct DomainPackInfo {
    /// Number of declared instrument slots in the domain panel (= N_d).
    pub n_panel_slots: usize,

    /// Number of modalities active in the runtime guard for this domain.
    /// Defaults to n_panel_slots; may be smaller if the domain pack
    /// declares a reduced guard set.
    pub n_active_modalities: usize,

    /// Number of candidate actions enumerated by the planner per plan.
    pub k_candidates: usize,
}

impl DomainPackInfo {
    pub fn from_domain_pack(domain: &DomainPack) -> Self {
        Self {
            n_panel_slots: domain.declared_instruments.len(),
            n_active_modalities: domain
                .constellation_global
                .as_ref()
                .map(|c| c.modalities.len())
                .unwrap_or(domain.declared_instruments.len()),
            k_candidates: domain.candidate_action_count,
        }
    }
}
```

## 7. Domain pack TOML extension

`crates/context-graph-core/src/dynamicjepa/domain_pack.rs`

Extend the `DomainPack` struct with optional v2 fields:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct DomainPack {
    // ... existing fields ...

    #[serde(default)]
    pub schema_version: u32,            // 1 or 2

    #[serde(default)]
    pub constellation_global: Option<ConstellationDeclaration>,

    #[serde(default)]
    pub constellation_subjects: Vec<ConstellationSubjectDeclaration>,

    pub declared_instruments: Vec<InstrumentDeclaration>,
    pub candidate_action_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct InstrumentDeclaration {
    pub id: InstrumentId,
    pub name: String,
    pub kind: InstrumentKind,
    pub output_dim: usize,

    /// Pair-kinds this instrument participates in. Default empty;
    /// validator rejects domain packs whose first instrument doesn't
    /// declare "cosine".
    #[serde(default)]
    pub pair_kinds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ConstellationDeclaration {
    pub modalities: ConstellationModalities,
    pub require_loo_min: f32,
    pub calibration_percentile: u8,
    pub calibration_set_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstellationModalities {
    All,
    List(Vec<InstrumentId>),
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ConstellationSubjectDeclaration {
    pub id: String,
    pub field: String,                  // must match a [[state.fields]] name
    pub value: String,                  // must match the field's variants

    pub modalities: ConstellationModalities,
    pub require_loo_min: f32,

    #[serde(default = "default_calibration_percentile")]
    pub calibration_percentile: u8,

    #[serde(default = "default_calibration_set_size")]
    pub calibration_set_size: u32,
}

fn default_calibration_percentile() -> u8 { 10 }
fn default_calibration_set_size() -> u32 { 50 }
```

## 8. Domain pack validator

`crates/context-graph-core/src/dynamicjepa/domain_pack_validator.rs`

Adds validation rules:

```rust
pub fn validate_domain_pack_v2(pack: &DomainPack) -> Result<(), DomainPackError> {
    if pack.schema_version != 2 {
        return Err(DomainPackError::UnsupportedSchemaVersion(pack.schema_version));
    }

    // Every declared instrument must declare at least the "cosine" pair-kind
    // (or the constellation-related machinery has nothing to fall back on).
    for inst in &pack.declared_instruments {
        if !inst.pair_kinds.contains(&"cosine".to_string()) {
            return Err(DomainPackError::CosineKindMissing(inst.name.clone()));
        }
    }

    // Each constellation subject's `field` must reference a declared state field.
    for subj in &pack.constellation_subjects {
        let field_decl = pack.state_fields().iter()
            .find(|f| f.name == subj.field)
            .ok_or_else(|| DomainPackError::ConstellationSubjectFieldUndeclared(
                subj.field.clone()
            ))?;

        // The value must be one of the field's declared variants if categorical
        if let FieldKind::Categorical { variants } = &field_decl.kind {
            if !variants.contains(&subj.value) {
                return Err(DomainPackError::ConstellationSubjectValueUndeclared(
                    subj.field.clone(),
                    subj.value.clone(),
                ));
            }
        }
    }

    // Default modality lookups
    if let Some(global) = &pack.constellation_global {
        validate_modalities(&global.modalities, pack)?;
    }
    for subj in &pack.constellation_subjects {
        validate_modalities(&subj.modalities, pack)?;
    }

    Ok(())
}
```

## 9. Re-export discipline

`crates/context-graph-core/src/dynamicjepa/mod.rs` re-exports the
new public surface:

```rust
pub mod ids;
pub mod pair_kinds;
pub mod records;
pub mod audit_log;
pub mod types;
pub mod domain_pack;
pub mod domain_pack_validator;

pub use ids::{
    PairwiseReadingId, ConstellationId, ThresholdCalibrationId,
};
pub use pair_kinds::PairKindBitset;
pub use records::pairwise::PairwiseReading;
pub use records::constellation::ConstellationCentroid;
pub use records::threshold::ThresholdCalibration;
pub use audit_log::signal_yield_for_operation;
pub use types::DomainPackInfo;
pub use domain_pack_validator::validate_domain_pack_v2;
```

The existing public exports remain unchanged.

## 10. Test additions per file

| File | Tests | Coverage target |
| --- | ---: | ---: |
| `records/pairwise.rs` | 8 | ≥ 90% |
| `records/constellation.rs` | 8 | ≥ 90% |
| `records/threshold.rs` | 8 | ≥ 90% |
| `pair_kinds.rs` | 6 | ≥ 95% |
| `domain_pack_validator.rs` | 12 | ≥ 95% (validators must be exhaustive) |
| `audit_log.rs` | extends existing 5 with 3 new | unchanged |

## 11. Migration impact

PR 2 is purely additive. Adding optional fields with
`#[serde(default)]` doesn't break existing v1 records; v1
deserialisers see the new field as default. The codebase's existing
v1 test suite continues to pass without modification.

Compilation impact: the `Validate` derive macro on the new structs
adds a small build-time cost; trivially under 1 second.
