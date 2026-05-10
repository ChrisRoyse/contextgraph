use context_graph_mejepa_instruments::{InstrumentSlot, Panel};

use crate::error::TctError;
use crate::types::EmbedderId;

pub fn panel_slot_for_embedder(embedder: EmbedderId) -> InstrumentSlot {
    match embedder {
        EmbedderId::E1 => InstrumentSlot::EProblem,
        EmbedderId::E2 => InstrumentSlot::ETrace,
        EmbedderId::E3 => InstrumentSlot::ETrace,
        EmbedderId::E4 => InstrumentSlot::ETrace,
        EmbedderId::E5 => InstrumentSlot::EDataFlow,
        EmbedderId::E6 => InstrumentSlot::EStaticAnalysis,
        EmbedderId::E7 => InstrumentSlot::EAst,
        EmbedderId::E8 => InstrumentSlot::ECfg,
        EmbedderId::E9 => InstrumentSlot::Scalars,
        EmbedderId::E10 => InstrumentSlot::ECommitMsg,
        EmbedderId::E11 => InstrumentSlot::ETypeGraph,
        EmbedderId::E12 => InstrumentSlot::EOracle,
        EmbedderId::E13 => InstrumentSlot::ETest,
        EmbedderId::E14 => InstrumentSlot::EProblem,
        EmbedderId::E15 => InstrumentSlot::EReasoning,
        EmbedderId::E16 => InstrumentSlot::EReasoning,
        EmbedderId::E17 => InstrumentSlot::EReasoning,
        EmbedderId::E18 => InstrumentSlot::ERuntime,
        EmbedderId::E19 => InstrumentSlot::ERuntime,
        EmbedderId::E20 => InstrumentSlot::ETrace,
        EmbedderId::E21 => InstrumentSlot::ETrace,
    }
}

pub fn panel_slice_for_embedder(panel: &Panel, embedder: EmbedderId) -> Result<&[f32], TctError> {
    let slot = panel_slot_for_embedder(embedder);
    if !panel.is_filled(slot) {
        return Err(TctError::MissingCentroid {
            detail: format!(
                "panel slot {} required by embedder {embedder} was not filled",
                slot.slug()
            ),
        });
    }
    let slice = panel.slot(slot);
    for (idx, value) in slice.iter().enumerate() {
        if !value.is_finite() {
            return Err(TctError::nan(
                format!("panel.slot({})", slot.slug()),
                format!("value[{idx}] is {value}"),
            ));
        }
    }
    Ok(slice)
}
