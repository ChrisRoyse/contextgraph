use crate::error::{EmbedError, EmbedResult};
use crate::types::VramBudgetReport;
use context_graph_cuda::safe::GpuDevice;
use serde::{Deserialize, Serialize};

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
