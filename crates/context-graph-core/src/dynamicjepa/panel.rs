use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::ids::{
    ActionId, EventId, InstrumentId, OutcomeId, PairwiseReadingId, PanelId, StateId,
};
use crate::dynamicjepa::record_header::DjRecordHeader;
use crate::dynamicjepa::validation::{ensure_finite, ensure_no_duplicates, Validate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const LATENT_PANEL_RECORD_VERSION: u8 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LatentPanel {
    pub header: DjRecordHeader,
    pub panel_id: PanelId,
    pub event_id: EventId,
    pub state_id: StateId,
    pub action_id: ActionId,
    pub outcome_id: Option<OutcomeId>,
    pub instrument_reading_ids: Vec<Uuid>,
    #[serde(default)]
    pub pairwise_reading_ids: Vec<PairwiseReadingId>,
    pub ordered_slots: Vec<PanelSlot>,
    pub slot_vectors: Vec<Vec<f32>>,
    pub slot_masks: Vec<bool>,
    pub panel_hash: [u8; 32],
    pub materializer_version: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelSlot {
    pub instrument_id: InstrumentId,
    pub dim: usize,
    pub kind: PanelSlotKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanelSlotKind {
    State,
    Action,
    Outcome,
    Time,
}

impl Validate for PanelSlot {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.instrument_id.validate()?;
        if self.dim == 0 {
            return Err(DynamicJepaError::validation(
                "PanelSlot.dim",
                "slot dim must be positive",
                "declare a strict positive slot shape",
            ));
        }
        Ok(())
    }
}

impl Validate for LatentPanel {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.panel_id.validate()?;
        self.event_id.validate()?;
        self.state_id.validate()?;
        self.action_id.validate()?;
        if let Some(outcome_id) = self.outcome_id {
            outcome_id.validate()?;
        }
        if self.materializer_version == 0 {
            return Err(DynamicJepaError::validation(
                "LatentPanel.materializer_version",
                "materializer_version must be >= 1",
                "set materializer_version=1 for the demo",
            ));
        }
        let expected = self.ordered_slots.len();
        let actual = vec![
            self.ordered_slots.len(),
            self.slot_vectors.len(),
            self.slot_masks.len(),
        ];
        if self.slot_vectors.len() != expected || self.slot_masks.len() != expected {
            return Err(DynamicJepaError::PanelShapeMismatch {
                domain_pack_id: self.header.domain_pack_id.to_string(),
                expected: vec![expected, expected, expected],
                actual,
            });
        }
        if self.instrument_reading_ids.len() != expected {
            return Err(DynamicJepaError::validation(
                "LatentPanel.instrument_reading_ids",
                format!(
                    "reading id count {} does not match slot count {}",
                    self.instrument_reading_ids.len(),
                    expected
                ),
                "write one reading id per panel slot",
            ));
        }
        for id in &self.instrument_reading_ids {
            if id.is_nil() {
                return Err(DynamicJepaError::validation(
                    "LatentPanel.instrument_reading_ids",
                    "reading ids must not be nil",
                    "write persisted InstrumentReading ids",
                ));
            }
        }
        for id in &self.pairwise_reading_ids {
            id.validate()?;
        }
        for slot in &self.ordered_slots {
            slot.validate()?;
        }
        ensure_no_duplicates(
            self.ordered_slots
                .iter()
                .map(|slot| slot.instrument_id.as_str()),
            "LatentPanel.ordered_slots.instrument_id",
        )?;
        for (idx, (slot, vector)) in self
            .ordered_slots
            .iter()
            .zip(self.slot_vectors.iter())
            .enumerate()
        {
            if !self.slot_masks[idx] && vector.is_empty() {
                continue;
            }
            if vector.len() != slot.dim {
                return Err(DynamicJepaError::PanelShapeMismatch {
                    domain_pack_id: self.header.domain_pack_id.to_string(),
                    expected: vec![slot.dim],
                    actual: vec![vector.len()],
                });
            }
            for (value_idx, value) in vector.iter().enumerate() {
                ensure_finite(
                    *value,
                    &format!("LatentPanel.slot_vectors[{idx}][{value_idx}]"),
                )?;
            }
        }
        for (idx, (slot, mask)) in self
            .ordered_slots
            .iter()
            .zip(self.slot_masks.iter())
            .enumerate()
        {
            if !*mask && matches!(slot.kind, PanelSlotKind::State | PanelSlotKind::Action) {
                return Err(DynamicJepaError::validation(
                    format!("LatentPanel.slot_masks[{idx}]"),
                    "state/action required slots cannot be masked absent",
                    "abort panel materialization instead of hiding required instrument failure",
                ));
            }
        }
        if self.panel_hash == [0; 32] {
            return Err(DynamicJepaError::validation(
                "LatentPanel.panel_hash",
                "panel_hash must be computed before persistence",
                "compute the deterministic panel hash from readings and slots",
            ));
        }
        Ok(())
    }
}

crate::impl_dynamic_jepa_record!(LatentPanel, LATENT_PANEL_RECORD_VERSION, "LatentPanel");
