use serde::{Deserialize, Serialize};

use crate::features::{
    add_hashed_pair, add_hashed_token_features, bounded_ratio, normalize_l2,
    validate_finite_output, validate_single_line, validate_text_field,
};
use crate::{Instrument, InstrumentError, InstrumentResult, InstrumentSlot};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextInstrumentInput {
    pub text: String,
    pub source_id: String,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextKind {
    Test,
    Problem,
    CommitMsg,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ETestInstrument;

#[derive(Debug, Clone, Copy, Default)]
pub struct EProblemInstrument;

#[derive(Debug, Clone, Copy, Default)]
pub struct ECommitMsgInstrument;

impl Instrument for ETestInstrument {
    type Input = TextInstrumentInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::ETest
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        encode_text(input, InstrumentSlot::ETest, TextKind::Test)
    }
}

impl Instrument for EProblemInstrument {
    type Input = TextInstrumentInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::EProblem
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        encode_text(input, InstrumentSlot::EProblem, TextKind::Problem)
    }
}

impl Instrument for ECommitMsgInstrument {
    type Input = TextInstrumentInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::ECommitMsg
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        encode_text(input, InstrumentSlot::ECommitMsg, TextKind::CommitMsg)
    }
}

fn encode_text(
    input: &TextInstrumentInput,
    slot: InstrumentSlot,
    kind: TextKind,
) -> InstrumentResult<Vec<f32>> {
    validate_text_input(input, kind)?;
    let dim = slot.dim();
    let mut out = vec![0.0_f32; dim];
    let text = input.text.as_str();
    let token_count = text
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .filter(|token| !token.is_empty())
        .count();
    add_hashed_token_features(&mut out, text, 1.0);
    add_hashed_token_features(&mut out, &input.source_id, 0.5);
    if let Some(language) = &input.language {
        add_hashed_pair(&mut out, language, 2.0, dim.saturating_sub(64), 32);
    }

    out[0] = bounded_ratio(text.len() as f32, 200_000.0);
    out[1] = bounded_ratio(token_count as f32, 20_000.0);
    out[2] = bounded_ratio(text.lines().count() as f32, 10_000.0);
    out[3] = if text.contains("assert") { 1.0 } else { 0.0 };
    out[4] = if text.contains("Traceback") || text.contains("Error") {
        1.0
    } else {
        0.0
    };
    out[5] = match kind {
        TextKind::Test => 1.0,
        TextKind::Problem => 2.0,
        TextKind::CommitMsg => 3.0,
    };
    normalize_l2(&mut out);
    validate_finite_output("e_text.output", &out)?;
    Ok(out)
}

fn validate_text_input(input: &TextInstrumentInput, kind: TextKind) -> InstrumentResult<()> {
    validate_text_field(
        "input.text",
        &input.text,
        "capture the real text source before encoding this ME-JEPA text instrument",
    )?;
    validate_single_line(
        "input.source_id",
        &input.source_id,
        "store a stable task/test/commit source id as one line of UTF-8 text",
    )?;
    if let Some(language) = &input.language {
        validate_single_line(
            "input.language",
            language,
            "store language identifiers as single-line canonical slugs",
        )?;
    }
    if kind == TextKind::CommitMsg && input.text.lines().count() > 200 {
        return Err(InstrumentError::invalid(
            "input.text",
            "commit-message text is implausibly large (>200 lines)",
            "pass the commit message or candidate-fix summary, not an entire transcript",
        ));
    }
    Ok(())
}
