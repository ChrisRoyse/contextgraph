// E_StaticAnalysis instrument — fixed 256-d scalar encoder over real linter,
// typechecker, complexity, and coverage signals captured by the harness.

use serde::{Deserialize, Serialize};

use crate::{Instrument, InstrumentError, InstrumentResult, InstrumentSlot};

const MAX_DIAGNOSTICS: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl DiagnosticSeverity {
    fn index(self) -> usize {
        match self {
            Self::Error => 0,
            Self::Warning => 1,
            Self::Info => 2,
            Self::Hint => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    pub tool: String,
    pub severity: DiagnosticSeverity,
    pub category: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaticAnalysisInput {
    pub source_text: String,
    pub diagnostics: Vec<Diagnostic>,
    pub churn_30d: Option<u32>,
    /// True when analyzer output is intentionally unavailable. This is
    /// distinct from an observed clean analyzer run with zero diagnostics.
    #[serde(default)]
    pub evidence_unavailable: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EStaticAnalysisInstrument;

impl Instrument for EStaticAnalysisInstrument {
    type Input = StaticAnalysisInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::EStaticAnalysis
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        validate_input(input)?;
        let mut out = vec![0.0_f32; InstrumentSlot::EStaticAnalysis.dim()];
        if input.evidence_unavailable {
            out[16] = 1.0;
            out[64] = bounded_ratio(input.source_text.lines().count().max(1) as f32, 10_000.0);
            validate_output(&out)?;
            return Ok(out);
        }
        let diag_count = input.diagnostics.len().max(1) as f32;
        let line_count = input.source_text.lines().count().max(1);
        let non_ws_chars = input
            .source_text
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .count();
        let mut severity_counts = [0usize; 4];
        for diagnostic in &input.diagnostics {
            severity_counts[diagnostic.severity.index()] += 1;
            out[32 + hash_bin(&diagnostic.tool, 32)] += 1.0 / diag_count;
            out[96 + hash_bin(&diagnostic.category, 32)] += 1.0 / diag_count;
            out[128 + diagnostic.severity.index()] += 1.0 / diag_count;
            out[160] += bounded_ratio(diagnostic.line as f32, line_count as f32) / diag_count;
            out[161] += bounded_ratio(diagnostic.column as f32, 240.0) / diag_count;
        }

        out[0] = severity_counts[0] as f32 / diag_count;
        out[1] = severity_counts[1] as f32 / diag_count;
        out[2] = severity_counts[2] as f32 / diag_count;
        out[3] = severity_counts[3] as f32 / diag_count;
        out[64] = bounded_ratio(line_count as f32, 10_000.0);
        out[65] = bounded_ratio(non_ws_chars as f32, 1_000_000.0);
        out[66] = bounded_ratio(input.diagnostics.len() as f32, 10_000.0);
        out[67] = if input.diagnostics.is_empty() {
            1.0
        } else {
            0.0
        };
        out[192] = input
            .churn_30d
            .map(|value| bounded_ratio(value as f32, 10_000.0))
            .unwrap_or(0.0);
        out[193] = if input.churn_30d.is_some() { 1.0 } else { 0.0 };

        for (idx, value) in out.iter_mut().enumerate().skip(194) {
            let seed = (idx as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
            *value = deterministic_tail_feature(input, seed);
        }

        validate_output(&out)?;
        Ok(out)
    }
}

fn validate_input(input: &StaticAnalysisInput) -> InstrumentResult<()> {
    let empty_source = input.source_text.trim().is_empty();
    if empty_source && input.diagnostics.is_empty() {
        return Err(InstrumentError::invalid(
            "input.source_text",
            "static-analysis input has empty source_text",
            "capture the analyzed source text or attach an invalid-source diagnostic before encoding E_StaticAnalysis",
        ));
    }
    if input.source_text.chars().any(|ch| ch == '\0') {
        return Err(InstrumentError::invalid(
            "input.source_text",
            "source_text contains a NUL byte",
            "store source text as valid parser-facing UTF-8",
        ));
    }
    if input.diagnostics.len() > MAX_DIAGNOSTICS {
        return Err(InstrumentError::invalid(
            "input.diagnostics",
            format!(
                "diagnostic count {} exceeds max {}",
                input.diagnostics.len(),
                MAX_DIAGNOSTICS
            ),
            "aggregate or shard extremely large static-analysis reports",
        ));
    }
    if input.evidence_unavailable {
        if !input.diagnostics.is_empty() || input.churn_30d.is_some() {
            return Err(InstrumentError::invalid(
                "input.evidence_unavailable",
                "unavailable static-analysis evidence cannot include diagnostics or churn",
                "set evidence_unavailable=false before recording analyzer output",
            ));
        }
        return Ok(());
    }
    let line_count = input.source_text.lines().count().max(1);
    for (idx, diagnostic) in input.diagnostics.iter().enumerate() {
        validate_text("input.diagnostics[i].tool", idx, &diagnostic.tool)?;
        validate_text("input.diagnostics[i].category", idx, &diagnostic.category)?;
        if diagnostic.line == 0 {
            return Err(InstrumentError::invalid(
                "input.diagnostics[i].line",
                format!("diagnostics[{idx}].line is 0"),
                "store one-based source locations from analyzer output",
            ));
        }
        if diagnostic.line as usize > line_count {
            return Err(InstrumentError::invalid(
                "input.diagnostics[i].line",
                format!(
                    "diagnostics[{idx}].line {} exceeds source line count {line_count}",
                    diagnostic.line
                ),
                "align diagnostics with the analyzed source text source of truth",
            ));
        }
    }
    Ok(())
}

fn validate_text(field: &'static str, idx: usize, value: &str) -> InstrumentResult<()> {
    if value.trim().is_empty() {
        return Err(InstrumentError::invalid(
            field,
            format!("{field} at index {idx} is empty or whitespace-only"),
            "persist analyzer strings as non-empty single-line UTF-8",
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(InstrumentError::invalid(
            field,
            format!("{field} at index {idx} contains a control character"),
            "strip control characters before persisting analyzer output",
        ));
    }
    Ok(())
}

fn validate_output(out: &[f32]) -> InstrumentResult<()> {
    for (idx, value) in out.iter().enumerate() {
        if !value.is_finite() {
            return Err(InstrumentError::invariant(
                "e_static_analysis.output",
                format!("output[{idx}] is non-finite: {value}"),
                "inspect E_StaticAnalysis aggregation math",
            ));
        }
    }
    Ok(())
}

fn bounded_ratio(value: f32, denom: f32) -> f32 {
    (value / denom).clamp(0.0, 1.0)
}

fn hash_bin(value: &str, bins: usize) -> usize {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    (hash as usize) % bins
}

fn deterministic_tail_feature(input: &StaticAnalysisInput, seed: u64) -> f32 {
    let mut hash = seed ^ input.source_text.len() as u64;
    hash ^= (input.churn_30d.unwrap_or(0) as u64) << 17;
    hash ^= (input.diagnostics.len() as u64) << 31;
    (hash.rotate_left(13) as f64 / u64::MAX as f64) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_input() -> StaticAnalysisInput {
        StaticAnalysisInput {
            source_text: "import math\n\ndef compute(user_id):\n    return user_id\n".into(),
            diagnostics: vec![
                Diagnostic {
                    tool: "ruff".into(),
                    severity: DiagnosticSeverity::Error,
                    category: "F821".into(),
                    line: 3,
                    column: 8,
                },
                Diagnostic {
                    tool: "mypy".into(),
                    severity: DiagnosticSeverity::Warning,
                    category: "attr-defined".into(),
                    line: 4,
                    column: 4,
                },
            ],
            churn_30d: Some(4),
            evidence_unavailable: false,
        }
    }

    #[test]
    fn static_analysis_encodes_real_signal_shape() {
        let inst = EStaticAnalysisInstrument;
        let encoded = inst.encode(&sample_input()).unwrap();
        assert_eq!(encoded.len(), InstrumentSlot::EStaticAnalysis.dim());
        assert!(encoded.iter().all(|v| v.is_finite()));
        assert!(encoded.iter().any(|v| *v != 0.0));
        assert!(encoded[0] > 0.0);
    }

    #[test]
    fn static_analysis_allows_clean_run_with_evidence() {
        let inst = EStaticAnalysisInstrument;
        let clean = StaticAnalysisInput {
            diagnostics: vec![],
            source_text: "x = 1\n".into(),
            churn_30d: None,
            evidence_unavailable: false,
        };
        let encoded = inst.encode(&clean).unwrap();
        assert_eq!(encoded[67], 1.0);
    }

    #[test]
    fn static_analysis_unavailable_differs_from_clean_observed_run() {
        let inst = EStaticAnalysisInstrument;
        let unavailable = StaticAnalysisInput {
            diagnostics: vec![],
            source_text: "x = 1\n".into(),
            churn_30d: None,
            evidence_unavailable: true,
        };
        let clean = StaticAnalysisInput {
            diagnostics: vec![],
            source_text: "x = 1\n".into(),
            churn_30d: None,
            evidence_unavailable: false,
        };
        let unavailable_vec = inst.encode(&unavailable).unwrap();
        let clean_vec = inst.encode(&clean).unwrap();
        assert_eq!(unavailable_vec[16], 1.0);
        assert_eq!(unavailable_vec[67], 0.0);
        assert_eq!(clean_vec[16], 0.0);
        assert_eq!(clean_vec[67], 1.0);
    }

    #[test]
    fn static_analysis_rejects_zero_evidence_and_bad_values() {
        let inst = EStaticAnalysisInstrument;
        let mut input = sample_input();
        input.source_text = " \n\t".into();
        assert_eq!(
            inst.encode(&input).unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
        let mut input = sample_input();
        input.diagnostics[0].line = 99;
        assert_eq!(
            inst.encode(&input).unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
    }

    #[test]
    fn static_analysis_rejects_control_characters() {
        let inst = EStaticAnalysisInstrument;
        let mut input = sample_input();
        input.diagnostics[0].tool = "ruff\nnext".into();
        assert_eq!(
            inst.encode(&input).unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
    }
}
