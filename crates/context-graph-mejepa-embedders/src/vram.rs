use crate::error::{EmbedError, EmbedResult};
use crate::types::VramBudgetReport;
use context_graph_cuda::safe::GpuDevice;
use serde::{Deserialize, Serialize};
use std::process::Command;

pub const GB: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VramBudget {
    pub required_bytes: u64,
    pub device_ordinal: i32,
}

impl VramBudget {
    pub const fn content_set_rtx5090() -> Self {
        Self {
            required_bytes: 18 * GB,
            device_ordinal: 0,
        }
    }

    pub const fn full_phase1_rtx5090() -> Self {
        Self {
            required_bytes: 29 * GB,
            device_ordinal: 0,
        }
    }
}

pub fn query_vram_budget(budget: VramBudget) -> EmbedResult<VramBudgetReport> {
    let device = GpuDevice::new(budget.device_ordinal).map_err(|err| EmbedError::GpuUnavailable {
        message: format!("{err}"),
        remediation: "fix Windows/WSL CUDA access and verify `nvidia-smi` works in this shell before loading embedders",
    })?;
    let (free, total) = device
        .memory_info()
        .map_err(|err| EmbedError::GpuUnavailable {
            message: format!("{err}"),
            remediation:
                "fix CUDA context creation before running cuMemGetInfo_v2-backed budget checks",
        })?;
    let (major, minor) = device
        .compute_capability()
        .map_err(|err| EmbedError::GpuUnavailable {
            message: format!("{err}"),
            remediation:
                "fix CUDA device-attribute access before running compute-capability checks",
        })?;
    let gpu_name = device.name().map_err(|err| EmbedError::GpuUnavailable {
        message: format!("{err}"),
        remediation: "fix CUDA device-name access before loading embedders",
    })?;
    let report = VramBudgetReport {
        required_bytes: budget.required_bytes,
        free_bytes: free as u64,
        total_bytes: total as u64,
        gpu_name,
        compute_capability: format!("{major}.{minor}"),
        telemetry_source: "cuda_driver_cuMemGetInfo_v2".to_string(),
        nvidia_smi_status: nvidia_smi_status(budget.device_ordinal),
        nvidia_smi_total_mb: nvidia_smi_total_mb(budget.device_ordinal).ok(),
        passes: (free as u64) >= budget.required_bytes,
    };
    if !report.passes {
        return Err(EmbedError::VramExceeded {
            required_bytes: report.required_bytes,
            free_bytes: report.free_bytes,
            total_bytes: report.total_bytes,
            remediation: "stop other GPU workloads or reduce hard-warmed learner-state embedders before loading the ensemble",
        });
    }
    Ok(report)
}

fn nvidia_smi_status(device_ordinal: i32) -> String {
    match nvidia_smi_total_mb(device_ordinal) {
        Ok(total) => format!("diagnostic_available_total_mb={total}"),
        Err(message) => format!("diagnostic_unavailable_not_authoritative: {message}"),
    }
}

fn nvidia_smi_total_mb(device_ordinal: i32) -> Result<u64, String> {
    let ordinal = device_ordinal.to_string();
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=memory.total",
            "--format=csv,noheader,nounits",
            "-i",
            &ordinal,
        ])
        .output()
        .map_err(|err| format!("failed to execute nvidia-smi: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "nvidia-smi exited with status {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|err| format!("nvidia-smi stdout was not UTF-8: {err}"))?;
    let first = stdout
        .lines()
        .next()
        .ok_or_else(|| "nvidia-smi returned no rows".to_string())?
        .trim();
    if first.is_empty() || first.eq_ignore_ascii_case("[not supported]") {
        return Err(format!("nvidia-smi returned unsupported memory.total row {first:?}"));
    }
    first
        .parse::<u64>()
        .map_err(|err| format!("failed to parse nvidia-smi memory.total row {first:?}: {err}"))
}
