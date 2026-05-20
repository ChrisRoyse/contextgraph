use serde::{Deserialize, Serialize};

use crate::features::{
    add_hashed_pair, bounded_ratio, normalize_l2, validate_finite_output, validate_single_line,
};
use crate::{Instrument, InstrumentError, InstrumentResult, InstrumentSlot};

const MAX_TRACE_EVENTS: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceEventKind {
    FunctionEnter,
    FunctionExit,
    LineHit,
    ValueSnapshot,
    Exception,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceEvent {
    pub function_id: String,
    pub line: u32,
    pub value_hash: String,
    pub timestamp_ms: u64,
    pub kind: TraceEventKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceInput {
    pub events: Vec<TraceEvent>,
    /// True when runtime trace evidence is intentionally unavailable.
    /// This is not an observed ValueSnapshot/Exception event.
    #[serde(default)]
    pub evidence_unavailable: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ETraceInstrument;

impl Instrument for ETraceInstrument {
    type Input = TraceInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::ETrace
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        validate_trace(input)?;
        let mut out = vec![0.0_f32; InstrumentSlot::ETrace.dim()];
        if input.evidence_unavailable {
            out[16] = 1.0;
            validate_finite_output("e_trace.output", &out)?;
            return Ok(out);
        }
        let denom = input.events.len() as f32;
        let mut last_ts = 0u64;
        for event in &input.events {
            let kind_idx = match event.kind {
                TraceEventKind::FunctionEnter => 0,
                TraceEventKind::FunctionExit => 1,
                TraceEventKind::LineHit => 2,
                TraceEventKind::ValueSnapshot => 3,
                TraceEventKind::Exception => 4,
            };
            out[kind_idx] += 1.0 / denom;
            out[8] += bounded_ratio(event.line as f32, 1_000_000.0) / denom;
            out[9] += bounded_ratio(event.timestamp_ms as f32, 86_400_000.0) / denom;
            add_hashed_pair(&mut out, &event.function_id, 1.0 / denom, 32, 192);
            add_hashed_pair(&mut out, &event.value_hash, 1.0 / denom, 224, 256);
            if event.timestamp_ms >= last_ts {
                out[10] += 1.0 / denom;
            }
            last_ts = event.timestamp_ms;
        }
        out[11] = bounded_ratio(input.events.len() as f32, 100_000.0);
        normalize_l2(&mut out);
        validate_finite_output("e_trace.output", &out)?;
        Ok(out)
    }
}

fn validate_trace(input: &TraceInput) -> InstrumentResult<()> {
    if input.evidence_unavailable {
        if !input.events.is_empty() {
            return Err(InstrumentError::invalid(
                "input.evidence_unavailable",
                "unavailable trace evidence cannot include observed trace events",
                "set evidence_unavailable=false before recording runtime trace events",
            ));
        }
        return Ok(());
    }
    if input.events.is_empty() {
        return Err(InstrumentError::invalid(
            "input.events",
            "trace has no events",
            "capture the real runtime trace before filling E_Trace",
        ));
    }
    if input.events.len() > MAX_TRACE_EVENTS {
        return Err(InstrumentError::invalid(
            "input.events",
            format!(
                "trace contains {} events; max supported events is {MAX_TRACE_EVENTS}",
                input.events.len()
            ),
            "aggregate or shard very large traces before encoding E_Trace",
        ));
    }
    for (idx, event) in input.events.iter().enumerate() {
        validate_single_line(
            "input.events[i].function_id",
            &event.function_id,
            "store trace function ids as single-line stable identifiers",
        )
        .map_err(|err| annotate_idx(err, idx))?;
        validate_single_line(
            "input.events[i].value_hash",
            &event.value_hash,
            "store trace value hashes as single-line digest strings",
        )
        .map_err(|err| annotate_idx(err, idx))?;
        if event.line == 0 {
            return Err(InstrumentError::invalid(
                "input.events[i].line",
                format!("trace event {idx} has line 0"),
                "store one-based runtime line numbers",
            ));
        }
    }
    Ok(())
}

fn annotate_idx(err: InstrumentError, idx: usize) -> InstrumentError {
    InstrumentError::invalid(
        "input.events[i]",
        format!("trace event {idx} invalid: {err}"),
        "fix the trace event source-of-truth before encoding",
    )
}
