use candle_core::{Device, DeviceLocation};
use serde::{Deserialize, Serialize};

use crate::config::{VRAM_STEADY_STATE_TARGET_BYTES, VRAM_WARN_THRESHOLD_BYTES};
use crate::error::PredictorError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VramReport {
    pub free_bytes: u64,
    pub total_bytes: u64,
    pub resident_bytes: u64,
    pub warn_threshold_bytes: u64,
    pub target_bytes: u64,
    pub device: String,
    pub dod_pass_8gb: bool,
    pub crossed_warn_threshold: bool,
}

impl VramReport {
    pub fn from_measurement(free_bytes: u64, total_bytes: u64, device: String) -> Self {
        let resident_bytes = total_bytes.saturating_sub(free_bytes);
        Self {
            free_bytes,
            total_bytes,
            resident_bytes,
            warn_threshold_bytes: VRAM_WARN_THRESHOLD_BYTES,
            target_bytes: VRAM_STEADY_STATE_TARGET_BYTES,
            device,
            dod_pass_8gb: resident_bytes < VRAM_STEADY_STATE_TARGET_BYTES,
            crossed_warn_threshold: resident_bytes >= VRAM_WARN_THRESHOLD_BYTES,
        }
    }
}

pub fn check_vram_steady_state(device: &Device) -> Result<VramReport, PredictorError> {
    let report = probe_vram(device)?;
    enforce_vram_report(report)
}

pub fn enforce_vram_report(report: VramReport) -> Result<VramReport, PredictorError> {
    if report.crossed_warn_threshold {
        return Err(PredictorError::VramExceeded {
            vram_resident_bytes: report.resident_bytes,
            threshold_bytes: report.warn_threshold_bytes,
        });
    }
    if !report.dod_pass_8gb {
        tracing::warn!(
            target: "mejepa::vram",
            resident_bytes = report.resident_bytes,
            target_bytes = report.target_bytes,
            warn_threshold_bytes = report.warn_threshold_bytes,
            "VRAM is above the Phase 2 steady-state target but below the hard warn threshold"
        );
    }
    Ok(report)
}

pub fn vram_resident_bytes(device: &Device) -> Result<u64, PredictorError> {
    Ok(probe_vram(device)?.resident_bytes)
}

fn probe_vram(device: &Device) -> Result<VramReport, PredictorError> {
    match device.location() {
        DeviceLocation::Cuda { gpu_id: 0 } => {}
        DeviceLocation::Cuda { gpu_id } => {
            return Err(PredictorError::DeviceUnavailable {
                detail: format!("VRAM probe requires CUDA device 0; got cuda:{gpu_id}"),
            });
        }
        other => {
            return Err(PredictorError::DeviceUnavailable {
                detail: format!("VRAM probe requires CUDA; got {other:?}; no CPU fallback"),
            });
        }
    }
    device.synchronize()?;
    let gpu =
        context_graph_cuda::GpuDevice::new(0).map_err(|err| PredictorError::DeviceUnavailable {
            detail: format!("CUDA driver VRAM probe failed during GpuDevice::new(0): {err}"),
        })?;
    let (free, total) = gpu
        .memory_info()
        .map_err(|err| PredictorError::DeviceUnavailable {
            detail: format!("CUDA driver cuMemGetInfo_v2 failed: {err}"),
        })?;
    Ok(VramReport::from_measurement(
        free as u64,
        total as u64,
        format!("{:?}", device.location()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn vram_classification_boundaries() {
        let total = 32 * GIB;
        let under = VramReport::from_measurement(total - (7 * GIB), total, "cuda:0".to_string());
        assert!(under.dod_pass_8gb);
        assert!(!under.crossed_warn_threshold);

        let elevated =
            VramReport::from_measurement(total - (8 * GIB + 64), total, "cuda:0".to_string());
        assert!(!elevated.dod_pass_8gb);
        assert!(!elevated.crossed_warn_threshold);

        let exceeded =
            VramReport::from_measurement(total - (9 * GIB + 64), total, "cuda:0".to_string());
        assert!(!exceeded.dod_pass_8gb);
        assert!(exceeded.crossed_warn_threshold);
    }
}
