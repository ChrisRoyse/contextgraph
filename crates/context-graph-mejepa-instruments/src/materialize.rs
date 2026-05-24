use crate::{InstrumentError, InstrumentResult, InstrumentSlot, Panel, PanelBuilder};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeStep {
    T0,
    T1,
    T2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelVectorInput {
    pub slot: InstrumentSlot,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializedPanel {
    pub time_step: TimeStep,
    pub panel: Panel,
    pub active_slots: Vec<InstrumentSlot>,
    pub zero_slots: Vec<InstrumentSlot>,
}

pub fn materialize_panel(
    time_step: TimeStep,
    inputs: Vec<PanelVectorInput>,
) -> InstrumentResult<MaterializedPanel> {
    if inputs.is_empty() {
        return Err(InstrumentError::invalid(
            "materialize_panel.inputs",
            "no instrument vectors were supplied",
            "supply the required vectors for the requested time step",
        ));
    }

    let mut builder = PanelBuilder::new();
    let mut seen = BTreeSet::new();
    for input in inputs {
        if !seen.insert(input.slot) {
            return Err(InstrumentError::invalid(
                "materialize_panel.inputs",
                format!("duplicate vector for slot {:?}", input.slot),
                "materialize each slot once and only once",
            ));
        }
        // #801 / #710: production panel-build uses the strict-constant
        // health check so embedder collapse (#666 E2/E3/E4 / #704) cannot
        // pass through unchecked. Explicit zero-fills below use the
        // permissive `set_slot` because zero is a legitimate value for
        // the explicitly-declared zero_slots(time_step).
        builder.set_slot_with_health_check(input.slot, &input.vector)?;
    }

    let active_slots = active_slots(time_step);
    let zero_slots = zero_slots(time_step);
    for slot in &active_slots {
        if !seen.contains(slot) {
            return Err(InstrumentError::invalid(
                "materialize_panel.inputs",
                format!("missing required {:?} vector for {:?}", slot, time_step),
                "run the source instrument and pass its vector instead of zero-filling required data",
            ));
        }
    }
    for slot in &zero_slots {
        if seen.contains(slot) {
            return Err(InstrumentError::invalid(
                "materialize_panel.inputs",
                format!(
                    "{:?} must be an explicit zero slot at {:?}",
                    slot, time_step
                ),
                "do not pass post-step evidence into an earlier panel time step",
            ));
        }
        builder.set_slot(*slot, &vec![0.0; slot.dim()])?;
    }
    for slot in seen {
        if !active_slots.contains(&slot) {
            return Err(InstrumentError::invalid(
                "materialize_panel.inputs",
                format!(
                    "{:?} is not allowed as active evidence at {:?}",
                    slot, time_step
                ),
                "route this evidence to the correct panel time step",
            ));
        }
    }
    let panel = builder.build()?;
    Ok(MaterializedPanel {
        time_step,
        panel,
        active_slots,
        zero_slots,
    })
}

pub fn active_slots(time_step: TimeStep) -> Vec<InstrumentSlot> {
    use InstrumentSlot::*;
    match time_step {
        TimeStep::T0 => vec![EAst, ECfg, EDataFlow, ETypeGraph, ETest, EProblem],
        TimeStep::T1 => vec![
            EAst, ECfg, EDataFlow, ETypeGraph, ETest, EDiff, EWitness, EProblem, ECommitMsg,
        ],
        TimeStep::T2 => InstrumentSlot::all().to_vec(),
    }
}

pub fn zero_slots(time_step: TimeStep) -> Vec<InstrumentSlot> {
    use InstrumentSlot::*;
    match time_step {
        TimeStep::T0 => vec![EDiff, ETrace, EWitness, EOracle, ECommitMsg],
        TimeStep::T1 => vec![ETrace, EOracle],
        TimeStep::T2 => vec![],
    }
}

pub fn vectors_from_panel(panel: &Panel, slots: &[InstrumentSlot]) -> Vec<PanelVectorInput> {
    slots
        .iter()
        .map(|slot| PanelVectorInput {
            slot: *slot,
            vector: panel.slot(*slot).to_vec(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(slot: InstrumentSlot) -> PanelVectorInput {
        // #801 / #710: avoid strict-constant vectors so the production
        // health check in `materialize_panel` accepts the test fixture.
        // The exact pattern is unimportant — only that no slot vector is
        // entirely identical across all dimensions.
        let vector: Vec<f32> = (0..slot.dim())
            .map(|i| 1.0 + (i as f32) * 0.001)
            .collect();
        PanelVectorInput { slot, vector }
    }

    #[test]
    fn materialize_t0_fills_required_and_zero_slots() {
        let panel = materialize_panel(
            TimeStep::T0,
            active_slots(TimeStep::T0).into_iter().map(input).collect(),
        )
        .unwrap();
        assert!(panel.panel.is_filled(InstrumentSlot::EAst));
        assert!(panel.panel.is_filled(InstrumentSlot::EOracle));
        assert!(panel
            .panel
            .slot(InstrumentSlot::EOracle)
            .iter()
            .all(|v| *v == 0.0));
    }

    #[test]
    fn materialize_fails_on_missing_or_future_slot() {
        let err = materialize_panel(TimeStep::T0, vec![input(InstrumentSlot::EAst)]).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");

        let mut inputs: Vec<_> = active_slots(TimeStep::T0).into_iter().map(input).collect();
        inputs.push(input(InstrumentSlot::EOracle));
        let err = materialize_panel(TimeStep::T0, inputs).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
    }
}
