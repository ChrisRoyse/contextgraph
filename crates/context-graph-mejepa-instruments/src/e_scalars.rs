use serde::{Deserialize, Serialize};

use crate::features::{bounded_ratio, validate_finite_output};
use crate::{Instrument, InstrumentError, InstrumentResult, InstrumentSlot};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScalarInput {
    pub bfs_depth: u32,
    pub blame_age_days: u32,
    pub churn_lines_30d: u32,
    pub coverage_delta: f32,
    pub repo_health_score: f32,
    pub files_touched: u32,
    pub hunks_touched: u32,
    /// True when runtime/repository scalar evidence is unavailable. Patch
    /// structure scalars may still be present, but neutral coverage/health
    /// values must not be read as observed measurements.
    #[serde(default)]
    pub evidence_unavailable: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ScalarsInstrument;

impl Instrument for ScalarsInstrument {
    type Input = ScalarInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::Scalars
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        validate_scalars(input)?;
        let mut out = vec![0.0_f32; InstrumentSlot::Scalars.dim()];
        out[0] = bounded_ratio(input.bfs_depth as f32, 10_000.0);
        out[1] = bounded_ratio(input.blame_age_days as f32, 3650.0);
        out[2] = bounded_ratio(input.churn_lines_30d as f32, 100_000.0);
        out[3] = (input.coverage_delta / 100.0).clamp(-1.0, 1.0);
        out[4] = input.repo_health_score;
        out[5] = bounded_ratio(input.files_touched as f32, 10_000.0);
        out[6] = bounded_ratio(input.hunks_touched as f32, 100_000.0);
        out[7] = if input.evidence_unavailable { 1.0 } else { 0.0 };
        for (idx, value) in out.iter_mut().enumerate().skip(16) {
            let mix = input.bfs_depth as u64
                ^ ((input.blame_age_days as u64) << 9)
                ^ ((input.churn_lines_30d as u64) << 19)
                ^ ((input.files_touched as u64) << 31)
                ^ ((input.hunks_touched as u64) << 43)
                ^ idx as u64;
            *value = (mix.rotate_left((idx % 61) as u32) as f64 / u64::MAX as f64) as f32;
        }
        validate_finite_output("scalars.output", &out)?;
        Ok(out)
    }
}

fn validate_scalars(input: &ScalarInput) -> InstrumentResult<()> {
    if !input.coverage_delta.is_finite() {
        return Err(InstrumentError::invalid(
            "input.coverage_delta",
            "coverage_delta is non-finite",
            "persist finite scalar values only",
        ));
    }
    if !input.repo_health_score.is_finite() || !(0.0..=1.0).contains(&input.repo_health_score) {
        return Err(InstrumentError::invalid(
            "input.repo_health_score",
            format!(
                "repo_health_score must be finite in [0,1], got {}",
                input.repo_health_score
            ),
            "normalize repo-health metrics before filling Scalars",
        ));
    }
    Ok(())
}
