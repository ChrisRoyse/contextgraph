use std::collections::{BTreeMap, BTreeSet};

use context_graph_mejepa_instruments::{hash_f32s, InstrumentSlot, Panel, PANEL_DIM};
use serde::{Deserialize, Serialize};

use crate::error::MejepaInferError;

pub const CONSTELLATION_OBSERVATION_SCHEMA_VERSION: u32 = 1;
pub const CONSTELLATION_PACKED_PANEL_LAYOUT: &str = "instrument_panel_5120_slot_map_v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConstellationPanelId(pub String);

impl ConstellationPanelId {
    pub fn validate(&self, field: &str) -> Result<(), MejepaInferError> {
        validate_id(field, &self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VectorSliceRef {
    pub layout: String,
    pub offset: usize,
    pub len: usize,
    pub vector_hash: String,
}

impl VectorSliceRef {
    fn validate(&self, field: &str, slot: InstrumentSlot) -> Result<(), MejepaInferError> {
        let (offset, len) = slot.extent();
        if self.layout != CONSTELLATION_PACKED_PANEL_LAYOUT {
            return invalid(
                &format!("{field}.layout"),
                format!(
                    "expected {CONSTELLATION_PACKED_PANEL_LAYOUT}, got {}",
                    self.layout
                ),
            );
        }
        if self.offset != offset || self.len != len {
            return Err(MejepaInferError::DimMismatch {
                expected: len,
                actual: self.len,
                context: format!(
                    "{field} must point at {} offset={offset} len={len}; got offset={} len={}",
                    slot.slug(),
                    self.offset,
                    self.len
                ),
            });
        }
        validate_sha256(&format!("{field}.vector_hash"), &self.vector_hash)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotProvenance {
    pub instrument_id: String,
    pub model_version_hash: String,
    pub source_evidence_id: String,
    pub calibration_cell: String,
}

impl SlotProvenance {
    pub fn validate(&self, field: &str) -> Result<(), MejepaInferError> {
        validate_id(&format!("{field}.instrument_id"), &self.instrument_id)?;
        validate_id(
            &format!("{field}.model_version_hash"),
            &self.model_version_hash,
        )?;
        validate_id(
            &format!("{field}.source_evidence_id"),
            &self.source_evidence_id,
        )?;
        validate_id(&format!("{field}.calibration_cell"), &self.calibration_cell)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstellationSlotObservation {
    pub slot_id: String,
    pub instrument_id: String,
    pub dim: usize,
    pub offset: usize,
    pub filled: bool,
    pub model_version_hash: Option<String>,
    pub source_evidence_id: Option<String>,
    pub calibration_cell: String,
    pub vector_hash: Option<String>,
    pub vector_slice: Option<VectorSliceRef>,
}

impl ConstellationSlotObservation {
    pub fn validate(&self, field: &str) -> Result<InstrumentSlot, MejepaInferError> {
        let slot = slot_from_slug(&self.slot_id).ok_or_else(|| MejepaInferError::InvalidInput {
            field: format!("{field}.slot_id"),
            detail: format!("unknown slot_id {}", self.slot_id),
        })?;
        let (offset, dim) = slot.extent();
        validate_id(&format!("{field}.instrument_id"), &self.instrument_id)?;
        validate_id(&format!("{field}.calibration_cell"), &self.calibration_cell)?;
        if self.dim != dim || self.offset != offset {
            return Err(MejepaInferError::DimMismatch {
                expected: dim,
                actual: self.dim,
                context: format!(
                    "{field} must match {} offset={offset} dim={dim}; got offset={} dim={}",
                    slot.slug(),
                    self.offset,
                    self.dim
                ),
            });
        }
        match self.filled {
            true => self.validate_filled(field, slot)?,
            false => self.validate_missing(field)?,
        }
        Ok(slot)
    }

    fn validate_filled(&self, field: &str, slot: InstrumentSlot) -> Result<(), MejepaInferError> {
        validate_present_id(
            &format!("{field}.model_version_hash"),
            self.model_version_hash.as_deref(),
        )?;
        validate_present_id(
            &format!("{field}.source_evidence_id"),
            self.source_evidence_id.as_deref(),
        )?;
        let vector_hash =
            validate_present_id(&format!("{field}.vector_hash"), self.vector_hash.as_deref())?;
        validate_sha256(&format!("{field}.vector_hash"), vector_hash)?;
        let slice = self
            .vector_slice
            .as_ref()
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: format!("{field}.vector_slice"),
                detail: "filled slot must include vector_slice".to_string(),
            })?;
        slice.validate(&format!("{field}.vector_slice"), slot)?;
        if slice.vector_hash != vector_hash {
            return invalid(
                &format!("{field}.vector_slice.vector_hash"),
                "must match slot vector_hash".to_string(),
            );
        }
        Ok(())
    }

    fn validate_missing(&self, field: &str) -> Result<(), MejepaInferError> {
        if self.model_version_hash.is_some()
            || self.source_evidence_id.is_some()
            || self.vector_hash.is_some()
            || self.vector_slice.is_some()
        {
            return invalid(
                field,
                "missing slot must not claim source/model/vector metadata".to_string(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairwiseRelationshipObservation {
    pub left_slot_id: String,
    pub right_slot_id: String,
    pub relationship_kind: String,
    pub correlation: Option<f32>,
    pub mutual_information: Option<f32>,
    pub source_evidence_id: String,
}

impl PairwiseRelationshipObservation {
    pub fn validate(&self, field: &str) -> Result<(String, String), MejepaInferError> {
        let left =
            slot_from_slug(&self.left_slot_id).ok_or_else(|| MejepaInferError::InvalidInput {
                field: format!("{field}.left_slot_id"),
                detail: format!("unknown slot_id {}", self.left_slot_id),
            })?;
        let right =
            slot_from_slug(&self.right_slot_id).ok_or_else(|| MejepaInferError::InvalidInput {
                field: format!("{field}.right_slot_id"),
                detail: format!("unknown slot_id {}", self.right_slot_id),
            })?;
        if left == right {
            return invalid(
                field,
                "pairwise relationship must use two distinct slots".to_string(),
            );
        }
        validate_id(
            &format!("{field}.relationship_kind"),
            &self.relationship_kind,
        )?;
        validate_id(
            &format!("{field}.source_evidence_id"),
            &self.source_evidence_id,
        )?;
        if let Some(value) = self.correlation {
            if !value.is_finite() || !(-1.0..=1.0).contains(&value) {
                return invalid(
                    &format!("{field}.correlation"),
                    format!("correlation must be finite and in [-1, 1]; got {value}"),
                );
            }
        }
        if let Some(value) = self.mutual_information {
            if !value.is_finite() || value < 0.0 {
                return invalid(
                    &format!("{field}.mutual_information"),
                    format!("mutual_information must be finite and >= 0; got {value}"),
                );
            }
        }
        let mut pair = [left.slug().to_string(), right.slug().to_string()];
        pair.sort();
        Ok((pair[0].clone(), pair[1].clone()))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstellationObservation {
    pub schema_version: u32,
    pub panel_id: ConstellationPanelId,
    pub chunk_id: String,
    pub calibration_cell: String,
    pub slots: Vec<ConstellationSlotObservation>,
    pub pairwise_relationships: Vec<PairwiseRelationshipObservation>,
}

impl ConstellationObservation {
    pub fn try_new(
        panel_id: ConstellationPanelId,
        chunk_id: String,
        calibration_cell: String,
        slots: Vec<ConstellationSlotObservation>,
        pairwise_relationships: Vec<PairwiseRelationshipObservation>,
    ) -> Result<Self, MejepaInferError> {
        let observation = Self {
            schema_version: CONSTELLATION_OBSERVATION_SCHEMA_VERSION,
            panel_id,
            chunk_id,
            calibration_cell,
            slots,
            pairwise_relationships,
        };
        observation.validate()?;
        Ok(observation)
    }

    pub fn from_panel(
        panel_id: ConstellationPanelId,
        chunk_id: String,
        calibration_cell: String,
        panel: &Panel,
        provenance: &BTreeMap<InstrumentSlot, SlotProvenance>,
        pairwise_relationships: Vec<PairwiseRelationshipObservation>,
    ) -> Result<Self, MejepaInferError> {
        let mut slots = Vec::with_capacity(InstrumentSlot::all().len());
        for slot in InstrumentSlot::all() {
            let (offset, dim) = slot.extent();
            let filled = panel.is_filled(slot);
            let slot_provenance = provenance.get(&slot);
            if filled && slot_provenance.is_none() {
                return invalid(
                    "provenance",
                    format!("filled slot {} lacks provenance", slot.slug()),
                );
            }
            if !filled && slot_provenance.is_some() {
                return invalid(
                    "provenance",
                    format!("missing slot {} has provenance", slot.slug()),
                );
            }
            let vector_hash = filled.then(|| hash_f32s(panel.slot(slot)));
            let vector_slice = vector_hash.as_ref().map(|hash| VectorSliceRef {
                layout: CONSTELLATION_PACKED_PANEL_LAYOUT.to_string(),
                offset,
                len: dim,
                vector_hash: hash.clone(),
            });
            let (instrument_id, model_version_hash, source_evidence_id) =
                if let Some(source) = slot_provenance {
                    source.validate(&format!("provenance.{}", slot.slug()))?;
                    if source.calibration_cell != calibration_cell {
                        return invalid(
                            &format!("provenance.{}.calibration_cell", slot.slug()),
                            format!(
                                "must match observation calibration_cell {}; got {}",
                                calibration_cell, source.calibration_cell
                            ),
                        );
                    }
                    (
                        source.instrument_id.clone(),
                        Some(source.model_version_hash.clone()),
                        Some(source.source_evidence_id.clone()),
                    )
                } else {
                    (format!("instrument:{}", slot.slug()), None, None)
                };
            slots.push(ConstellationSlotObservation {
                slot_id: slot.slug().to_string(),
                instrument_id,
                dim,
                offset,
                filled,
                model_version_hash,
                source_evidence_id,
                calibration_cell: calibration_cell.clone(),
                vector_hash,
                vector_slice,
            });
        }
        Self::try_new(
            panel_id,
            chunk_id,
            calibration_cell,
            slots,
            pairwise_relationships,
        )
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != CONSTELLATION_OBSERVATION_SCHEMA_VERSION {
            return invalid(
                "schema_version",
                format!(
                    "expected {CONSTELLATION_OBSERVATION_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        self.panel_id.validate("panel_id")?;
        validate_id("chunk_id", &self.chunk_id)?;
        validate_id("calibration_cell", &self.calibration_cell)?;
        if self.slots.len() != InstrumentSlot::all().len() {
            return Err(MejepaInferError::DimMismatch {
                expected: InstrumentSlot::all().len(),
                actual: self.slots.len(),
                context: "constellation observation must enumerate every packed panel slot"
                    .to_string(),
            });
        }
        let mut seen = BTreeSet::new();
        for (idx, slot) in self.slots.iter().enumerate() {
            let parsed =
                slot_from_slug(&slot.slot_id).ok_or_else(|| MejepaInferError::InvalidInput {
                    field: format!("slots[{idx}].slot_id"),
                    detail: format!("unknown slot_id {}", slot.slot_id),
                })?;
            if !seen.insert(parsed) {
                return invalid(
                    &format!("slots[{idx}].slot_id"),
                    format!("duplicate slot {}", parsed.slug()),
                );
            }
            slot.validate(&format!("slots[{idx}]"))?;
            if slot.calibration_cell != self.calibration_cell {
                return invalid(
                    &format!("slots[{idx}].calibration_cell"),
                    format!(
                        "must match observation calibration_cell {}; got {}",
                        self.calibration_cell, slot.calibration_cell
                    ),
                );
            }
        }
        for expected in InstrumentSlot::all() {
            if !seen.contains(&expected) {
                return invalid("slots", format!("missing slot {}", expected.slug()));
            }
        }
        let mut pairs = BTreeSet::new();
        for (idx, pair) in self.pairwise_relationships.iter().enumerate() {
            let key = pair.validate(&format!("pairwise_relationships[{idx}]"))?;
            if !pairs.insert(key.clone()) {
                return invalid(
                    &format!("pairwise_relationships[{idx}]"),
                    format!("duplicate pair {}|{}", key.0, key.1),
                );
            }
        }
        Ok(())
    }

    pub fn filled_slot_count(&self) -> usize {
        self.slots.iter().filter(|slot| slot.filled).count()
    }

    pub fn missing_slot_ids(&self) -> Vec<String> {
        self.slots
            .iter()
            .filter(|slot| !slot.filled)
            .map(|slot| slot.slot_id.clone())
            .collect()
    }

    pub fn slot_hashes(&self) -> BTreeMap<String, String> {
        self.slots
            .iter()
            .filter_map(|slot| {
                slot.vector_hash
                    .as_ref()
                    .map(|hash| (slot.slot_id.clone(), hash.clone()))
            })
            .collect()
    }
}

pub fn slot_from_slug(slug: &str) -> Option<InstrumentSlot> {
    InstrumentSlot::all()
        .into_iter()
        .find(|slot| slot.slug() == slug)
}

fn validate_present_id<'a>(
    field: &str,
    value: Option<&'a str>,
) -> Result<&'a str, MejepaInferError> {
    let value = value.ok_or_else(|| MejepaInferError::InvalidInput {
        field: field.to_string(),
        detail: "required field is missing".to_string(),
    })?;
    validate_id(field, value)?;
    Ok(value)
}

fn validate_id(field: &str, value: &str) -> Result<(), MejepaInferError> {
    if value.trim().is_empty() || value.len() > 512 || value.bytes().any(|b| b < 0x20 || b == 0x7f)
    {
        return invalid(
            field,
            "must be non-empty, <=512 bytes, and control-free".to_string(),
        );
    }
    Ok(())
}

fn validate_sha256(field: &str, value: &str) -> Result<(), MejepaInferError> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return invalid(
            field,
            "must be a lowercase 64-byte sha256 hex digest".to_string(),
        );
    }
    if value.bytes().any(|b| b.is_ascii_uppercase()) {
        return invalid(field, "sha256 digest must be lowercase".to_string());
    }
    Ok(())
}

fn invalid<T>(field: &str, detail: String) -> Result<T, MejepaInferError> {
    Err(MejepaInferError::InvalidInput {
        field: field.to_string(),
        detail,
    })
}

pub fn expected_slot_count() -> usize {
    InstrumentSlot::all().len()
}

pub fn expected_panel_dim() -> usize {
    PANEL_DIM
}

#[cfg(test)]
#[path = "constellation_observation_tests.rs"]
mod constellation_observation_tests;
