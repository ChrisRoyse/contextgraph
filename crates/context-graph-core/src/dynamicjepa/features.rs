use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::panel::{LatentPanel, PanelSlotKind};

fn ensure_panel_slot_vectors_aligned(panel: &LatentPanel) -> DynamicJepaResult<()> {
    let expected = panel.ordered_slots.len();
    if panel.slot_vectors.len() != expected || panel.slot_masks.len() != expected {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: panel.header.domain_pack_id.to_string(),
            expected: vec![expected, expected, expected],
            actual: vec![
                panel.ordered_slots.len(),
                panel.slot_vectors.len(),
                panel.slot_masks.len(),
            ],
        });
    }
    Ok(())
}

pub fn flatten_panel(panel: &LatentPanel) -> DynamicJepaResult<Vec<f32>> {
    flatten_panel_with_action_override(panel, None)
}

pub fn flatten_panel_with_action_override(
    panel: &LatentPanel,
    action_override: Option<&[f32]>,
) -> DynamicJepaResult<Vec<f32>> {
    ensure_panel_slot_vectors_aligned(panel)?;
    let mut out = Vec::new();
    let mut action_offset = 0usize;
    for (idx, (slot, vector)) in panel
        .ordered_slots
        .iter()
        .zip(panel.slot_vectors.iter())
        .enumerate()
    {
        if !panel.slot_masks[idx] {
            return Err(DynamicJepaError::PanelShapeMismatch {
                domain_pack_id: panel.header.domain_pack_id.to_string(),
                expected: vec![slot.dim],
                actual: vec![0],
            });
        }
        if vector.len() != slot.dim {
            return Err(DynamicJepaError::PanelShapeMismatch {
                domain_pack_id: panel.header.domain_pack_id.to_string(),
                expected: vec![slot.dim],
                actual: vec![vector.len()],
            });
        }
        if matches!(slot.kind, PanelSlotKind::Action) {
            if let Some(action_override) = action_override {
                let next_offset = action_offset + slot.dim;
                let action_slice =
                    action_override
                        .get(action_offset..next_offset)
                        .ok_or_else(|| DynamicJepaError::PanelShapeMismatch {
                            domain_pack_id: panel.header.domain_pack_id.to_string(),
                            expected: vec![next_offset],
                            actual: vec![action_override.len()],
                        })?;
                out.extend_from_slice(action_slice);
                action_offset = next_offset;
                continue;
            }
        }
        out.extend_from_slice(vector);
    }
    if let Some(action_override) = action_override {
        if action_offset != action_override.len() {
            return Err(DynamicJepaError::PanelShapeMismatch {
                domain_pack_id: panel.header.domain_pack_id.to_string(),
                expected: vec![action_offset],
                actual: vec![action_override.len()],
            });
        }
    }
    if out.is_empty() {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: panel.header.domain_pack_id.to_string(),
            expected: vec![1],
            actual: vec![0],
        });
    }
    Ok(out)
}

pub fn panel_action_vector(panel: &LatentPanel) -> DynamicJepaResult<Vec<f32>> {
    ensure_panel_slot_vectors_aligned(panel)?;
    let mut out = Vec::new();
    for (idx, (slot, vector)) in panel
        .ordered_slots
        .iter()
        .zip(panel.slot_vectors.iter())
        .enumerate()
    {
        if matches!(slot.kind, PanelSlotKind::Action) {
            if !panel.slot_masks[idx] || vector.len() != slot.dim {
                return Err(DynamicJepaError::PanelShapeMismatch {
                    domain_pack_id: panel.header.domain_pack_id.to_string(),
                    expected: vec![slot.dim],
                    actual: vec![vector.len()],
                });
            }
            out.extend_from_slice(vector);
        }
    }
    if out.is_empty() {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: panel.header.domain_pack_id.to_string(),
            expected: vec![1],
            actual: vec![0],
        });
    }
    Ok(out)
}
