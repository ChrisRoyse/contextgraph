use serde::{Deserialize, Serialize};

use crate::features::{bounded_ratio, validate_finite_output};
use crate::{Instrument, InstrumentError, InstrumentResult, InstrumentSlot};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeInput {
    pub wall_time_ms: u64,
    pub peak_rss_bytes: u64,
    pub exit_code: i32,
    pub timed_out: bool,
    pub coverage_percent: Option<f32>,
    pub network_events: u32,
    pub filesystem_writes: u32,
    /// True when runtime evidence is intentionally unavailable because the
    /// code has not been executed yet. This is distinct from an observed
    /// timeout or non-zero exit code.
    #[serde(default)]
    pub evidence_unavailable: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ERuntimeInstrument;

impl Instrument for ERuntimeInstrument {
    type Input = RuntimeInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::ERuntime
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        validate_runtime(input)?;
        let mut out = vec![0.0_f32; InstrumentSlot::ERuntime.dim()];
        if input.evidence_unavailable {
            out[16] = 1.0;
            validate_finite_output("e_runtime.output", &out)?;
            return Ok(out);
        }
        out[0] = bounded_ratio(input.wall_time_ms as f32, 3_600_000.0);
        out[1] = bounded_ratio(
            input.peak_rss_bytes as f32,
            128.0 * 1024.0 * 1024.0 * 1024.0,
        );
        out[2] = if input.exit_code == 0 { 1.0 } else { 0.0 };
        out[3] = if input.timed_out { 1.0 } else { 0.0 };
        out[4] = bounded_ratio(input.exit_code.unsigned_abs() as f32, 255.0);
        out[5] = input.coverage_percent.unwrap_or(-1.0) / 100.0;
        out[6] = if input.coverage_percent.is_some() {
            1.0
        } else {
            0.0
        };
        out[7] = bounded_ratio(input.network_events as f32, 100_000.0);
        out[8] = bounded_ratio(input.filesystem_writes as f32, 1_000_000.0);
        for (idx, value) in out.iter_mut().enumerate().skip(32) {
            let base = input.wall_time_ms
                ^ input.peak_rss_bytes
                ^ ((input.network_events as u64) << 16)
                ^ ((input.filesystem_writes as u64) << 32);
            *value = ((base.rotate_left((idx % 63) as u32) as f64 / u64::MAX as f64) as f32)
                .clamp(0.0, 1.0);
        }
        validate_finite_output("e_runtime.output", &out)?;
        Ok(out)
    }
}

fn validate_runtime(input: &RuntimeInput) -> InstrumentResult<()> {
    if input.evidence_unavailable {
        if input.wall_time_ms != 0
            || input.peak_rss_bytes != 0
            || input.exit_code != -1
            || input.timed_out
            || input.coverage_percent.is_some()
            || input.network_events != 0
            || input.filesystem_writes != 0
        {
            return Err(InstrumentError::invalid(
                "input.evidence_unavailable",
                "unavailable runtime evidence cannot also carry observed runtime values",
                "set evidence_unavailable=false before recording runtime observations",
            ));
        }
        return Ok(());
    }
    if input.wall_time_ms == 0 && !input.timed_out {
        return Err(InstrumentError::invalid(
            "input.wall_time_ms",
            "runtime wall_time_ms is zero for a non-timeout run",
            "record measured wall-clock time from the actual test/oracle run",
        ));
    }
    if let Some(coverage) = input.coverage_percent {
        if !coverage.is_finite() || !(0.0..=100.0).contains(&coverage) {
            return Err(InstrumentError::invalid(
                "input.coverage_percent",
                format!("coverage_percent must be finite in [0,100], got {coverage}"),
                "persist coverage as a percent or omit it when unavailable",
            ));
        }
    }
    Ok(())
}
