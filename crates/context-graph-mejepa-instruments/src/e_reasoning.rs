use serde::{Deserialize, Serialize};

use crate::features::{
    add_hashed_pair, add_hashed_token_features, bounded_ratio, normalize_l2,
    validate_finite_output, validate_single_line, validate_text_field,
};
use crate::{Instrument, InstrumentError, InstrumentResult, InstrumentSlot};

const MAX_REASONING_EVENTS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReasoningEvent {
    pub actor: String,
    pub event_type: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReasoningInput {
    pub task_id: String,
    pub transcript: String,
    pub events: Vec<ReasoningEvent>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EReasoningInstrument;

impl Instrument for EReasoningInstrument {
    type Input = ReasoningInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::EReasoning
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        validate_reasoning(input)?;
        let mut out = vec![0.0_f32; InstrumentSlot::EReasoning.dim()];
        add_hashed_token_features(&mut out, &input.transcript, 1.0);
        add_hashed_pair(&mut out, &input.task_id, 2.0, 32, 64);
        let denom = input.events.len().max(1) as f32;
        for event in &input.events {
            add_hashed_pair(&mut out, &event.actor, 1.0 / denom, 96, 64);
            add_hashed_pair(&mut out, &event.event_type, 1.0 / denom, 160, 64);
            add_hashed_token_features(&mut out[224..], &event.text, 0.25 / denom);
        }
        out[0] = bounded_ratio(input.transcript.len() as f32, 1_000_000.0);
        out[1] = bounded_ratio(input.events.len() as f32, 10_000.0);
        out[2] = if input.transcript.contains("error") || input.transcript.contains("failed") {
            1.0
        } else {
            0.0
        };
        normalize_l2(&mut out);
        validate_finite_output("e_reasoning.output", &out)?;
        Ok(out)
    }
}

fn validate_reasoning(input: &ReasoningInput) -> InstrumentResult<()> {
    validate_single_line(
        "input.task_id",
        &input.task_id,
        "store the task id as a stable single-line identifier",
    )?;
    validate_text_field(
        "input.transcript",
        &input.transcript,
        "capture a real agent/reviewer reasoning transcript before encoding E_Reasoning",
    )?;
    if input.events.len() > MAX_REASONING_EVENTS {
        return Err(InstrumentError::invalid(
            "input.events",
            format!(
                "reasoning input contains {} events; max supported events is {MAX_REASONING_EVENTS}",
                input.events.len()
            ),
            "aggregate or shard long reasoning transcripts before encoding",
        ));
    }
    for (idx, event) in input.events.iter().enumerate() {
        validate_single_line(
            "input.events[i].actor",
            &event.actor,
            "store reasoning event actors as single-line identifiers",
        )
        .map_err(|err| annotate_idx(err, idx))?;
        validate_single_line(
            "input.events[i].event_type",
            &event.event_type,
            "store reasoning event types as single-line identifiers",
        )
        .map_err(|err| annotate_idx(err, idx))?;
        validate_text_field(
            "input.events[i].text",
            &event.text,
            "store the real event text before encoding",
        )
        .map_err(|err| annotate_idx(err, idx))?;
    }
    Ok(())
}

fn annotate_idx(err: InstrumentError, idx: usize) -> InstrumentError {
    InstrumentError::invalid(
        "input.events[i]",
        format!("reasoning event {idx} invalid: {err}"),
        "fix the reasoning event source-of-truth before encoding",
    )
}
